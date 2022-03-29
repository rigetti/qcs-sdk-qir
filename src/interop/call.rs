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

use eyre::{eyre, ContextCompat, Result};
use inkwell::{
    module::Linkage,
    types::BasicMetadataTypeEnum,
    values::{BasicMetadataValueEnum, FloatValue, IntValue, PointerValue},
};

use crate::context::QCSCompilerContext;

#[allow(dead_code)]
pub(crate) fn printf<'ctx>(context: &mut QCSCompilerContext<'ctx>, string: PointerValue) {
    let string_type = context.types.string();
    let printf_type = context
        .base_context
        .i32_type()
        .fn_type(&[BasicMetadataTypeEnum::PointerType(string_type)], true);
    let printf = context
        .module
        .add_function("printf", printf_type, Some(Linkage::External));
    context.builder.build_call(
        printf,
        &[BasicMetadataValueEnum::PointerValue(
            string.const_cast(string_type),
        )],
        "",
    );
}

pub(crate) struct Executable<'ctx>(pub(crate) PointerValue<'ctx>);

#[allow(dead_code)]
pub(crate) fn executable_from_quil<'ctx>(
    context: &mut QCSCompilerContext<'ctx>,
    quil: PointerValue<'ctx>,
) -> Executable<'ctx> {
    let string_type = context.types.string();
    let executable_call_site_value = context.builder.build_call(
        context.values.executable_from_quil_function(),
        &[BasicMetadataValueEnum::PointerValue(
            quil.const_cast(string_type),
        )],
        "",
    );
    Executable(
        executable_call_site_value
            .try_as_basic_value()
            .left()
            .ok_or_else(|| eyre!("expected basic value"))
            .into_pointer_value(),
    )
}

pub(crate) struct ExecutionResult<'ctx>(PointerValue<'ctx>);

pub(crate) fn execute_on_qpu<'ctx>(
    context: &mut QCSCompilerContext<'ctx>,
    executable: &Executable<'ctx>,
) -> Result<ExecutionResult<'ctx>> {
    let execution_result = context.builder.build_call(
        context.values.execute_on_qpu_function(),
        &[
            executable.0.into(),
            context
                .values
                .quantum_processor_id()
                .ok_or_else(|| eyre!("expected a quantum processor ID to be provided"))?
                .into(),
        ],
        "",
    );

    Ok(ExecutionResult(
        execution_result
            .try_as_basic_value()
            .left()
            .ok_or_else(|| eyre!("Expected a basic value"))?
            .into_pointer_value(),
    ))
}

pub(crate) fn execute_on_qvm<'ctx>(
    context: &mut QCSCompilerContext<'ctx>,
    executable: &Executable<'ctx>,
) -> Result<ExecutionResult<'ctx>> {
    let execution_result = context.builder.build_call(
        context.values.execute_on_qvm_function(),
        &[executable.0.into()],
        "",
    );

    Ok(ExecutionResult(
        execution_result
            .try_as_basic_value()
            .left()
            .ok_or_else(|| eyre!("Expected a basic value"))?
            .into_pointer_value(),
    ))
}

#[allow(dead_code)]
pub(crate) fn free_executable<'ctx>(
    context: &mut QCSCompilerContext<'ctx>,
    executable: &Executable<'ctx>,
) {
    context.builder.build_call(
        context.values.free_executable_function(),
        &[executable.0.into()],
        "",
    );
}

pub(crate) fn free_execution_result<'ctx>(
    context: &mut QCSCompilerContext<'ctx>,
    execution_result: &ExecutionResult<'ctx>,
) {
    context.builder.build_call(
        context.values.free_execution_result_function(),
        &[execution_result.0.into()],
        "",
    );
}

/// Insert a call which retrieves the executable stored at a given index in the cache.
pub(crate) fn get_executable<'ctx>(
    context: &mut QCSCompilerContext<'ctx>,
    index: IntValue<'ctx>,
) -> Result<Executable<'ctx>> {
    let cache_pointer = context
        .builder
        .build_load(context.values.executable_cache().as_pointer_value(), "");

    let call_site_value = context.builder.build_call(
        context.values.read_from_executable_cache(),
        &[cache_pointer.into(), index.into()],
        "",
    );

    Ok(Executable(
        call_site_value
            .try_as_basic_value()
            .left()
            .ok_or_else(|| eyre!("function does not return a value"))?
            .into_pointer_value(),
    ))
}

/// Insert a call which accepts as its only argument an `ExecutionResult`, and panics and exits if that
/// result is a failure/error.
pub(crate) fn panic_on_execution_result_failure<'ctx>(
    context: &mut QCSCompilerContext<'ctx>,
    execution_result: &ExecutionResult<'ctx>,
) {
    context.builder.build_call(
        context.values.panic_on_failure_function(),
        &[execution_result.0.into()],
        "",
    );
}

pub(crate) fn get_readout_bit<'ctx>(
    context: &mut QCSCompilerContext<'ctx>,
    execution_result: &ExecutionResult<'ctx>,
    shot_index: IntValue<'ctx>,
    readout_index: u64,
) -> Result<IntValue<'ctx>> {
    let result = context.builder.build_call(
        context.values.get_readout_bit_function(),
        &[
            BasicMetadataValueEnum::PointerValue(execution_result.0),
            shot_index.into(),
            context
                .base_context
                .i64_type()
                .const_int(readout_index, false)
                .into(),
        ],
        "",
    );

    Ok(result
        .try_as_basic_value()
        .left()
        .ok_or_else(|| eyre!("Expected basic value"))?
        .into_int_value())
}

pub(crate) fn set_param<'ctx>(
    context: &mut QCSCompilerContext<'ctx>,
    executable: &Executable<'ctx>,
    index: u64,
    value: FloatValue<'ctx>,
) {
    context.builder.build_call(
        context.values.set_param_function(),
        &[
            BasicMetadataValueEnum::PointerValue(executable.0),
            context.values.parameter_memory_region_name().into(),
            context
                .base_context
                .i32_type()
                .const_int(index, false)
                .into(),
            value.into(),
        ],
        "",
    );
}

pub(crate) fn wrap_in_shots<'ctx>(
    context: &mut QCSCompilerContext<'ctx>,
    executable: &Executable<'ctx>,
    shots: u64,
) {
    context.builder.build_call(
        context.values.wrap_in_shots_function(),
        &[
            BasicMetadataValueEnum::PointerValue(executable.0),
            context
                .base_context
                .i32_type()
                .const_int(shots, false)
                .into(),
        ],
        "",
    );
}
