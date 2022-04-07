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

use eyre::{eyre, ContextCompat, Result};
use inkwell::{
    basic_block::BasicBlock,
    values::{AnyValue, FunctionValue, InstructionValue},
};
use log::{debug, info};
use quil_rs::instruction::Vector;

use crate::interop::instruction::{
    get_conditional_branch_else_target, remove_instructions_in_safe_order,
    replace_conditional_branch_target, replace_phi_clauses,
};
use crate::{context::QCSCompilerContext, interop::call, interop::entrypoint::get_entry_function};

use super::pattern::ShotCountPatternMatchContext;
use super::PARAMETER_MEMORY_REGION_NAME;

/// Build and insert an LLVM function which performs up-front translation of
/// all Quil programs used in the module and stores them in an array referred
/// to as the "executable cache".
pub(crate) fn build_populate_executable_cache_function<'ctx>(
    context: &mut QCSCompilerContext<'ctx>,
) -> Result<FunctionValue<'ctx>> {
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
        .ok_or_else(|| eyre!("create_executable_cache does not have a return value"))?
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

    Ok(populate_executable_array_function)
}

/// Mutate a context such that all contiguous instructions which may be transpiled by `transpile_instruction`
/// are inlined and executed using a shared library call.
#[allow(dead_code)]
pub(crate) fn transpile_module(context: &mut QCSCompilerContext) -> Result<()> {
    let entrypoint_function = get_entry_function(&context.module)
        .ok_or_else(|| eyre!("entrypoint not found in module"))?;
    transpile_function(context, entrypoint_function, &[])?;
    let populate_function = build_populate_executable_cache_function(context)?;

    let entry_basic_block = entrypoint_function
        .get_first_basic_block()
        .ok_or_else(|| eyre!("entrypoint function has no basic blocks"))?;

    match entry_basic_block.get_first_instruction() {
        Some(instruction) => context.builder.position_before(&instruction),
        None => context.builder.position_at_end(entry_basic_block),
    };

    context.builder.build_call(populate_function, &[], "");

    Ok(())
}

pub(crate) fn transpile_function<'ctx>(
    context: &mut QCSCompilerContext<'ctx>,
    function: FunctionValue<'ctx>,
    visited_functions: &[&str],
) -> eyre::Result<()> {
    for current_basic_block in function.get_basic_blocks() {
        transpile_basic_block(context, current_basic_block, visited_functions)?;
    }
    Ok(())
}

pub(crate) fn transpile_basic_block<'ctx>(
    context: &mut QCSCompilerContext<'ctx>,
    basic_block: BasicBlock<'ctx>,
    visited_functions: &[&str],
) -> eyre::Result<()> {
    let pattern_context = ShotCountPatternMatchContext::from_basic_block(
        context,
        basic_block,
        visited_functions,
        transpile_function,
    )?;

    insert_quil_program(context, pattern_context, basic_block)
}

/// Insert the quil program which has been collected from a `BasicBlock` (if any):
///
/// 1. Create a global variable with the program text
/// 2. Insert a shared library call to execute that program text with shot count
/// 3. Remove all of the relevant instructions from the program
#[allow(clippy::too_many_lines, clippy::unnecessary_wraps)]
pub(crate) fn insert_quil_program<'ctx, 'p: 'ctx>(
    context: &mut QCSCompilerContext<'ctx>,
    pattern_context: ShotCountPatternMatchContext<'p>,
    basic_block: BasicBlock,
) -> eyre::Result<()> {
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

        if !pattern_context.get_dynamic_parameters().is_empty() {
            program.add_instruction(quil_rs::instruction::Instruction::Declaration(
                quil_rs::instruction::Declaration {
                    name: String::from(PARAMETER_MEMORY_REGION_NAME),
                    size: Vector {
                        data_type: quil_rs::instruction::ScalarType::Real,
                        length: pattern_context.get_dynamic_parameters().len() as u64,
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
            &format!("{}_execution", basic_block.get_name().to_str()?),
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
            )?
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
            call::executable_from_quil(context, quil_program_global_string.as_pointer_value())?
        };

        call::wrap_in_shots(context, &executable, shots);

        for (index, value) in pattern_context.parameters.iter().enumerate() {
            call::set_param(context, &executable, index as u64, *value);
        }

        let execution_result = match &context.target {
            crate::context::target::ExecutionTarget::Qpu(_) => {
                call::execute_on_qpu(context, &executable)?
            }
            crate::context::target::ExecutionTarget::Qvm => {
                call::execute_on_qvm(context, &executable)?
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
                .and_then(InstructionValue::get_next_instruction)
                .ok_or_else(|| eyre!("Expected an initial instruction"))?,
        );

        let shot_index = pattern_context
            .initial_instruction
            .ok_or_else(|| eyre!("Expected an initial instruction"))?
            .as_any_value_enum()
            .into_int_value();

        for (readout_index, instruction) in &pattern_context.readout_instruction_mapping {
            let new_instruction =
                call::get_readout_bit(context, &execution_result, shot_index, *readout_index)?;

            instruction.replace_all_uses_with(
                &new_instruction
                    .as_instruction()
                    .ok_or_else(|| eyre!("Expected an instruction"))?,
            );
        }

        let cleanup_basic_block = context.base_context.insert_basic_block_after(
            basic_block,
            format!("{}_cleanup", basic_block.get_name().to_str()?).as_str(),
        );

        // Record which block was originally the target following execution & processing of shots in this block
        let original_next_block = get_conditional_branch_else_target(
            basic_block
                .get_terminator()
                .ok_or_else(|| eyre!("Expected a terminator"))?,
        )
        .wrap_err("expected the basic block to have a conditional 'else' target")?;

        context.builder.position_at_end(cleanup_basic_block);
        call::free_execution_result(context, &execution_result);
        context
            .builder
            .build_unconditional_branch(original_next_block);

        replace_conditional_branch_target(
            context,
            basic_block
                .get_terminator()
                .ok_or_else(|| eyre!("Expected a terminator"))?,
            Some(&basic_block),
            Some(&cleanup_basic_block),
        )?;

        replace_phi_clauses(
            context,
            basic_block,
            basic_block,
            execution_basic_block,
            true,
        )?;

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
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    mod can_transpile_program_with {
        use crate::context::context::{ContextOptions, QCSCompilerContext};
        use crate::context::target::ExecutionTarget;

        use super::*;

        macro_rules! make_snapshot_test {
            ($name:ident) => {
                #[test]
                fn $name() {
                    let _ = env_logger::builder().is_test(true).try_init();

                    let base_context = inkwell::context::Context::create();
                    let data =
                        std::fs::read(format!("tests/fixtures/programs/{}.bc", stringify!($name)))
                            .unwrap();
                    let mut context = QCSCompilerContext::new_from_data(
                        &base_context,
                        &data,
                        ExecutionTarget::Qvm,
                        ContextOptions {
                            cache_executables: false,
                            rewiring_pragma: None,
                        },
                    )
                    .unwrap();
                    transpile_module(&mut context).expect("transpilation failed");

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
        make_snapshot_test!(cartesian_rotations);
        make_snapshot_test!(pauli_xyz);
        make_snapshot_test!(s_and_adjoint_s);
    }
}
