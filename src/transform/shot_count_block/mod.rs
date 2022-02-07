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
// This module is responsible for the transpilation of contiguous subsequences of LLVM instructions
// into quil, substituting those instructions with inline calls to a shared library responsible for
// executing those quil instructions.
mod pattern;

use either::Either;
use inkwell::{
    basic_block::BasicBlock,
    values::{AnyValue, BasicValueEnum, FunctionValue, InstructionOpcode},
};
use log::{debug, info};
use quil_rs::instruction::Vector;

use crate::interop::instruction::{
    get_conditional_branch_else_target, remove_instructions_in_safe_order,
    replace_conditional_branch_target, replace_phi_clauses,
};

use crate::{context::QCSCompilerContext, interop::call, interop::entrypoint::get_entry_function};

use pattern::{
    quantum_instruction, shot_count_loop_end, shot_count_loop_start, ShotCountPatternMatchContext,
};

pub(crate) const PARAMETER_MEMORY_REGION_NAME: &'static str = "__qir_param";

pub(crate) fn build_populate_executable_cache_function<'ctx>(
    context: &mut QCSCompilerContext<'ctx>,
) -> FunctionValue<'ctx> {
    let populate_executable_array_function_type =
        context.base_context.void_type().fn_type(&[], false);
    let populate_executable_array_function = context.module.add_function(
        "populate_executable_array",
        populate_executable_array_function_type,
        None,
    );
    let basic_block = context
        .base_context
        .append_basic_block(populate_executable_array_function, "entry");

    context.builder.position_at_end(basic_block);

    let actual_executable_cache = context
        .builder
        .build_call(
            context.values.create_executable_cache(),
            &[context
                .base_context
                .i32_type()
                .const_int(context.quil_programs.len() as u64, false)
                .into()],
            "",
        )
        .try_as_basic_value()
        .left()
        .expect("create_executable_cache does not have a return value")
        .into_pointer_value();

    context.builder.build_store(
        context.values.executable_cache().as_pointer_value(),
        actual_executable_cache,
    );

    for index in 0..context.quil_programs.len() {
        let program_text = context.quil_programs[index].to_string(true);

        let quil_program_global_string = unsafe {
            // NOTE: this segfaults if the builder is not already positioned within a basic block
            // see https://github.com/TheDan64/inkwell/issues/32
            context
                .builder
                .build_global_string(&program_text, "quil_program")
        };

        context.builder.build_call(
            context.values.add_executable_cache_item(),
            &[
                actual_executable_cache.into(),
                context
                    .base_context
                    .i32_type()
                    .const_int(index as u64, false)
                    .into(),
                quil_program_global_string
                    .as_pointer_value()
                    .const_cast(context.types.string())
                    .into(),
            ],
            "",
        );
    }

    context.builder.build_return(None);

    populate_executable_array_function
}

/// Mutate a context such that all contiguous instructions which may be transpiled by `transpile_instruction`
/// are inlined and executed using a shared library call.
#[allow(dead_code)]
pub(crate) fn transpile_module(context: &mut QCSCompilerContext) {
    let entrypoint_function =
        get_entry_function(&context.module).expect("entrypoint not found in module");
    transpile_function(context, entrypoint_function, &vec![]);
    let populate_function = build_populate_executable_cache_function(context);

    let entry_basic_block = entrypoint_function.get_first_basic_block().unwrap();

    match entry_basic_block.get_first_instruction() {
        Some(instruction) => context.builder.position_before(&instruction),
        None => context.builder.position_at_end(entry_basic_block),
    };

    context.builder.build_call(populate_function, &[], "");
}

pub(crate) fn transpile_function<'ctx>(
    context: &mut QCSCompilerContext<'ctx>,
    function: FunctionValue<'ctx>,
    visited_functions: &[&str],
) {
    for current_basic_block in function.get_basic_blocks() {
        transpile_basic_block(context, current_basic_block, visited_functions);
    }
}

pub(crate) fn transpile_basic_block<'ctx>(
    context: &mut QCSCompilerContext<'ctx>,
    basic_block: BasicBlock<'ctx>,
    visited_functions: &[&str],
) {
    let mut next_instruction = basic_block.get_first_instruction();
    let mut pattern_context = ShotCountPatternMatchContext::default();

    info!(
        "starting transpile: block {}",
        basic_block.get_name().to_str().unwrap()
    );

    while let Some(instruction) = next_instruction {
        if instruction.get_opcode() == InstructionOpcode::Call {
            // TODO: handle callbr?
            if let Some(Either::Left(BasicValueEnum::PointerValue(pointer_value))) =
                instruction.get_operand(0)
            {
                let function_name = pointer_value.get_name().to_str().unwrap();
                if !visited_functions.contains(&function_name) {
                    if let Some(function) = context.module.get_function(function_name) {
                        let mut visited_functions = Vec::from(visited_functions);
                        visited_functions.push(function_name);
                        transpile_function(context, function, &visited_functions);
                        next_instruction = instruction.get_next_instruction();
                        continue;
                    }
                }
            }
        }

        // If we haven't yet found the loop start...
        if pattern_context.initial_instruction.is_none() {
            // Check if we've found it in this instruction. If not, continue on to the next instruction until we do find it.
            // FIXME: ensure we encounter this first (i.e. the pattern must be matched in order)
            if let Some((pattern_instruction, _)) =
                shot_count_loop_start(&mut pattern_context, instruction)
            {
                debug!("matched shot count start: {:?}", instruction);
                next_instruction = pattern_instruction;
                continue;
            }
        } else {
            if let Some((pattern_instruction, _)) =
                quantum_instruction(context, &mut pattern_context, instruction)
            {
                debug!("matched quantum instruction: {:?}", instruction);
                next_instruction = pattern_instruction;
                continue;
            } else if let Some((_, _)) =
                shot_count_loop_end(context, &mut pattern_context, instruction)
            {
                debug!("matched shot count end: {:?}", instruction);
                break;
            }
        }

        next_instruction = instruction.get_next_instruction();
    }

    insert_quil_program(context, pattern_context, basic_block);
}

/// Insert the quil program which has been collected from a BasicBlock (if any):
///
/// 1. Create a global variable with the program text
/// 2. Insert a shared library call to execute that program text with shot count
/// 3. Remove all of the relevant instructions from the program
pub(crate) fn insert_quil_program<'ctx, 'p: 'ctx>(
    context: &mut QCSCompilerContext<'ctx>,
    pattern_context: ShotCountPatternMatchContext<'p>,
    basic_block: BasicBlock,
) {
    if let Some((program, shots)) = pattern_context.get_program_data() {
        debug!(
            "inserting quil program with {} shots: {}",
            shots,
            program.to_string(true)
        );

        let mut program = program.clone();

        program.add_instruction(quil_rs::instruction::Instruction::Declaration(
            quil_rs::instruction::Declaration {
                name: String::from("ro"),
                size: Vector {
                    data_type: quil_rs::instruction::ScalarType::Bit,
                    length: pattern_context.read_result_mapping.len() as u64,
                },
                sharing: None,
            },
        ));

        if !pattern_context.parameters.is_empty() {
            program.add_instruction(quil_rs::instruction::Instruction::Declaration(
                quil_rs::instruction::Declaration {
                    name: String::from(PARAMETER_MEMORY_REGION_NAME),
                    size: Vector {
                        data_type: quil_rs::instruction::ScalarType::Real,
                        length: pattern_context.parameters.len() as u64,
                    },
                    sharing: None,
                },
            ));
        }

        if pattern_context.use_active_reset {
            // Prepend a reset to the program via copy
            let instructions = program.to_instructions(true);
            let mut new_program = quil_rs::program::Program::new();
            new_program.add_instruction(quil_rs::instruction::Instruction::Reset(
                quil_rs::instruction::Reset { qubit: None },
            ));
            for instruction in instructions {
                new_program.add_instruction(instruction);
            }
            program = new_program;
        }

        if let Some(rewiring_pragma) = &context.options.rewiring_pragma {
            // Prepend a pragma to the program via copy
            let instructions = program.to_instructions(true);
            let mut new_program = quil_rs::program::Program::new();
            new_program.add_instruction(quil_rs::instruction::Instruction::Pragma(
                quil_rs::instruction::Pragma {
                    name: String::from("INITIAL_REWIRING"),
                    arguments: vec![format!("\"{}\"", rewiring_pragma.clone())],
                    data: None,
                },
            ));
            for instruction in instructions {
                new_program.add_instruction(instruction);
            }
            program = new_program;
        }

        // We write all the new instructions to a new basic block
        let execution_basic_block = context.base_context.insert_basic_block_after(
            basic_block,
            format!("{}_execution", basic_block.get_name().to_str().unwrap()).as_str(),
        );
        basic_block.replace_all_uses_with(&execution_basic_block);
        context.builder.position_at_end(execution_basic_block);

        let executable = if context.options.cache_executables {
            let quil_program_index = context.quil_programs.len();
            context.quil_programs.push(program);

            call::get_executable(
                context,
                context
                    .base_context
                    .i32_type()
                    .const_int(quil_program_index as u64, false),
            )
        } else {
            let program_text = program.to_string(true);
            let quil_program_global_string = unsafe {
                // NOTE: this segfaults if the builder is not already positioned within a basic block
                // see https://github.com/TheDan64/inkwell/issues/32
                context
                    .builder
                    .build_global_string(&program_text, "quil_program")
            };

            // Insert the shared library calls to send this program for execution
            call::executable_from_quil(context, quil_program_global_string.as_pointer_value())
        };

        call::wrap_in_shots(context, &executable, shots);

        for (index, value) in pattern_context.parameters.iter().enumerate() {
            call::set_param(context, &executable, index as u64, *value);
        }

        let execution_result = match &context.target {
            crate::context::target::ExecutionTarget::QPU(_) => {
                call::execute_on_qpu(context, &executable)
            }
            crate::context::target::ExecutionTarget::QVM => {
                call::execute_on_qvm(context, &executable)
            }
        };

        call::panic_on_execution_result_failure(context, &execution_result);

        // After execution we branch into the reduction block, which is everything left over after the
        // quantum instructions are removed.
        context.builder.build_unconditional_branch(basic_block);

        // We place our cursor right after the beginning of the loop, which should come before any reduction instructions.
        context.builder.position_at(
            basic_block,
            &pattern_context
                .initial_instruction
                .unwrap()
                .get_next_instruction()
                .unwrap(),
        );

        let shot_index = pattern_context
            .initial_instruction
            .unwrap()
            .as_any_value_enum()
            .into_int_value();

        for (readout_index, instruction) in &pattern_context.readout_instruction_mapping {
            let new_instruction =
                call::get_readout_bit(context, &execution_result, shot_index, *readout_index);

            instruction.replace_all_uses_with(&new_instruction.as_instruction().unwrap());
        }

        let cleanup_basic_block = context.base_context.insert_basic_block_after(
            basic_block,
            format!("{}_cleanup", basic_block.get_name().to_str().unwrap()).as_str(),
        );

        // Record which block was originally the target following execution & processing of shots in this block
        let original_next_block =
            get_conditional_branch_else_target(&basic_block.get_terminator().unwrap())
                .expect("expected the basic block to have a conditional 'else' target");

        context.builder.position_at_end(cleanup_basic_block);
        call::free_execution_result(context, &execution_result);
        context
            .builder
            .build_unconditional_branch(original_next_block);

        replace_conditional_branch_target(
            context,
            &basic_block.get_terminator().unwrap(),
            Some(&basic_block),
            Some(&cleanup_basic_block),
        );

        replace_phi_clauses(
            context,
            &basic_block,
            &basic_block,
            &execution_basic_block,
            true,
        );

        remove_instructions_in_safe_order(pattern_context.instructions_to_remove);

        info!(
            "transpiled basic block {}",
            basic_block.get_name().to_string_lossy()
        );
        info!(
            "inserted basic block {}",
            execution_basic_block.get_name().to_string_lossy()
        );
    } else {
        debug!(
            "not inserting quil program, pattern context: {:?}",
            pattern_context
        );
    }
}

#[cfg(test)]
mod test {
    use super::*;

    mod can_transpile_program_with {
        use super::*;
        use crate::context::target::ExecutionTarget;
        use crate::context::QCSCompilerContext;

        macro_rules! make_snapshot_test {
            ($name:ident) => {
                #[test]
                fn $name() {
                    let _ = env_logger::builder().is_test(true).try_init();

                    let base_context = inkwell::context::Context::create();
                    let mut context = QCSCompilerContext::new_from_file(
                        &base_context,
                        "qcs",
                        format!("test/fixtures/programs/{}.bc", stringify!($name)).as_str(),
                        ExecutionTarget::QVM,
                    );
                    transpile_module(&mut context);

                    insta::assert_snapshot!(context.module.print_to_string().to_str().unwrap());
                }
            };
        }

        make_snapshot_test!(shot_count_loop);
        make_snapshot_test!(measure);
        make_snapshot_test!(measure_sparse);
        make_snapshot_test!(parametric);
        make_snapshot_test!(reduction);
        make_snapshot_test!(vqe_iteration);
    }
}
