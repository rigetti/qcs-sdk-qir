// Copyright 2022 Rigetti Computing
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::{collections::HashMap, sync::LazyLock};

use eyre::{eyre, Result};
use inkwell::{
    basic_block::BasicBlock,
    values::{FloatValue, InstructionOpcode, InstructionValue},
};
use log::{debug, info};
use qcs::quil_rs::instruction::{Gate, Instruction, Measurement};
use qcs::quil_rs::Program;
use qcs::quil_rs::{
    expression::Expression,
    instruction::{GateModifier, MemoryReference, Qubit},
};
use regex::Regex;

use crate::{
    context::QCSCompilerContext,
    interop::instruction::{
        get_called_function_name, get_qis_function_arguments, OperationArgument,
    },
    transform::PARAMETER_MEMORY_REGION_NAME,
    RecordedOutput,
};

/// Each pattern matching function, if it finds a match, returns a result tuple including:
/// * data about the pattern which was matched.
/// * what the next instruction following the pattern is, if any.
type PatternResult<'ctx, T> = Option<(Option<InstructionValue<'ctx>>, T)>;

/// A `UnitaryPatternMatchContext` accumulates state as it scans a QIR program.
/// It is used to extract a single quantum program from a QIR program, without shot count
/// or any classical instructions beyond a final return statement.
///
/// The pattern it looks for within a basic block is the following:
///
/// * A sequence of these instructions only:
///   * Quantum Instructions - `quantum_qis` intrinsics
///   * Select Quantum Runtime Instructions - `quantum_rt`
/// * Terminated by _exactly_ a `ret void`
///
/// If any other instructions are encountered, an error is returned.
#[derive(Debug, Default)]
pub(crate) struct UnitaryPatternMatchContext<'ctx> {
    /// The quil program transpiled from quantum intrinsics
    pub(crate) quil_program: Program,

    /// Signifies output to be recorded at the end of program execution
    pub(crate) recorded_output: Vec<RecordedOutput>,

    /// A list of instructions to remove from the program (for substitution with Quil)
    pub(crate) instructions_to_remove: Vec<InstructionValue<'ctx>>,

    /// Mapping of `(read_result *Result index) - >(ro memory region index)`
    pub(crate) read_result_mapping: HashMap<u64, u64>,

    /// Pairings of (readout buffer index/offset) with the instruction which stores that readout value.
    pub(crate) readout_instruction_mapping: Vec<(u64, InstructionValue<'ctx>)>,

    /// All `FloatValues` used as instruction parameters. Indices within this Vec map to indices within the Quil
    /// `MemoryRegion` used to read the values at runtime.
    pub(crate) parameters: Vec<FloatValue<'ctx>>,

    /// Whether or not to prepend a RESET instruction to the program to actively reset all qubits on each shot
    pub(crate) use_active_reset: bool,
}

impl<'ctx> UnitaryPatternMatchContext<'ctx> {
    /// Build the pattern context from a basic block.
    ///
    /// # Arguments
    ///
    /// * `context`: overall compiler context
    /// * `basic_block`: the subject block to be searched for the pattern
    pub(crate) fn from_basic_block(
        context: &mut QCSCompilerContext<'ctx>,
        basic_block: BasicBlock<'ctx>,
    ) -> Result<Self> {
        let mut next_instruction = basic_block.get_first_instruction();
        let mut pattern_context = UnitaryPatternMatchContext::default();

        info!(
            "starting transpile: block {}",
            basic_block.get_name().to_str()?
        );

        while let Some(instruction) = next_instruction {
            // If we haven't yet found the loop start...
            if let Some((pattern_instruction, ())) =
                quantum_instruction(context, &mut pattern_context, instruction)?
            {
                debug!("matched quantum instruction: {instruction:?}");
                next_instruction = pattern_instruction;
            } else if let Some((pattern_instruction, ())) =
                rt_record_instruction(context, &mut pattern_context, instruction)?
            {
                debug!("matched rt_record instruction: {instruction:?}");
                next_instruction = pattern_instruction;
            } else if instruction.get_opcode() == InstructionOpcode::Return {
                return Ok(pattern_context);
            } else {
                return Err(eyre::eyre!(
                    "found instruction disallowed in Unitary QIR: {:?}",
                    instruction
                ));
            }
        }

        Ok(pattern_context)
    }

    /// If the program contains any executable instructions (gates, pulses, etc) return that
    /// information; otherwise, return `None` indicating that the pattern was not matched.
    pub(crate) fn get_program_data(&self) -> Option<&Program> {
        if self.quil_program.body_instructions().count() == 0 {
            None
        } else {
            Some(&self.quil_program)
        }
    }

    /// Returns the parameters which do not have a constant value.
    pub(crate) fn get_dynamic_parameters(&self) -> Vec<&FloatValue<'ctx>> {
        self.parameters
            .iter()
            .filter(|v| !v.is_const())
            .collect::<Vec<&FloatValue>>()
    }
}

macro_rules! match_qis_argument {
    ($variant:ident, $arguments:expr, $index:expr, $function_name:expr) => {{
        use OperationArgument::*;

        match $arguments.get($index) {
            Some($variant(contents)) => Ok(contents),
            other => Err(eyre!(
                "expected argument {} of {} to be of type {:?}; got {:?}",
                $index,
                $function_name,
                stringify!($variant),
                other
            )),
        }
    }};
}

/// Given a `FloatValue` which may be the parameter of a QIS intrinsic call, return the Quil Expression
/// which will be used to store its value at execution time.
fn get_quil_parameter_expression<'ctx>(
    pattern_context: &mut UnitaryPatternMatchContext<'ctx>,
    float_value: FloatValue<'ctx>,
) -> Expression {
    if let Some((constant, _)) = float_value.get_constant() {
        return Expression::Number(constant.into());
    }

    let index = get_quil_parameter_index(pattern_context, float_value);
    Expression::Address(MemoryReference {
        name: String::from(PARAMETER_MEMORY_REGION_NAME),
        index: index as u64,
    })
}

/// Given a `FloatValue` to be used as the parameter to a gate, return the index within the
/// Quil `MemoryReference` that should be used to store this parameter's value.
pub(crate) fn get_quil_parameter_index<'ctx>(
    pattern_context: &mut UnitaryPatternMatchContext<'ctx>,
    float_value: FloatValue<'ctx>,
) -> usize {
    if let Some(index) = pattern_context
        .parameters
        .iter()
        .position(|el| el == &float_value)
    {
        index
    } else {
        pattern_context.parameters.push(float_value);
        pattern_context.parameters.len() - 1
    }
}

#[allow(clippy::too_many_arguments)]
fn add_gate_instruction<'ctx>(
    pattern_context: &mut UnitaryPatternMatchContext<'ctx>,
    arguments: &[OperationArgument<'ctx>],
    function_name: &str,
    name: &str,
    adjoint: bool,
    controlled: bool,
    parameter_count: usize,
    qubit_count: usize,
) -> Result<()> {
    let parameters = (0..parameter_count)
        .map(|arg_index| {
            let float_value = *match_qis_argument!(Parameter, arguments, arg_index, function_name)?;
            Ok(get_quil_parameter_expression(pattern_context, float_value))
        })
        .collect::<Result<Vec<Expression>>>()?;

    let qubits = (parameter_count..parameter_count + qubit_count)
        .map(|arg_index| {
            Ok(Qubit::Fixed(*match_qis_argument!(
                Qubit,
                arguments,
                arg_index,
                function_name
            )?))
        })
        .collect::<Result<Vec<Qubit>>>()?;

    let mut modifiers = vec![];

    if adjoint {
        modifiers.push(GateModifier::Dagger);
    }

    if controlled {
        modifiers.push(GateModifier::Controlled);
    }

    let instruction = Instruction::Gate(Gate {
        name: name.to_owned(),
        parameters,
        qubits,
        modifiers,
    });

    pattern_context.quil_program.add_instruction(instruction);
    Ok(())
}

static QIS_INTRINSIC_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^__quantum__qis__(?P<operation>[^_]+)(?P<controlled>__ctl)?(?P<adjoint>__adj)?(__body)?$",
    )
    .unwrap()
});

static RT_RECORD_OUTPUT_INTRINSIC_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("^__quantum__rt__(?P<record_type>.+)_record_output$").unwrap());

pub(crate) fn rt_record_instruction<'ctx>(
    context: &QCSCompilerContext<'ctx>,
    pattern_context: &mut UnitaryPatternMatchContext<'ctx>,
    instruction: InstructionValue<'ctx>,
) -> Result<PatternResult<'ctx, ()>> {
    match instruction.get_opcode() {
        inkwell::values::InstructionOpcode::Call => {
            let function_target_name = get_called_function_name(instruction)?;

            if let Some(function_name) = function_target_name {
                if let Some(captures) = RT_RECORD_OUTPUT_INTRINSIC_REGEX.captures(&function_name) {
                    let record_type = &captures["record_type"];

                    match record_type {
                        "result" => {
                            let arguments = get_qis_function_arguments(context, instruction)?;
                            if let Some(OperationArgument::Result(result_index)) = arguments.first()
                            {
                                let next_ro_index =
                                    pattern_context.read_result_mapping.len() as u64;
                                let index = pattern_context.read_result_mapping.entry(*result_index).or_insert_with(|| {
                                    log::info!("Result index {result_index} was read but was never the target of a measurement operation, so recorded output value will always be 0");
                                    next_ro_index
                                });
                                pattern_context
                                    .recorded_output
                                    .push(RecordedOutput::ResultReadoutOffset(*index));
                            } else {
                                return Err(eyre!(
                                    "malformed or missing arguments for: __quantum_rt__{}_record_output",
                                    record_type
                                ));
                            }
                        }
                        "bool" | "integer" | "double" => {
                            return Err(eyre!("unimplemented record type: {}", record_type));
                        }
                        "tuple_start" => pattern_context
                            .recorded_output
                            .push(RecordedOutput::TupleStart),
                        "tuple_end" => pattern_context
                            .recorded_output
                            .push(RecordedOutput::TupleEnd),
                        "array_start" => pattern_context
                            .recorded_output
                            .push(RecordedOutput::ArrayStart),
                        "array_end" => pattern_context
                            .recorded_output
                            .push(RecordedOutput::ArrayEnd),
                        _ => {
                            return Ok(None);
                        }
                    }
                    return Ok(Some((instruction.get_next_instruction(), ())));
                }
            }
            Ok(None)
        }
        _ => Ok(None),
    }
}

#[allow(clippy::too_many_lines)]
pub(crate) fn quantum_instruction<'ctx>(
    context: &QCSCompilerContext<'ctx>,
    pattern_context: &mut UnitaryPatternMatchContext<'ctx>,
    instruction: InstructionValue<'ctx>,
) -> Result<PatternResult<'ctx, ()>> {
    match instruction.get_opcode() {
        inkwell::values::InstructionOpcode::Call => {
            let function_target_name = get_called_function_name(instruction)?;

            if let Some(function_name) = function_target_name {
                if let Some(captures) = QIS_INTRINSIC_REGEX.captures(&function_name) {
                    let operation = &captures["operation"];
                    let adjoint = captures.name("adjoint").is_some();
                    let controlled = captures.name("controlled").is_some();

                    let arguments = get_qis_function_arguments(context, instruction)?;

                    let matched = match operation {
                        "swap" => {
                            add_gate_instruction(
                                pattern_context,
                                &arguments,
                                &function_name,
                                "SWAP",
                                adjoint,
                                controlled,
                                0,
                                2,
                            )?;
                            true
                        }
                        "toffoli" => {
                            add_gate_instruction(
                                pattern_context,
                                &arguments,
                                &function_name,
                                "CCNOT",
                                adjoint,
                                controlled,
                                0,
                                3,
                            )?;
                            true
                        }
                        "cnot" => {
                            add_gate_instruction(
                                pattern_context,
                                &arguments,
                                &function_name,
                                "CNOT",
                                adjoint,
                                controlled,
                                0,
                                2,
                            )?;
                            true
                        }
                        "cz" => {
                            add_gate_instruction(
                                pattern_context,
                                &arguments,
                                &function_name,
                                "CZ",
                                adjoint,
                                controlled,
                                0,
                                2,
                            )?;
                            true
                        }
                        "h" => {
                            add_gate_instruction(
                                pattern_context,
                                &arguments,
                                &function_name,
                                "H",
                                adjoint,
                                controlled,
                                0,
                                1,
                            )?;
                            true
                        }
                        "reset" => {
                            // TODO: Alert the user that we're resetting for all qubits instead of just the targeted qubit here
                            pattern_context.use_active_reset = true;
                            true
                        }
                        "rx" => {
                            add_gate_instruction(
                                pattern_context,
                                &arguments,
                                &function_name,
                                "RX",
                                adjoint,
                                controlled,
                                1,
                                1,
                            )?;
                            true
                        }
                        "ry" => {
                            add_gate_instruction(
                                pattern_context,
                                &arguments,
                                &function_name,
                                "RY",
                                adjoint,
                                controlled,
                                1,
                                1,
                            )?;
                            true
                        }
                        "rz" => {
                            add_gate_instruction(
                                pattern_context,
                                &arguments,
                                &function_name,
                                "RZ",
                                adjoint,
                                controlled,
                                1,
                                1,
                            )?;
                            true
                        }
                        "s" => {
                            add_gate_instruction(
                                pattern_context,
                                &arguments,
                                &function_name,
                                "S",
                                adjoint,
                                controlled,
                                0,
                                1,
                            )?;
                            true
                        }
                        "t" => {
                            add_gate_instruction(
                                pattern_context,
                                &arguments,
                                &function_name,
                                "T",
                                adjoint,
                                controlled,
                                0,
                                1,
                            )?;
                            true
                        }
                        "x" => {
                            add_gate_instruction(
                                pattern_context,
                                &arguments,
                                &function_name,
                                "X",
                                adjoint,
                                controlled,
                                0,
                                1,
                            )?;
                            true
                        }
                        "y" => {
                            add_gate_instruction(
                                pattern_context,
                                &arguments,
                                &function_name,
                                "Y",
                                adjoint,
                                controlled,
                                0,
                                1,
                            )?;
                            true
                        }
                        "z" => {
                            add_gate_instruction(
                                pattern_context,
                                &arguments,
                                &function_name,
                                "Z",
                                adjoint,
                                controlled,
                                0,
                                1,
                            )?;
                            true
                        }
                        "mz" => {
                            let qubit =
                                *match_qis_argument!(Qubit, arguments, 0, function_name.as_str())?;
                            let result =
                                *match_qis_argument!(Result, arguments, 1, function_name.as_str())?;

                            // Result indices may be sparse rather than increasing monotonically from 0.
                            // If used naively (i.e. %Result 5 as `ro[5]`), this would result in sparse, suboptimal allocation
                            // in the readout data fetched following execution. So, instead, we assign Result indices to
                            // monotonically increasing `ro` region offsets so as to keep `ro` dense.
                            let next_ro_index = pattern_context.read_result_mapping.len() as u64;
                            let ro_buffer_index = pattern_context
                                .read_result_mapping
                                .entry(result)
                                .or_insert_with(|| next_ro_index);

                            pattern_context
                                .quil_program
                                .add_instruction(Instruction::Measurement(Measurement {
                                    target: Some(MemoryReference {
                                        name: String::from("ro"),
                                        index: *ro_buffer_index,
                                    }),
                                    qubit: Qubit::Fixed(qubit),
                                }));

                            true
                        }
                        _ => false,
                    };

                    if matched {
                        pattern_context.instructions_to_remove.push(instruction);
                        Ok(Some((instruction.get_next_instruction(), ())))
                    } else {
                        Ok(None)
                    }
                } else if function_name == "__quantum__qis__read_result__body" {
                    let arguments = get_qis_function_arguments(context, instruction)?;
                    if let Some(OperationArgument::Result(result_index)) = arguments.first() {
                        let ro_index = pattern_context.read_result_mapping.get(result_index).ok_or_else(|| eyre!("Result index {} was never the target of a measurement operation", result_index))?;
                        pattern_context
                            .readout_instruction_mapping
                            .push((*ro_index, instruction));
                    } else {
                        // TODO: Support more read results
                        return Err(eyre!("malformed read_result instrinsic"));
                    }
                    pattern_context.instructions_to_remove.push(instruction);
                    Ok(Some((instruction.get_next_instruction(), ())))
                } else {
                    Ok(None)
                }
            } else {
                Ok(None)
            }
        }
        _ => Ok(None),
    }
}
