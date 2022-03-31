#![deny(clippy::pedantic)]

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

use eyre::{Result, WrapErr};
use inkwell::context::Context;
use inkwell::module::Module;

use context::context::ContextOptions;
pub use context::target::ExecutionTarget;

use crate::context::QCSCompilerContext;
use crate::shot_count_block::quil::ProgramOutput;
use crate::transform::shot_count_block;

/// This module contains different functions intended for use as LLVM passes.
pub(crate) mod context;
pub(crate) mod interop;
pub(crate) mod transform;

/// Given an LLVM bitcode, replace quantum intrinsics with calls to execute equivalent Quil on Rigetti QCS
///
/// # Errors
/// 1. Returns a [`eyre::Report`] with human readable messages if the compilation fails.
pub fn patch_qir_with_qcs<'ctx>(
    options: PatchOptions,
    bitcode: &[u8],
    context: &'ctx Context,
) -> Result<Module<'ctx>> {
    let context_options = ContextOptions {
        cache_executables: options.cache_executables,
        rewiring_pragma: options.quil_rewiring_pragma,
    };

    let mut context = QCSCompilerContext::new_from_data(
        context,
        bitcode,
        options.execution_target,
        context_options,
    )?;

    shot_count_block::qir::transpile_module(&mut context).wrap_err("transformation failed")?;

    if options.add_main_entrypoint {
        crate::interop::entrypoint::add_main_entrypoint(&mut context)?;
    }
    Ok(context.module)
}

pub struct PatchOptions {
    pub add_main_entrypoint: bool,
    pub execution_target: ExecutionTarget,
    pub cache_executables: bool,
    pub quil_rewiring_pragma: Option<String>,
}

/// Transpile the given QIR bitcode into the equivalent Quil program, extracting the shot count from
/// the main program loop.
///
///
/// # Errors
/// 1. Returns a [`eyre::Report`] with human readable messages if the transpilation fails.
pub fn transpile_qir_to_quil(bitcode: &[u8]) -> Result<ProgramOutput> {
    let context = inkwell::context::Context::create();
    let mut context = QCSCompilerContext::new_from_data(
        &context,
        bitcode,
        ExecutionTarget::Qvm,
        ContextOptions::default(),
    )?;
    shot_count_block::quil::transpile_module(&mut context).wrap_err("transpilation failed")
}
