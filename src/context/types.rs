/**
 * Copyright 2022 Rigetti Computing
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * <http://www.apache.org/licenses/LICENSE-2.0>
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 **/
use inkwell::{
    context::Context,
    types::{PointerType, StructType},
    AddressSpace,
};

fn build_string_type(context: &Context) -> PointerType {
    context.i8_type().ptr_type(AddressSpace::Generic)
}

pub struct Types<'ctx> {
    string: PointerType<'ctx>,
    executable: StructType<'ctx>,
    executable_cache: StructType<'ctx>,
    execution_result: StructType<'ctx>,
}

impl<'ctx> Types<'ctx> {
    pub fn executable(&self) -> StructType<'ctx> {
        self.executable
    }

    pub fn executable_cache(&self) -> StructType<'ctx> {
        self.executable_cache
    }

    pub fn execution_result(&self) -> StructType<'ctx> {
        self.execution_result
    }

    pub fn new(context: &'ctx Context) -> Self {
        let execution_result = context.opaque_struct_type("ExecutionResult");
        let executable = context.opaque_struct_type("Executable");
        let executable_cache = context.opaque_struct_type("ExecutableCache");

        Self {
            string: build_string_type(context),
            executable,
            executable_cache,
            execution_result,
        }
    }

    pub fn string(&self) -> PointerType<'ctx> {
        self.string
    }
}
