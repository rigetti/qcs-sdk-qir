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

// This module is responsible for the transpilation of contiguous subsequences of LLVM instructions
// into quil, substituting those instructions with inline calls to a shared library responsible for
// executing those quil instructions.
use eyre::{eyre, Result};

#[allow(unused)]
use inkwell::types::AnyType;

use inkwell::{basic_block::BasicBlock, values::FunctionValue};
use quil_rs::instruction::Vector;

#[cfg(feature = "serde_support")]
use serde::{ser::SerializeStruct, Serialize, Serializer};

use crate::{
    context::QCSCompilerContext, interop::entrypoint::get_entry_function,
    transform::PARAMETER_MEMORY_REGION_NAME, RecordedOutput,
};

use super::pattern::UnitaryPatternMatchContext;

/// Encapsulates the result of transpiling a QIR module to a Quil program
#[derive(Debug)]
pub struct ProgramOutput {
    /// The Quil program itself
    pub program: quil_rs::Program,

    /// Signifies output to be recorded at the end of program execution
    pub recorded_output: Vec<RecordedOutput>,
}

#[cfg(feature = "serde_support")]
impl Serialize for ProgramOutput {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut output = serializer.serialize_struct("ProgramOutput", 3)?;
        output.serialize_field("program", &self.program.to_string(true))?;
        output.serialize_field("recorded_output", &self.recorded_output)?;
        output.end()
    }
}

/// Transform an entire QIR module to a single Quil program
#[allow(dead_code)]
pub(crate) fn transpile_module(context: &mut QCSCompilerContext) -> Result<ProgramOutput> {
    let entrypoint_function = get_entry_function(&context.module)
        .ok_or_else(|| eyre!("entrypoint not found in module"))?;
    transpile_function(context, entrypoint_function)
}

/// Transpile a single QIR function body to a Quil program. This function may have a single basic
/// block, comprised of quantum instructions.
pub(crate) fn transpile_function<'ctx>(
    context: &mut QCSCompilerContext<'ctx>,
    function: FunctionValue<'ctx>,
) -> eyre::Result<ProgramOutput> {
    // Validate that the function is a nullary function that returns i64 (standards-compliant) or
    // void (old behavior) , as a requirement of the Unitary format
    let func_ty = function.get_type();
    if !(func_ty.count_param_types() == 0
        && !func_ty.is_var_arg()
        && func_ty
            .get_return_type()
            .is_none_or(|ret_ty| ret_ty == context.base_context.i64_type().into()))
    {
        return Err(eyre::eyre!(
            "expected function to return i64 or (as a legacy extension) void; found {}",
            func_ty.print_to_string()
        ));
    }

    let blocks = function.get_basic_blocks();
    let block_count = blocks.len();

    match blocks.into_iter().next() {
        Some(first_block) if block_count == 1 => transpile_basic_block(context, first_block),
        _ => Err(eyre::eyre!(
            "expected function to have a single basic block; found {}",
            block_count
        )),
    }
}

#[test]
fn validates_unitary_ret_void() {
    use crate::transform::unitary::quil::transpile_module;

    let context = inkwell::context::Context::create();
    let path = "tests/fixtures/programs/unitary/non_void_terminator.bc";
    let data = std::fs::read(path).unwrap();
    let mut context = QCSCompilerContext::new_from_data(
        &context,
        &data,
        crate::ExecutionTarget::Qvm,
        Default::default(),
    )
    .unwrap();

    assert!(transpile_module(&mut context).is_err());

    let context = inkwell::context::Context::create();
    let path = "tests/fixtures/programs/unitary/bell_state.bc";
    let data = std::fs::read(path).unwrap();
    let mut context = QCSCompilerContext::new_from_data(
        &context,
        &data,
        crate::ExecutionTarget::Qvm,
        Default::default(),
    )
    .unwrap();

    assert!(transpile_module(&mut context).is_ok());
}

/// Transpile a single QIR basic block to a Quil program. This block must match the pattern
/// recognized by `UnitaryPatternMatchContext` in order to succeed.
pub(crate) fn transpile_basic_block<'ctx>(
    context: &mut QCSCompilerContext<'ctx>,
    basic_block: BasicBlock<'ctx>,
) -> eyre::Result<ProgramOutput> {
    let pattern_context = UnitaryPatternMatchContext::from_basic_block(context, basic_block)?;

    build_quil_program(context, &pattern_context)
}

/// Build a Quil program from the information scraped into a unitary pattern match.
/// If no pattern was detected, return an error.
#[allow(clippy::unnecessary_wraps)]
pub(crate) fn build_quil_program<'ctx, 'p: 'ctx>(
    context: &mut QCSCompilerContext<'ctx>,
    pattern_context: &UnitaryPatternMatchContext<'p>,
) -> eyre::Result<ProgramOutput> {
    let mut program = pattern_context.quil_program.clone();

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

    Ok(ProgramOutput {
        program,
        recorded_output: pattern_context.recorded_output.clone(),
    })
}

#[cfg(test)]
mod test {
    use super::*;

    mod can_transpile_program_with {
        use super::*;
        use crate::context::context::{ContextOptions, QCSCompilerContext};
        use crate::context::target::ExecutionTarget;

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
                    let result = transpile_module(&mut context).expect("transpilation failed");

                    insta::assert_snapshot!(result.program.to_string(true));
                }
            };
        }

        make_snapshot_test!(bell_state);
        make_snapshot_test!(entrypoint_attribute);
    }
}
