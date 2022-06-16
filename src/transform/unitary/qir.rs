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

use eyre::{eyre, Result};
use inkwell::{
    basic_block::BasicBlock,
    values::{FunctionValue, InstructionOpcode},
};
use log::{debug, info};
use quil_rs::instruction::Vector;

use crate::interop::{
    call, entrypoint::get_entry_function, instruction::remove_instructions_in_safe_order,
};
use crate::{context::QCSCompilerContext, transform::PARAMETER_MEMORY_REGION_NAME};

use super::pattern::UnitaryPatternMatchContext;

/// Build and insert an LLVM function which performs up-front translation of
/// all Quil programs used in the module and stores them in an array referred
/// to as the "executable cache".
pub(crate) fn build_populate_executable_cache_function<'ctx>(
    context: &mut QCSCompilerContext<'ctx>,
) -> Result<FunctionValue<'ctx>> {
    const FN_NAME_POPULATE_EXECUTABLE_ARRAY: &str = "populate_executable_array";

    if let Some(existing_function) = context
        .module
        .get_function(FN_NAME_POPULATE_EXECUTABLE_ARRAY)
    {
        Ok(existing_function)
    } else {
        let populate_executable_array_function_type =
            context.base_context.void_type().fn_type(&[], false);
        let populate_executable_array_function = context.module.add_function(
            FN_NAME_POPULATE_EXECUTABLE_ARRAY,
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
}

/// Mutate a context such that all contiguous instructions which may be transpiled by `transpile_instruction`
/// are inlined and executed using a shared library call.
#[allow(dead_code)]
pub(crate) fn transpile_module(context: &mut QCSCompilerContext) -> Result<()> {
    let entrypoint_function = get_entry_function(&context.module)
        .ok_or_else(|| eyre!("entrypoint not found in module"))?;
    transpile_function(context, entrypoint_function)?;
    let populate_function = build_populate_executable_cache_function(context)?;

    let entry_basic_block = entrypoint_function
        .get_first_basic_block()
        .ok_or_else(|| eyre!("entrypoint function has no basic blocks"))?;

    match entry_basic_block.get_first_instruction() {
        Some(instruction) => {
            context.builder.position_before(&instruction);
        }
        None => context.builder.position_at_end(entry_basic_block),
    };

    context.builder.build_call(populate_function, &[], "");

    Ok(())
}

pub(crate) fn transpile_function<'ctx>(
    context: &mut QCSCompilerContext<'ctx>,
    function: FunctionValue<'ctx>,
) -> eyre::Result<()> {
    for current_basic_block in function.get_basic_blocks() {
        transpile_basic_block(context, current_basic_block)?;
    }
    Ok(())
}

pub(crate) fn transpile_basic_block<'ctx>(
    context: &mut QCSCompilerContext<'ctx>,
    basic_block: BasicBlock<'ctx>,
) -> eyre::Result<()> {
    let pattern_context = UnitaryPatternMatchContext::from_basic_block(context, basic_block)?;

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
    pattern_context: UnitaryPatternMatchContext<'p>,
    basic_block: BasicBlock,
) -> eyre::Result<()> {
    if let Some(program) = pattern_context.get_program_data() {
        debug!("inserting quil program: {}", program.to_string(true));

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

        let cleanup_basic_block = context.base_context.insert_basic_block_after(
            basic_block,
            format!("{}_cleanup", basic_block.get_name().to_str()?).as_str(),
        );

        context.builder.position_at_end(execution_basic_block);
        context
            .builder
            .build_unconditional_branch(cleanup_basic_block);

        context.builder.position_at_end(cleanup_basic_block);
        call::free_execution_result(context, &execution_result);
        context.builder.build_return(None);

        let entry_function = get_entry_function(&context.module)
            .ok_or_else(|| eyre::eyre!("no entry function found in module"))?;

        let entry_basic_block = entry_function
            .get_first_basic_block()
            .ok_or_else(|| eyre::eyre!("no basic block found in entry function"))?;

        let last_entry_block_instruction = entry_basic_block
            .get_last_instruction()
            .ok_or_else(|| eyre::eyre!("no instructions in entry basic block"))?;

        context
            .builder
            .position_before(&last_entry_block_instruction);

        context
            .builder
            .build_unconditional_branch(execution_basic_block);

        if last_entry_block_instruction.get_opcode() == InstructionOpcode::Return {
            last_entry_block_instruction.remove_from_basic_block();
        }

        remove_instructions_in_safe_order(pattern_context.instructions_to_remove)?;

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
                    let data = std::fs::read(format!(
                        "tests/fixtures/programs/unitary/{}.bc",
                        stringify!($name)
                    ))
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

        make_snapshot_test!(bell_state);
        make_snapshot_test!(entrypoint_attribute);
        make_snapshot_test!(qiskit_qir_measure);
        make_snapshot_test!(qiskit_qir_allow_unmeasured);
    }
}
