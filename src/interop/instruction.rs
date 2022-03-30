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

use std::convert::{TryFrom, TryInto};

// Functions which operate on and extract information from `inkwell` `InstructionValue`s
use either::Either;
use eyre::{eyre, Result, WrapErr};
use inkwell::{
    basic_block::BasicBlock,
    types::AnyTypeEnum,
    values::{
        BasicValue, BasicValueEnum, FloatValue, InstructionOpcode, InstructionValue, IntValue,
        PhiValue, PointerValue,
    },
};

use crate::context::QCSCompilerContext;

pub(crate) fn get_called_function_name(instruction: InstructionValue) -> Result<Option<String>> {
    let intrinsic_function_target = instruction
        .get_operand(instruction.get_num_operands() - 1)
        .ok_or_else(|| eyre!("expected a final operand in Call instruction"))?;

    match intrinsic_function_target {
        Either::Left(BasicValueEnum::PointerValue(ptr_value)) => Ok(ptr_value
            .get_name()
            .to_str()
            .ok()
            .map(std::borrow::ToOwned::to_owned)),
        _ => Err(eyre!(
            "BasicBlock target of function call is not yet implemented"
        )),
    }
}

#[derive(Debug)]
pub(crate) enum OperationArgument<'ctx> {
    Qubit(u64),
    Result(u64),
    Parameter(FloatValue<'ctx>),
    Instruction(InstructionValue<'ctx>),
}

/// Return the arguments used to invoke a quantum runtime intrinsic, `@__quantum__qis__*__body`, in order.
pub(crate) fn get_qis_function_arguments<'ctx>(
    context: &QCSCompilerContext,
    instruction: InstructionValue<'ctx>,
) -> Result<Vec<OperationArgument<'ctx>>> {
    let operand_count = instruction.get_num_operands();

    // The final operand of a call instruction is the function being called
    (0..operand_count - 1)
        .map(|operand_index| {
            let target = instruction
                .get_operand(operand_index)
                .ok_or_else(|| eyre!("expected a first operand in Call instruction"))?;
            if let Either::Left(BasicValueEnum::PointerValue(ptr_value)) = target {
                if let AnyTypeEnum::StructType(struct_type) =
                    ptr_value.get_type().get_element_type()
                {
                    let type_name = struct_type
                        .get_name()
                        .ok_or_else(|| eyre!("expected struct type to have name"))?;
                    match type_name
                        .to_str()
                        .wrap_err("unable to convert C String to string")?
                    {
                        "Qubit" => {
                            let qubit_index =
                                pointer_value_to_u64(context, ptr_value).ok_or_else(|| {
                                    eyre!("qubit index must be a non-negative number")
                                })?;
                            Ok(OperationArgument::Qubit(qubit_index))
                        }
                        "Result" => {
                            let result_index = pointer_value_to_u64(context, ptr_value)
                                .ok_or_else(|| {
                                    eyre!("unable to derive Result index from pointer")
                                })?;
                            Ok(OperationArgument::Result(result_index))
                        }
                        other => Err(eyre!(
                            "got unexpected type {} as argument to {:?}",
                            other,
                            instruction
                        )),
                    }
                } else if let Some(inst) = ptr_value.as_instruction() {
                    Ok(OperationArgument::Instruction(inst))
                } else {
                    Err(eyre!(
                        "unexpected pointer value {:?} as operand {} of instruction {:?}",
                        ptr_value,
                        operand_index,
                        instruction
                    ))
                }
            } else if let Either::Left(BasicValueEnum::FloatValue(value)) = target {
                Ok(OperationArgument::Parameter(value))
            } else {
                Err(eyre!(
                    "non-pointer/float function argument in {:?}",
                    instruction
                ))
            }
        })
        .collect()
}

/// Attempt to extract an integer value from an operand, and return that integer value if successful
pub(crate) fn operand_to_integer<'ctx>(
    operand: Either<BasicValueEnum<'ctx>, BasicBlock<'ctx>>,
) -> Option<IntValue<'ctx>> {
    if let Either::Left(BasicValueEnum::IntValue(int_value)) = operand {
        Some(int_value)
    } else {
        None
    }
}

/// Attempt to cast a pointer to an immediate int and return that value if successful
pub(crate) fn pointer_value_to_u64(
    context: &QCSCompilerContext,
    value: PointerValue,
) -> Option<u64> {
    value
        .const_to_int(context.base_context.i64_type())
        .get_sign_extended_constant()
        .and_then(|value| u64::try_from(value).ok())
}

/// Attempt to cast a pointer to an immediate int and return that value if successful
pub(crate) fn integer_value_to_u64(_context: &QCSCompilerContext, value: IntValue) -> Option<u64> {
    value
        .get_sign_extended_constant()
        .and_then(|value| u64::try_from(value).ok())
}

pub(crate) fn get_conditional_branch_else_target(
    instruction: InstructionValue,
) -> Option<BasicBlock> {
    if let Some(Either::Right(target)) = instruction.get_operand(1) {
        Some(target)
    } else {
        None
    }
}

/// Given a conditional branch (`br`) instruction, replace its then and/or else targets with the specified basic blocks.
///
/// Note: this function moves the builder's position and does not restore it.
pub(crate) fn replace_conditional_branch_target(
    context: &mut QCSCompilerContext,
    instruction: InstructionValue,
    replace_then: Option<&BasicBlock>,
    replace_else: Option<&BasicBlock>,
) -> Result<()> {
    context.builder.position_at(
        instruction
            .get_parent()
            .ok_or_else(|| eyre!("Expected instruction to have a parent"))?,
        &instruction,
    );

    let original_then_block = if let Some(Either::Right(target)) = instruction.get_operand(2) {
        target
    } else {
        return Err(eyre!("expected basic block target for branch"));
    };

    let original_else_block = if let Some(Either::Right(target)) = instruction.get_operand(1) {
        target
    } else {
        return Err(eyre!("expected basic block target for branch"));
    };

    let (then_block, else_block) = (
        replace_then.unwrap_or(&original_then_block),
        replace_else.unwrap_or(&original_else_block),
    );

    let comparison = if let Some(Either::Left(BasicValueEnum::IntValue(comparison))) =
        instruction.get_operand(0)
    {
        comparison
    } else {
        return Err(eyre!("expected integer comparison for branch"));
    };

    let new_instruction =
        context
            .builder
            .build_conditional_branch(comparison, *then_block, *else_block);
    instruction.replace_all_uses_with(&new_instruction);
    instruction.remove_from_basic_block();
    Ok(())
}

/// Given a `phi` instruction, replace the existing matching block with the new one specified.
///
/// Parameters:
/// * `reverse_match`: whether to match all incoming clauses that _aren't_ from the specified original basic block instead
///   of those that _are_.
pub(crate) fn replace_phi_clause(
    context: &mut QCSCompilerContext,
    instruction: PhiValue,
    old_basic_block: BasicBlock,
    new_basic_block: BasicBlock,
    reverse_match: bool,
) -> Result<()> {
    let basic_block_parent = instruction
        .as_instruction()
        .get_parent()
        .ok_or_else(|| eyre!("Expected instruction to have a parent"))?;

    // We have to ensure that we're writing all phi instructions at the start of the basic block;
    // in LLVM IR no non-phi instructions may precede any phi instruction in the block.
    context.builder.position_before(
        &basic_block_parent
            .get_first_instruction()
            .ok_or_else(|| eyre!("Expected basic block to have at least one instruction"))?,
    );

    let mut new_incoming: Vec<(BasicValueEnum, BasicBlock)> = vec![];

    // FromIterator not implemented
    // let new_incoming = (0..instruction.count_incoming()).map(|index| {
    // let value = instruction.get_incoming(index).unwrap();

    // TODO: Rework this so it's less clumsy.
    // The trick is that get_incoming gives you an owned value but add_incoming wants a &dyn,
    // so you need to own the values somewhere long enough to be able to supply them to `add_incoming`.
    for index in 0..instruction.count_incoming() {
        let value = instruction
            .get_incoming(index)
            .ok_or_else(|| eyre!("Expected phi instruction to have incoming values"))?;

        if reverse_match ^ (value.1 == old_basic_block) {
            new_incoming.push((value.0, new_basic_block));
        } else {
            new_incoming.push((value.0, value.1));
        }
    }

    let mut new_incoming_ref: Vec<(&dyn BasicValue, BasicBlock)> = vec![];

    for element in &new_incoming {
        new_incoming_ref.push((&element.0, element.1));
    }

    // TODO: derive the type from the actual instruction instead of assuming i64
    let new_instruction = context
        .builder
        .build_phi(context.base_context.i64_type(), "");
    new_instruction.add_incoming(new_incoming_ref.as_slice());

    instruction.replace_all_uses_with(&new_instruction);
    instruction.as_instruction().remove_from_basic_block();
    Ok(())
}

/// Print each of the operands of an instruction in debug format to stdout on its own labeled line.
#[allow(dead_code)]
pub(crate) fn print_all_operands(instruction: InstructionValue) {
    println!("instruction: {:?}", instruction);

    for i in 0..instruction.get_num_operands() {
        println!("operand {}: {:?}", i, instruction.get_operand(i));
    }
}

pub(crate) fn replace_phi_clauses(
    context: &mut QCSCompilerContext,
    within_basic_block: BasicBlock,
    old_basic_block: BasicBlock,
    new_basic_block: BasicBlock,
    reverse_match: bool,
) -> Result<()> {
    let mut instruction = within_basic_block.get_first_instruction();

    while let Some(current_instruction) = instruction {
        // We have to get a valid handle on the next instruction before replace_phi_clause deletes this one.
        let next_instruction = current_instruction.get_next_instruction();
        if current_instruction.get_opcode() == InstructionOpcode::Phi {
            replace_phi_clause(
                context,
                current_instruction
                    .try_into()
                    .map_err(|_| eyre!("Expected phi instruction"))?,
                old_basic_block,
                new_basic_block,
                reverse_match,
            )?;
        }
        instruction = next_instruction;
    }
    Ok(())
}

/// Remove instructions in topological order such that none is removed while any other instruction uses it.
/// Note that this will panic if there is a cycle in the use graph or
pub(crate) fn remove_instructions_in_safe_order(instructions: Vec<InstructionValue>) {
    let mut instructions = instructions;

    loop {
        let mut instructions_removed_in_round = false;

        instructions.retain(|instr| {
            if instr.get_first_use().is_some() {
                true
            } else {
                instr.remove_from_basic_block();
                instructions_removed_in_round = true;
                false
            }
        });

        if instructions.is_empty() {
            return;
        }

        assert!(
            instructions_removed_in_round,
            "Unable to remove remaining instructions safely"
        );
    }
}
