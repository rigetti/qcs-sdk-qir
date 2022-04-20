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
use inkwell::attributes::AttributeLoc;
use inkwell::module::Module;
use inkwell::values::FunctionValue;

use crate::context::QCSCompilerContext;

/// First, check for a function with an attribute value of "`EntryPoint`". This indicates the starting
/// point for the program. If no such function exists, look for one with the default name,
/// "`QuantumApplication__Run__body`".
pub(crate) fn get_entry_function<'ctx>(module: &Module<'ctx>) -> Option<FunctionValue<'ctx>> {
    let ns = "QuantumApplication";
    let method = "Run";
    let entrypoint_name = format!("{}__{}__body", ns, method);
    get_entrypoint_function(module).or_else(|| module.get_function(&entrypoint_name))
}

/// By-attribute lookup of the entrypoint function in a given module. High-level languages may add
/// an attribute to a function, informing compilers of a module's entry point. This attribute will
/// have the value, "`EntryPoint`".
pub(crate) fn get_entrypoint_function<'ctx>(module: &Module<'ctx>) -> Option<FunctionValue<'ctx>> {
    module
        .get_functions()
        .filter(|f| f.count_attributes(AttributeLoc::Function) > 0)
        .find(|f| {
            f.get_string_attribute(AttributeLoc::Function, "EntryPoint")
                .is_some()
        })
}

/// Mutate the context to add a `main` function as an entrypoint for `x86_64`, which
/// itself calls the QIR standard entrypoint.
pub(crate) fn add_main_entrypoint(context: &mut QCSCompilerContext) -> Result<()> {
    let main_function = context.module.add_function(
        "main",
        context.base_context.i32_type().fn_type(&[], false),
        None,
    );
    let entry = context
        .base_context
        .append_basic_block(main_function, "entry");
    context.builder.position_at_end(entry);

    let qir_entrypoint = get_entry_function(&context.module)
        .ok_or_else(|| eyre!("QIR expected entrypoint not found"))?;

    context.builder.build_call(qir_entrypoint, &[], "");
    context
        .builder
        .build_return(Some(&context.base_context.i32_type().const_int(0, false)));
    Ok(())
}

#[test]
fn test_entrypoint_attribute() {
    let path = "tests/fixtures/programs/entrypoint_attribute.bc";
    let data = std::fs::read(path).unwrap();
    let context = inkwell::context::Context::create();
    let module = super::load::load_module_from_bitcode(&context, &data).unwrap();
    let function = get_entrypoint_function(&module);

    assert!(function.is_some());
    assert_eq!(b"some_function", function.unwrap().get_name().to_bytes());
}
