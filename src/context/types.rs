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
use inkwell::{
    context::Context,
    types::{PointerType, StructType},
    AddressSpace,
};

fn build_string_type<'ctx>(context: &'ctx Context) -> PointerType<'ctx> {
    context.i8_type().ptr_type(AddressSpace::Generic)
}

pub struct Types<'ctx> {
    string: PointerType<'ctx>,
    executable: StructType<'ctx>,
    execution_result: StructType<'ctx>,
}

impl<'ctx> Types<'ctx> {
    pub fn new(context: &'ctx Context) -> Self {
        let execution_result = context.opaque_struct_type("ExecutionResult");
        let executable = context.opaque_struct_type("Executable");

        Self {
            string: build_string_type(context),
            executable,
            execution_result,
        }
    }

    pub fn executable(&self) -> StructType<'ctx> {
        self.executable
    }

    pub fn execution_result(&self) -> StructType<'ctx> {
        self.execution_result
    }

    pub fn string(&self) -> PointerType<'ctx> {
        self.string
    }
}
