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
use inkwell::{basic_block::BasicBlock, values::FunctionValue};
use quil_rs::instruction::Vector;

#[cfg(feature = "serde_support")]
use serde::{ser::SerializeStruct, Serialize, Serializer};

use crate::transform::PARAMETER_MEMORY_REGION_NAME;
use crate::{context::QCSCompilerContext, interop::entrypoint::get_entry_function, RecordedOutput};

use super::pattern::ShotCountPatternMatchContext;

/// Encapsulates the result of transpiling a QIR module to a Quil program
#[derive(Debug)]
pub struct ProgramOutput {
    /// The Quil program itself
    pub program: quil_rs::Program,
    /// The number of shots to run the program for, extracted from the primary execution loop
    pub shot_count: u64,
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
        output.serialize_field("shot_count", &self.shot_count)?;
        output.serialize_field("recorded_output", &self.recorded_output)?;
        output.end()
    }
}

/// Transform an entire QIR module to a single Quil program with shot count inferred
/// from a program loop counter.
#[allow(dead_code)]
pub(crate) fn transpile_module(context: &mut QCSCompilerContext) -> Result<ProgramOutput> {
    let entrypoint_function = get_entry_function(&context.module)
        .ok_or_else(|| eyre!("entrypoint not found in module"))?;
    transpile_function(context, entrypoint_function, &[])
}

/// Transpile a single QIR function body to a Quil program. This function may have any number
/// of basic blocks, but only the block named `body` will be parsed for quantum instructions
/// and transpiled to Quil; others will be ignored.
pub(crate) fn transpile_function<'ctx>(
    context: &mut QCSCompilerContext<'ctx>,
    function: FunctionValue<'ctx>,
    visited_functions: &[&str],
) -> eyre::Result<ProgramOutput> {
    let blocks = function.get_basic_blocks();
    let body_block = blocks
        .into_iter()
        .find(|el| el.get_name().to_string_lossy() == "body")
        .ok_or(eyre::eyre!("no basic block named 'body' found in function"))?;

    transpile_basic_block(context, body_block, visited_functions)
}

/// Transpile a single QIR basic block to a Quil program. This block must match the pattern
/// recognized by `ShotCountPatternMatchContext` in order to succeed.
pub(crate) fn transpile_basic_block<'ctx>(
    context: &mut QCSCompilerContext<'ctx>,
    basic_block: BasicBlock<'ctx>,
    visited_functions: &[&str],
) -> eyre::Result<ProgramOutput> {
    let pattern_context = ShotCountPatternMatchContext::from_basic_block(
        context,
        basic_block,
        visited_functions,
        fail_on_nested_function_call,
    )?;

    build_quil_program(context, &pattern_context)
}

/// This function exists to satisfy the signature needed to form a `ShotCountPatternMatchContext`.
///
/// However, because Quil does not itself have function calls, if there is a function call within
/// the basic block being transpiled, this returns an error.
pub(crate) fn fail_on_nested_function_call<'ctx>(
    _context: &mut QCSCompilerContext<'ctx>,
    function: FunctionValue<'ctx>,
    _visited_functions: &[&str],
) -> eyre::Result<()> {
    Err(eyre::eyre!(
        "cannot transpile nested function calls to Quil; found \"{}\"",
        function.get_name().to_string_lossy()
    ))
}

/// Build a Quil program from the information scraped into a shot-count pattern match.
/// If no pattern was detected, return an error.
pub(crate) fn build_quil_program<'ctx, 'p: 'ctx>(
    context: &mut QCSCompilerContext<'ctx>,
    pattern_context: &ShotCountPatternMatchContext<'p>,
) -> eyre::Result<ProgramOutput> {
    if let Some((program, shots)) = pattern_context.get_program_data() {
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

        Ok(ProgramOutput {
            program,
            shot_count: shots,
            recorded_output: pattern_context.recorded_output.clone(),
        })
    } else {
        Err(eyre::eyre!(
            "the shot count pattern was not detected in the program's basic block"
        ))
    }
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
                    let result = transpile_module(&mut context).expect("transpilation failed");

                    insta::assert_snapshot!(result.program.to_string(true));
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
        make_snapshot_test!(t_and_adjoint_t);
        make_snapshot_test!(toffoli);
        make_snapshot_test!(swap);
        make_snapshot_test!(entrypoint_attribute);
    }
}
