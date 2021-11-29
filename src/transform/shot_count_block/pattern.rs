/**
 * Copyright 2021 Rigetti Computing
 *
 *    Licensed under the Apache License, Version 2.0 (the "License");
 *    you may not use this file except in compliance with the License.
 *    You may obtain a copy of the License at
 *
 *        http://www.apache.org/licenses/LICENSE-2.0
 *
 *    Unless required by applicable law or agreed to in writing, software
 *    distributed under the License is distributed on an "AS IS" BASIS,
 *    WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 *    See the License for the specific language governing permissions and
 *    limitations under the License.
 **/
// This module concerns itself with the handling of patterns in spans of instructions.
use either::Either;
use inkwell::{
    basic_block::BasicBlock,
    values::{BasicValue, BasicValueEnum, FloatValue, InstructionOpcode, InstructionValue},
};
use lazy_static::lazy_static;
use quil_rs::{
    expression::Expression,
    instruction::{GateModifier, MemoryReference, Qubit},
};
use regex::Regex;
use std::{
    collections::{hash_map::DefaultHasher, HashMap},
    convert::TryFrom,
    hash::{Hash, Hasher},
};

use crate::{
    context::QCSCompilerContext,
    interop::instruction::{
        get_called_function_name, get_qis_function_arguments, integer_value_to_u64,
        operand_to_integer, OperationArgument,
    },
};

use super::PARAMETER_MEMORY_REGION_NAME;

/// Each pattern matching function, if it finds a match, returns a result tuple including:
/// * data about the pattern which was matched.
/// * what the next instruction following the pattern is, if any.
type PatternResult<'ctx, T> = Option<(Option<InstructionValue<'ctx>>, T)>;

#[derive(Debug, Default)]
pub(crate) struct ShotCountPatternMatchContext<'ctx> {
    // The instruction used to initialize the shot count value
    // pub loop_initializer: Option<InstructionValue<'ctx>>,
    /// Hash of the instruction used to initialize the shot count value.
    /// By using the hash, we don't need to maintain a reference to the instruction itself.
    pub initial_instruction: Option<InstructionValue<'ctx>>,

    /// Hash of the instruction used to initialize the shot count value.
    /// By using the hash, we don't need to maintain a reference to the instruction itself.
    pub initial_instruction_hash: Option<u64>,

    /// The quil program transpiled from quantum intrinsics
    pub quil_program: quil_rs::Program,

    /// The shot count inferred from the loop instructions
    pub shot_count: Option<u64>,

    /// A list of instructions to remove from the program (for substitution with Quil)
    pub instructions_to_remove: Vec<InstructionValue<'ctx>>,

    /// The terminating instruction to append to the BasicBlock in place of a conditional
    /// branch on shot count
    pub next_basic_block: Option<BasicBlock<'ctx>>,

    /// Mapping of (read_result *Result index)->(ro memory region index)
    pub read_result_mapping: HashMap<u64, u64>,

    /// Pairings of (readout buffer index/offset) with the instruction which stores that readout value.
    pub readout_instruction_mapping: Vec<(u64, InstructionValue<'ctx>)>,

    /// How long the quil program's `ro` register must be to accommodate all readout indices.
    pub readout_register_length: u64,

    // All FloatValues used as instruction parameters. Indices within this Vec map to indices within the Quil
    // MemoryRegion used to read the values at runtime.
    pub parameters: Vec<FloatValue<'ctx>>,

    /// Whether or not to prepend a RESET instruction to the program to actively reset all qubits on each shot
    pub use_active_reset: bool,
}

impl<'ctx> ShotCountPatternMatchContext<'ctx> {
    pub fn get_program_data(&self) -> Option<(&quil_rs::Program, u64)> {
        if let Some(shots) = self.shot_count {
            Some((&self.quil_program, shots))
        } else {
            None
        }
    }
}

/// Match the initial instruction of a shot-count loop. This may take one of the following forms:
///
/// * a `phi` instruction with two branches, where one of the branches is the name of the current basic block
///   and the other branch sets a constant value of `1`
/// * (others TBD)
///
/// Example:
///
/// ```llvm
/// %116 = phi i64 [ %119, %body__1.i15.i23 ], [ 1, %body__1.i12.i19 ]
/// ```
///
/// If matched, this function returns the variable name assigned by the `phi` operand, for use in identifying the end of the loop,
/// as well as the number of shots
pub(crate) fn shot_count_loop_start<'a, 'ctx>(
    pattern_context: &'a mut ShotCountPatternMatchContext<'ctx>,
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
pub(crate) fn shot_count_loop_end<'a, 'ctx>(
    context: &QCSCompilerContext,
    pattern_context: &'a mut ShotCountPatternMatchContext<'ctx>,
    instruction: InstructionValue<'ctx>,
) -> PatternResult<'ctx, ()> {
    match instruction.get_opcode() {
        inkwell::values::InstructionOpcode::Add => {
            // We only want to match spans starting with an add of constant 1 to the same register
            // initialized at the beginning of the block, i.e. `add nuw nsw i64 %0, 1`
            let shot_count_increment_is_1 = instruction
                .get_operand(1)
                .and_then(|operand| operand_to_integer(operand))
                .and_then(|integer| integer_value_to_u64(context, &integer))
                .map_or(false, |int_value| int_value == 1);

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
                                pattern_context.initial_instruction.map_or(false, |instr| {
                                    next_instruction
                                        .get_operand_use(0)
                                        .unwrap()
                                        .get_used_value()
                                        .left()
                                        .unwrap()
                                        .as_instruction_value()
                                        == Some(instr)
                                });

                            if matches_shot_count_loop_start {
                                let operand = next_instruction.get_operand(1);
                                if let Some(Either::Left(BasicValueEnum::IntValue(operand_value))) =
                                    operand
                                {
                                    let shot_count = u64::try_from(
                                        operand_value.get_sign_extended_constant().unwrap(),
                                    )
                                    .expect("shot count value must be non-negative");

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
                                            panic!("expected branch instruction to end shot count block, got {:?}", final_instruction);
                                        }
                                    } else {
                                        panic!("expected a branch instruction to end shot count block, none present");
                                    }
                                } else {
                                    panic!("expected integer operand, got {:?}", operand);
                                }
                            } else {
                                panic!(
                                    "expected\n{:?}\nto equal\n{:?}",
                                    next_instruction
                                        .get_operand_use(0)
                                        .unwrap()
                                        .get_used_value()
                                        .left()
                                        .unwrap()
                                        .as_instruction_value(),
                                    pattern_context.initial_instruction
                                )
                            }
                        }
                    }

                    return Some((next_instruction.get_next_instruction(), ()));
                }
            }
            None
        }
        _ => None,
    }
}

macro_rules! match_qis_argument {
    ($variant:ident, $arguments:expr, $index:expr, $function_name:expr) => {{
        use OperationArgument::*;

        match $arguments.get($index) {
            Some($variant(contents)) => contents,
            other => {
                panic!(
                    "expected argument {} of {} to be of type {:?}; got {:?}",
                    $index,
                    $function_name,
                    stringify!($variant),
                    other
                )
            }
        }
    }};
}

/// Given a FloatValue which may be the parameter of a QIS intrinsic call, return the Quil Expression
/// which will be used to store its value at execution time.
fn get_quil_parameter_expression<'ctx>(
    pattern_context: &mut ShotCountPatternMatchContext<'ctx>,
    float_value: FloatValue<'ctx>,
) -> Expression {
    let index = get_quil_parameter_index(pattern_context, float_value);
    Expression::Address(MemoryReference {
        name: String::from(PARAMETER_MEMORY_REGION_NAME),
        index: index as u64,
    })
}

/// Given a FloatValue to be used as the parameter to a gate, return the index within the
/// Quil MemoryReference that should be used to store this parameter's value.
pub(crate) fn get_quil_parameter_index<'ctx>(
    pattern_context: &mut ShotCountPatternMatchContext<'ctx>,
    float_value: FloatValue<'ctx>,
) -> usize {
    if let Some(index) = pattern_context
        .parameters
        .iter()
        .position(|el| el == &float_value)
    {
        return index;
    } else {
        pattern_context.parameters.push(float_value);
        return pattern_context.parameters.len() - 1;
    }
}

fn add_gate_instruction<'ctx>(
    pattern_context: &mut ShotCountPatternMatchContext<'ctx>,
    arguments: &[OperationArgument<'ctx>],
    function_name: &str,
    name: &str,
    adjoint: bool,
    controlled: bool,
    parameter_count: usize,
    qubit_count: usize,
) {
    let parameters = (0..parameter_count)
        .map(|arg_index| {
            let float_value = *match_qis_argument!(Parameter, arguments, arg_index, function_name);
            let expression = get_quil_parameter_expression(pattern_context, float_value);

            expression
        })
        .collect();
    let qubits = (parameter_count..parameter_count + qubit_count)
        .map(|arg_index| {
            quil_rs::instruction::Qubit::Fixed(*match_qis_argument!(
                Qubit,
                arguments,
                arg_index,
                function_name
            ))
        })
        .collect();

    let mut modifiers = vec![];

    if adjoint {
        modifiers.push(GateModifier::Dagger)
    }

    if controlled {
        modifiers.push(GateModifier::Controlled)
    }

    let instruction = quil_rs::instruction::Instruction::Gate(quil_rs::instruction::Gate {
        name: name.to_owned(),
        parameters,
        qubits,
        modifiers,
    });

    pattern_context.quil_program.add_instruction(instruction);
}

lazy_static! {
    static ref QIS_INTRINSIC_REGEX: Regex = Regex::new(
        r"^__quantum__qis__(?P<operation>[^_]+)(?P<controlled>__ctl)?(?P<adjoint>__adj)?(__body)?$"
    )
    .unwrap();
}

pub(crate) fn quantum_instruction<'ctx>(
    context: &QCSCompilerContext<'ctx>,
    pattern_context: &mut ShotCountPatternMatchContext<'ctx>,
    instruction: InstructionValue<'ctx>,
) -> PatternResult<'ctx, ()> {
    match instruction.get_opcode() {
        inkwell::values::InstructionOpcode::Call => {
            let function_target_name = get_called_function_name(&instruction);

            if let Some(function_name) = function_target_name {
                if let Some(captures) = QIS_INTRINSIC_REGEX.captures(&function_name) {
                    let operation = &captures["operation"];
                    let adjoint = captures.name("adjoint").is_some();
                    let controlled = captures.name("controlled").is_some();

                    let arguments = get_qis_function_arguments(context, &instruction);

                    let matched = match operation {
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
                            );
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
                            );
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
                            );
                            true
                        }
                        "reset" => {
                            // TODO: Alert the user that we're resetting for all qubits instead of just the targeted qubit here
                            pattern_context.use_active_reset = true;
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
                            );
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
                            );
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
                            );
                            true
                        }
                        "mz" => {
                            let qubit =
                                *match_qis_argument!(Qubit, arguments, 0, function_name.as_str());
                            let result =
                                *match_qis_argument!(Result, arguments, 1, function_name.as_str());

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
                                quil_rs::instruction::Instruction::Measurement(
                                    quil_rs::instruction::Measurement {
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
                        Some((instruction.get_next_instruction(), ()))
                    } else {
                        None
                    }
                } else if function_name == "__quantum__qis__read_result__body" {
                    let arguments = get_qis_function_arguments(context, &instruction);
                    if let Some(OperationArgument::Result(result_index)) = arguments.get(0) {
                        let ro_index = pattern_context.read_result_mapping.get(result_index).unwrap_or_else(|| panic!("Result index {} was never the target of a measurement operation", result_index));
                        pattern_context
                            .readout_instruction_mapping
                            .push((*ro_index, instruction))
                    } else {
                        todo!("malformed read_result intrinsic")
                    }
                    pattern_context.instructions_to_remove.push(instruction);
                    Some((instruction.get_next_instruction(), ()))
                } else {
                    None
                }
            } else {
                None
            }
        }
        _ => None,
    }
}
