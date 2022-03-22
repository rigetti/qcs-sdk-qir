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

use inkwell::module::Module;
use inkwell::values::FunctionValue;

use crate::context::QCSCompilerContext;

pub(crate) fn get_entry_function<'ctx>(module: &Module<'ctx>) -> Option<FunctionValue<'ctx>> {
    let ns = "QuantumApplication";
    let method = "Run";
    let entrypoint_name = format!("{}__{}__body", ns, method);
    module
        .get_function(&entrypoint_name)
        .or_else(|| get_alternate_entry_function(module))
}

// TODO: temporary, replace with by-attribute lookup of the entrypoint. This is hardcoded to work with the provided VQE examples.
pub(crate) fn get_alternate_entry_function<'ctx>(
    module: &Module<'ctx>,
) -> Option<FunctionValue<'ctx>> {
    module.get_function("Microsoft__Quantum__Samples__RunMain__Interop")
}

/// Mutate the context to add a `main` function as an entrypoint for `x86_64`, which
/// itself calls the QIR standard entrypoint.
pub(crate) fn add_main_entrypoint(context: &mut QCSCompilerContext) {
    let main_function = context.module.add_function(
        "main",
        context.base_context.i32_type().fn_type(&[], false),
        None,
    );
    let entry = context
        .base_context
        .append_basic_block(main_function, "entry");
    context.builder.position_at_end(entry);

    let qir_entrypoint =
        get_entry_function(&context.module).expect("QIR expected entrypoint not found");

    context.builder.build_call(qir_entrypoint, &[], "");
    context
        .builder
        .build_return(Some(&context.base_context.i32_type().const_int(0, false)));
}
