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

use eyre::Result;

use crate::interop::load::load_module_from_bitcode;

use super::{target::ExecutionTarget, types::Types, values::Values};

pub(crate) struct QCSCompilerContext<'ctx> {
    pub(crate) base_context: &'ctx inkwell::context::Context,
    pub(crate) module: inkwell::module::Module<'ctx>,
    pub(crate) builder: inkwell::builder::Builder<'ctx>,
    pub(crate) types: Types<'ctx>,
    pub(crate) values: Values<'ctx>,
    pub(crate) target: ExecutionTarget,
    pub(crate) quil_programs: Vec<quil_rs::program::Program>,
    pub(crate) options: ContextOptions,
}

impl<'ctx> QCSCompilerContext<'ctx> {
    pub(crate) fn new_from_data(
        context: &'ctx inkwell::context::Context,
        data: &[u8],
        target: ExecutionTarget,
        options: ContextOptions,
    ) -> Result<Self> {
        let builder = context.create_builder();
        let module = load_module_from_bitcode(context, data)?;
        let types = Types::new(context);
        let values = Values::new(context, &builder, &module, &types, &target)?;

        Ok(Self {
            base_context: context,
            builder,
            module,
            types,
            values,
            target,
            quil_programs: vec![],
            options,
        })
    }
}

#[derive(Default)]
pub(crate) struct ContextOptions {
    pub(crate) cache_executables: bool,
    pub(crate) rewiring_pragma: Option<String>,
}
