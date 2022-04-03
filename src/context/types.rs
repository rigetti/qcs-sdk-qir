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

use inkwell::{
    context::Context,
    module::Module,
    types::{PointerType, StructType},
    AddressSpace,
};

fn build_string_type(context: &Context) -> PointerType {
    context.i8_type().ptr_type(AddressSpace::Generic)
}

const TYPE_NAME_EXECUTION_RESULT: &str = "ExecutionResult";
const TYPE_NAME_EXECUTABLE: &str = "Executable";
const TYPE_NAME_EXECUTABLE_CACHE: &str = "ExecutableCache";

pub(crate) struct Types<'ctx> {
    string: PointerType<'ctx>,
    executable: StructType<'ctx>,
    executable_cache: StructType<'ctx>,
    execution_result: StructType<'ctx>,
}

impl<'ctx> Types<'ctx> {
    pub(crate) fn executable(&self, module: &Module<'ctx>) -> StructType<'ctx> {
        match module.get_struct_type(TYPE_NAME_EXECUTABLE) {
            Some(s) => s,
            None => self.executable,
        }
    }

    pub(crate) fn executable_cache(&self, module: &Module<'ctx>) -> StructType<'ctx> {
        match module.get_struct_type(TYPE_NAME_EXECUTABLE_CACHE) {
            Some(s) => s,
            None => self.executable_cache,
        }
    }

    pub(crate) fn execution_result(&self, module: &Module<'ctx>) -> StructType<'ctx> {
        match module.get_struct_type(TYPE_NAME_EXECUTION_RESULT) {
            Some(s) => s,
            None => self.execution_result,
        }
    }

    pub(crate) fn new(context: &'ctx Context) -> Self {
        let execution_result = context.opaque_struct_type(TYPE_NAME_EXECUTION_RESULT);
        let executable = context.opaque_struct_type(TYPE_NAME_EXECUTABLE);
        let executable_cache = context.opaque_struct_type(TYPE_NAME_EXECUTABLE_CACHE);

        Self {
            string: build_string_type(context),
            executable,
            executable_cache,
            execution_result,
        }
    }

    pub(crate) fn string(&self) -> PointerType<'ctx> {
        self.string
    }
}
