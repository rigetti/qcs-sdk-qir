//! This module concerns itself with the handling of patterns in spans of instructions.

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

use std::{
    collections::{hash_map::DefaultHasher, HashMap},
    convert::TryFrom,
    hash::{Hash, Hasher},
    sync::LazyLock,
};

use either::Either;
use eyre::{eyre, Result, WrapErr};
use inkwell::{
    basic_block::BasicBlock,
    values::{
        BasicValue, BasicValueEnum, FloatValue, FunctionValue, InstructionOpcode, InstructionValue,
    },
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
        get_called_function_name, get_qis_function_arguments, integer_value_to_u64,
        operand_to_integer, OperationArgument,
    },
    transform::PARAMETER_MEMORY_REGION_NAME,
    RecordedOutput,
};

/// Each pattern matching function, if it finds a match, returns a result tuple including:
/// * data about the pattern which was matched.
/// * what the next instruction following the pattern is, if any.
type PatternResult<'ctx, T> = Option<(Option<InstructionValue<'ctx>>, T)>;

/// A `ShotCountPatternMatchContext` accumulates state as it scans a QIR program.
/// It is used to infer a shot count from a fixed count loop wrapping some number of
/// quantum and classical instructions within a basic block.
///
/// The pattern it looks for is the following:
///
/// * loop start and shot count - see [`shot_count_loop_start`].
/// * any number of the following:
///   * quantum instructions, which it earmarks for removal from the program, transpiles to Quil,
///     and appends to its running Quil program - see [`quantum_instruction`]
///   * classical instructions, which are ignored and left in place.
/// * a shot count increment and branch instruction - see [`shot_count_loop_end`].
#[derive(Debug, Default)]
pub(crate) struct ShotCountPatternMatchContext<'ctx> {
    // The instruction used to initialize the shot count value
    // pub loop_initializer: Option<InstructionValue<'ctx>>,
    /// Hash of the instruction used to initialize the shot count value.
    /// By using the hash, we don't need to maintain a reference to the instruction itself.
    pub(crate) initial_instruction: Option<InstructionValue<'ctx>>,

    /// Hash of the instruction used to initialize the shot count value.
    /// By using the hash, we don't need to maintain a reference to the instruction itself.
    pub(crate) initial_instruction_hash: Option<u64>,

    /// The quil program transpiled from quantum intrinsics
    pub(crate) quil_program: Program,

    /// Signifies output to be recorded at the end of program execution
    pub(crate) recorded_output: Vec<RecordedOutput>,

    /// The shot count inferred from the loop instructions
    pub(crate) shot_count: Option<u64>,

    /// A list of instructions to remove from the program (for substitution with Quil)
    pub(crate) instructions_to_remove: Vec<InstructionValue<'ctx>>,

    /// The terminating instruction to append to the `BasicBlock` in place of a conditional
    /// branch on shot count
    pub(crate) next_basic_block: Option<BasicBlock<'ctx>>,

    /// Mapping of `(read_result *Result index) -> (ro memory region index)`
    pub(crate) read_result_mapping: HashMap<u64, u64>,

    /// Pairings of (readout buffer index/offset) with the instruction which stores that readout value.
    pub(crate) readout_instruction_mapping: Vec<(u64, InstructionValue<'ctx>)>,

    /// All `FloatValues` used as instruction parameters. Indices within this Vec map to indices within the Quil
    /// `MemoryRegion` used to read the values at runtime.
    pub(crate) parameters: Vec<FloatValue<'ctx>>,

    /// Whether or not to prepend a RESET instruction to the program to actively reset all qubits on each shot
    pub(crate) use_active_reset: bool,
}

impl<'ctx> ShotCountPatternMatchContext<'ctx> {
    /// If the program contains any executable instructions (gates, pulses, etc) and a shot count has been inferred,
    /// return that information; otherwise, return `None` indicating that the pattern was not matched.
    pub(crate) fn get_program_data(&self) -> Option<(&Program, u64)> {
        if let Some(shots) = self.shot_count {
            if self.quil_program.body_instructions().count() == 0 {
                None
            } else {
                Some((&self.quil_program, shots))
            }
        } else {
            None
        }
    }

    /// Build the pattern context from a basic block.
    ///
    /// # Arguments
    ///
    /// * `context`: overall compiler context
    /// * `basic_block`: the subject block to be searched for the pattern
    /// * `visited_functions`: list of function names which have already been transpiled. Used to prevent recursion loops.
    /// * `function_call_callback`: callback to be invoked when a function call is found within the block, in order to recursively
    ///   transpile an LLVM module.
    pub(crate) fn from_basic_block(
        context: &mut QCSCompilerContext<'ctx>,
        basic_block: BasicBlock<'ctx>,
        visited_functions: &[&str],
        function_call_callback: fn(
            &mut QCSCompilerContext<'ctx>,
            FunctionValue<'ctx>,
            &[&str],
        ) -> Result<()>,
    ) -> Result<Self> {
        let mut next_instruction = basic_block.get_first_instruction();
        let mut pattern_context = ShotCountPatternMatchContext::default();

        info!(
            "starting transpile: block {}",
            basic_block.get_name().to_str()?
        );

        while let Some(instruction) = next_instruction {
            // If we haven't yet found the loop start...
            if pattern_context.initial_instruction.is_none() {
                // Check if we've found it in this instruction. If not, continue on to the next instruction until we do find it.
                // FIXME: ensure we encounter this first (i.e. the pattern must be matched in order)
                if let Some((pattern_instruction, ())) =
                    shot_count_loop_start(&mut pattern_context, instruction)
                {
                    debug!("matched shot count start: {:?}", instruction);
                    pattern_context
                        .recorded_output
                        .push(RecordedOutput::ShotStart);
                    next_instruction = pattern_instruction;
                    continue;
                }
            } else if let Some((pattern_instruction, ())) =
                quantum_instruction(context, &mut pattern_context, instruction)?
            {
                debug!("matched quantum instruction: {:?}", instruction);
                next_instruction = pattern_instruction;
                continue;
            } else if let Some((pattern_instruction, ())) =
                rt_record_instruction(context, &mut pattern_context, instruction)?
            {
                debug!("matched rt_record instruction: {:?}", instruction);
                next_instruction = pattern_instruction;
                continue;
            } else if let Some((_, ())) =
                shot_count_loop_end(context, &mut pattern_context, instruction)?
            {
                debug!("matched shot count end: {:?}", instruction);
                pattern_context
                    .recorded_output
                    .push(RecordedOutput::ShotEnd);
                break;
            } else if instruction.get_opcode() == InstructionOpcode::Call {
                // TODO: handle callbr?
                if let Some(Either::Left(BasicValueEnum::PointerValue(pointer_value))) =
                    instruction.get_operand(0)
                {
                    let function_name = pointer_value.get_name().to_str()?;
                    if !visited_functions.contains(&function_name) {
                        if let Some(function) = context.module.get_function(function_name) {
                            let mut visited_functions = Vec::from(visited_functions);
                            visited_functions.push(function_name);
                            function_call_callback(context, function, &visited_functions)?;
                            next_instruction = instruction.get_next_instruction();
                            continue;
                        }
                    }
                }
            }

            next_instruction = instruction.get_next_instruction();
        }

        Ok(pattern_context)
    }

    /// Returns the parameters which do not have a constant value.
    pub(crate) fn get_dynamic_parameters(&self) -> Vec<&FloatValue<'ctx>> {
        self.parameters
            .iter()
            .filter(|v| !v.is_const())
            .collect::<Vec<&FloatValue>>()
    }
}

/// Match the initial instruction of a shot-count loop. This may take one of the following forms:
///
/// * a `phi` instruction with two branches, where one of the branches is the name of the current basic block
///   and the other branch sets a constant value of `1`
///
/// Example:
///
/// ```llvm
/// %116 = phi i64 [ %119, %body__1.i15.i23 ], [ 1, %body__1.i12.i19 ]
/// ```
///
/// If matched, this function returns the variable name assigned by the `phi` operand, for use in identifying the end of the loop,
/// as well as the number of shots.
pub(crate) fn shot_count_loop_start<'ctx>(
    pattern_context: &mut ShotCountPatternMatchContext<'ctx>,
    instruction: InstructionValue<'ctx>,
) -> PatternResult<'ctx, ()> {
    match instruction.get_opcode() {
        inkwell::values::InstructionOpcode::Phi => {
            // println!("Phi instruction: {:?}", instruction);
            // TODO: figure out references here to prevent this error:
            // While deleting: i64 %
            // Use still stuck around after Def is destroyed:  <badref> = phi i64 [ <badref>, %body ], [ 1, blockaddress
            // pattern_context.variable_name = Some(instruction.clone());
            pattern_context.initial_instruction = Some(instruction);
            // pattern_context.instructions_to_remove.push(instruction);

            let mut hasher = DefaultHasher::new();
            instruction.hash(&mut hasher);
            pattern_context.initial_instruction_hash = Some(hasher.finish());
            Some((instruction.get_next_instruction(), ()))
        }
        _ => None,
    }
}

/// Match the final instructions of a shot-count loop, which increment the shot count,
/// test for equality to the number of shots, and then rev
///
/// Example:
///
/// ```llvm
/// %119 = add nuw nsw i64 %116, 1
/// %120 = icmp ult i64 %116, 1000
/// br i1 %120, label %body__1.i15.i23, label %body__1.i18.i27
/// ```
///
/// These three instructions must immediately follow one another as depicted here.
///
/// If matched, this function returns the shot count.
pub(crate) fn shot_count_loop_end<'ctx>(
    context: &QCSCompilerContext,
    pattern_context: &mut ShotCountPatternMatchContext<'ctx>,
    instruction: InstructionValue<'ctx>,
) -> Result<PatternResult<'ctx, ()>> {
    match instruction.get_opcode() {
        inkwell::values::InstructionOpcode::Add => {
            // We only want to match spans starting with an add of constant 1 to the same register
            // initialized at the beginning of the block, i.e. `add nuw nsw i64 %0, 1`
            let shot_count_increment_is_1 = instruction
                .get_operand(1)
                .and_then(operand_to_integer)
                .and_then(|integer| integer_value_to_u64(context, integer))
                == Some(1);

            if shot_count_increment_is_1 {
                // Here: test that the target of the instruction is same as the shot count start variable
                if let Some(next_instruction) = instruction.get_next_instruction() {
                    if next_instruction.get_opcode() == inkwell::values::InstructionOpcode::ICmp {
                        // Here: test that the first operand is a constant; extract the shot count
                        // and save that into the pattern context
                        if let Some(Either::Left(BasicValueEnum::IntValue(_))) =
                            next_instruction.get_operand(0)
                        {
                            // Test that the first operand is the shot count variable (the initial Phi instruction)
                            let matches_shot_count_loop_start =
                                if let Some(instr) = pattern_context.initial_instruction {
                                    next_instruction
                                        .get_operand_use(0)
                                        .ok_or_else(|| eyre!("No operand use for operand 0"))?
                                        .get_used_value()
                                        .left()
                                        .ok_or_else(|| eyre!("Operand was not a basic value"))?
                                        .as_instruction_value()
                                        == Some(instr)
                                } else {
                                    false
                                };

                            if matches_shot_count_loop_start {
                                let operand = next_instruction.get_operand(1);
                                if let Some(Either::Left(BasicValueEnum::IntValue(operand_value))) =
                                    operand
                                {
                                    let shot_count = u64::try_from(
                                        operand_value
                                            .get_sign_extended_constant()
                                            .ok_or_else(|| eyre!("No constant value"))?,
                                    )
                                    .wrap_err("shot count value must be non-negative")?;

                                    if let Some(final_instruction) =
                                        next_instruction.get_next_instruction()
                                    {
                                        // Test that it's branching on the correct instruction value (the result of the comparison)
                                        let branching_on_loop_variable = next_instruction
                                            .get_first_use()
                                            == final_instruction.get_operand_use(0);

                                        if final_instruction.get_opcode() == InstructionOpcode::Br
                                            && branching_on_loop_variable
                                        {
                                            pattern_context.shot_count = Some(shot_count);

                                            if let Some(Either::Right(next_basic_block)) =
                                                final_instruction.get_operand(1)
                                            {
                                                pattern_context.next_basic_block =
                                                    Some(next_basic_block);
                                            }
                                        } else {
                                            return Err(eyre!("expected branch instruction to end shot count block, got {:?}", final_instruction));
                                        }
                                    } else {
                                        return Err(eyre!("expected a branch instruction to end shot count block, none present"));
                                    }
                                } else {
                                    return Err(eyre!(
                                        "expected integer operand, got {:?}",
                                        operand
                                    ));
                                }
                            } else {
                                return Err(eyre!(
                                    "expected\n{:?}\nto equal\n{:?}",
                                    next_instruction
                                        .get_operand_use(0)
                                        .ok_or_else(|| eyre!("No operand use for operand 0"))?
                                        .get_used_value()
                                        .left()
                                        .ok_or_else(|| eyre!("Operand was not a basic value"))?
                                        .as_instruction_value(),
                                    pattern_context.initial_instruction
                                ));
                            }
                        }
                    }

                    return Ok(Some((next_instruction.get_next_instruction(), ())));
                }
            }
            Ok(None)
        }
        _ => Ok(None),
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
    pattern_context: &mut ShotCountPatternMatchContext<'ctx>,
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
    pattern_context: &mut ShotCountPatternMatchContext<'ctx>,
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
    pattern_context: &mut ShotCountPatternMatchContext<'ctx>,
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
    pattern_context: &mut ShotCountPatternMatchContext<'ctx>,
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
                                    log::info!("Result index {} was read but was never the target of a measurement operation, so recorded output value will always be 0", result_index);
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
    pattern_context: &mut ShotCountPatternMatchContext<'ctx>,
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

                            pattern_context.quil_program.add_instruction(
                                Instruction::Measurement(
                                    Measurement {
                                        target: Some(MemoryReference {
                                            name: String::from("ro"),
                                            index: *ro_buffer_index,
                                        }),
                                        qubit: Qubit::Fixed(qubit),
                                    },
                                ),
                            );

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
