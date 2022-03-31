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
use inkwell::memory_buffer::MemoryBuffer;
use inkwell::module::Module;

// Given a file path to an LLVM bitcode file, load its contents into an `inkwell::Module`.
pub(crate) fn load_module_from_bitcode<'ctx>(
    context: &'ctx inkwell::context::Context,
    data: &[u8],
) -> Result<Module<'ctx>> {
    let buffer = MemoryBuffer::create_from_memory_range_copy(data, "qcs");
    Module::parse_bitcode_from_buffer(&buffer, context)
        .map_err(|e| eyre!(e.to_string()).wrap_err("failed to parse bitcode"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_load_bitcode_file() {
        let path = "tests/fixtures/programs/module.bc";
        let data = std::fs::read(path).unwrap();
        let context = inkwell::context::Context::create();
        load_module_from_bitcode(&context, &data).unwrap();
    }
}
