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
use inkwell::memory_buffer::MemoryBuffer;
use inkwell::module::Module;

// Given a file path to an LLVM bitcode file, load its contents into an `inkwell::Module`.
pub(crate) fn load_module_from_bitcode_file<'ctx>(
    context: &'ctx inkwell::context::Context,
    name: &'ctx str,
    file_path: &str,
) -> Module<'ctx> {
    let data = std::fs::read(file_path).expect("unable to read from specified file path");
    let buffer = MemoryBuffer::create_from_memory_range_copy(&data, name);
    let module = Module::parse_bitcode_from_buffer(&buffer, context).unwrap();
    module
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_load_bitcode_file() {
        let path = "src/passes/data/module.bc";
        let context = inkwell::context::Context::create();
        load_module_from_bitcode_file(&context, "test-module", path);
    }
}
