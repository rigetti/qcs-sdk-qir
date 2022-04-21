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

use std::collections::HashMap;

use eyre::Result;
use inkwell::values::BasicValueEnum;

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

        let compiler_context = Self {
            base_context: context,
            builder,
            module,
            types,
            values,
            target,
            quil_programs: vec![],
            options,
        };

        compiler_context.validate_quantum_fn_params()?;

        Ok(compiler_context)
    }

    /// Runs a validation check on the params of quantum functions in a module to ensure only
    /// `double` or `Qubit` and `Result` opaque struct pointers are used.
    pub(crate) fn validate_quantum_fn_params(&self) -> Result<()> {
        let mut bad_params = vec![];
        let mut allowed_pointer_params = vec![];
        for struct_name in ["Qubit", "Result"] {
            if let Some(struct_ty) = self.module.get_struct_type(struct_name) {
                allowed_pointer_params.push(struct_ty.ptr_type(inkwell::AddressSpace::Generic));
            }
        }

        // iterate through functions that need validation, collect functions with bad parameters
        for func in self
            .module
            .get_functions()
            .filter(|func| func.get_name().to_bytes().starts_with(b"__quantum__qis__"))
        {
            let func_name = func.get_name().to_string_lossy().to_string();
            for param in func.get_params() {
                match param {
                    inkwell::values::BasicValueEnum::FloatValue(_) => continue,
                    inkwell::values::BasicValueEnum::PointerValue(value) => {
                        if !allowed_pointer_params
                            .iter()
                            .any(|allowed| value.get_type().eq(allowed))
                        {
                            bad_params.push((func_name.clone(), param));
                        }
                    }
                    _ => bad_params.push((func_name.clone(), param)),
                }
            }
        }

        if bad_params.is_empty() {
            return Ok(());
        }

        // if we have invalid contents, present a nice error to the user with the function names and
        // invalid parameters for correction.
        let mut formatted_fn_params = String::new();

        let name_params = bad_params.iter().fold(
            HashMap::new(),
            |mut fn_params: HashMap<&str, Vec<&BasicValueEnum>>, (func_name, param)| {
                fn_params
                    .entry(func_name)
                    .and_modify(|v| v.push(param))
                    .or_insert_with(|| vec![param]);
                fn_params
            },
        );

        for (name, params) in name_params {
            formatted_fn_params.push_str(&format!(
                "\nFunction `@{}`:\n{}\n",
                name,
                params
                    .iter()
                    .map(|p| format!("- {:?}", p))
                    .collect::<Vec<String>>()
                    .join("\n")
            ));
        }

        return Err(eyre::eyre!(
            "Encountered invalid parameter{}. Quantum functions may only be parameterized with `Qubit` or `Result` pointers or `double` types.\n{}",
            if bad_params.len() > 1 { "(s)" } else { "" },
            formatted_fn_params
        ));
    }
}

#[test]
fn context_validates_fn_params() {
    let base_context = inkwell::context::Context::create();
    let data = std::fs::read("tests/fixtures/programs/bad_param_types.bc").unwrap();
    let context = QCSCompilerContext::new_from_data(
        &base_context,
        &data,
        ExecutionTarget::Qvm,
        ContextOptions {
            cache_executables: false,
            rewiring_pragma: None,
        },
    );
    assert!(context.is_err());

    let base_context = inkwell::context::Context::create();
    let data = std::fs::read("tests/fixtures/programs/parametric.bc").unwrap();
    let context = QCSCompilerContext::new_from_data(
        &base_context,
        &data,
        ExecutionTarget::Qvm,
        ContextOptions {
            cache_executables: false,
            rewiring_pragma: None,
        },
    );
    assert!(context.is_ok());
}

#[derive(Default)]
pub(crate) struct ContextOptions {
    pub(crate) cache_executables: bool,
    pub(crate) rewiring_pragma: Option<String>,
}
