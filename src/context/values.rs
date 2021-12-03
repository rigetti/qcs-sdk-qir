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
    builder::Builder,
    context::Context,
    module::{Linkage, Module},
    types::BasicMetadataTypeEnum,
    values::{FunctionValue, PointerValue},
    AddressSpace,
};

use crate::interop::entrypoint::get_entry_function;
use crate::transform::shot_count_block::PARAMETER_MEMORY_REGION_NAME;

use super::{target::ExecutionTarget, types::Types};

fn build_executable_from_quil_function<'ctx>(
    _context: &'ctx Context,
    _builder: &Builder<'ctx>,
    module: &Module<'ctx>,
    types: &Types<'ctx>,
) -> FunctionValue<'ctx> {
    let string_type = types.string();
    let executable_type = types.executable();
    let executable_pointer_type = executable_type.ptr_type(AddressSpace::Generic);

    let executable_from_quil_type =
        executable_pointer_type.fn_type(&[BasicMetadataTypeEnum::PointerType(string_type)], false);
    module.add_function(
        "executable_from_quil",
        executable_from_quil_type,
        Some(Linkage::External),
    )
}

fn build_execute_on_qpu_function<'ctx>(
    _context: &'ctx Context,
    _builder: &Builder<'ctx>,
    module: &Module<'ctx>,
    types: &Types<'ctx>,
) -> FunctionValue<'ctx> {
    let string_type = types.string();
    let executable_type = types.executable();
    let executable_pointer_type = executable_type.ptr_type(AddressSpace::Generic);

    let execution_result_type = types.execution_result();
    let execution_result_pointer_type = execution_result_type.ptr_type(AddressSpace::Generic);
    let execute_on_qpu_type = execution_result_pointer_type.fn_type(
        &[
            BasicMetadataTypeEnum::PointerType(executable_pointer_type),
            BasicMetadataTypeEnum::PointerType(string_type),
        ],
        false,
    );
    module.add_function(
        "execute_on_qpu",
        execute_on_qpu_type,
        Some(Linkage::External),
    )
}

fn build_execute_on_qvm_function<'ctx>(
    _context: &'ctx Context,
    _builder: &Builder<'ctx>,
    module: &Module<'ctx>,
    types: &Types<'ctx>,
) -> FunctionValue<'ctx> {
    let executable_type = types.executable();
    let executable_pointer_type = executable_type.ptr_type(AddressSpace::Generic);

    let execution_result_type = types.execution_result();
    let execution_result_pointer_type = execution_result_type.ptr_type(AddressSpace::Generic);
    let execute_on_qvm_type = execution_result_pointer_type.fn_type(
        &[BasicMetadataTypeEnum::PointerType(executable_pointer_type)],
        false,
    );
    module.add_function(
        "execute_on_qvm",
        execute_on_qvm_type,
        Some(Linkage::External),
    )
}

fn build_get_readout_bit_function<'ctx>(
    context: &'ctx Context,
    _builder: &Builder<'ctx>,
    module: &Module<'ctx>,
    types: &Types<'ctx>,
) -> FunctionValue<'ctx> {
    let execution_result_type = types.execution_result();
    let execution_result_pointer_type = execution_result_type.ptr_type(AddressSpace::Generic);

    let i64_type = context.i64_type();

    let get_readout_bit_type = context.bool_type().fn_type(
        &[
            BasicMetadataTypeEnum::PointerType(execution_result_pointer_type),
            BasicMetadataTypeEnum::IntType(i64_type),
            BasicMetadataTypeEnum::IntType(i64_type),
        ],
        false,
    );

    module.add_function(
        "get_readout_bit",
        get_readout_bit_type,
        Some(Linkage::External),
    )
}

fn build_set_param_function<'ctx>(
    context: &'ctx Context,
    _builder: &Builder<'ctx>,
    module: &Module<'ctx>,
    types: &Types<'ctx>,
) -> FunctionValue<'ctx> {
    let executable_type = types.executable();
    let executable_pointer_type = executable_type.ptr_type(AddressSpace::Generic);

    let string_type = types.string();
    let name_type = string_type;
    let index_type = context.i32_type();
    let value_type = context.f64_type();

    let set_param_type = context.void_type().fn_type(
        &[
            executable_pointer_type.into(),
            name_type.into(),
            index_type.into(),
            value_type.into(),
        ],
        false,
    );

    module.add_function("set_param", set_param_type, Some(Linkage::External))
}

fn build_panic_on_failure_function<'ctx>(
    context: &'ctx Context,
    _builder: &Builder<'ctx>,
    module: &Module<'ctx>,
    types: &Types<'ctx>,
) -> FunctionValue<'ctx> {
    let execution_result_type = types.execution_result();
    let execution_result_pointer_type = execution_result_type.ptr_type(AddressSpace::Generic);

    let panic_type = context.void_type().fn_type(
        &[BasicMetadataTypeEnum::PointerType(
            execution_result_pointer_type,
        )],
        false,
    );

    module.add_function("panic_on_failure", panic_type, Some(Linkage::External))
}

fn build_parameter_memory_region_name<'ctx>(
    _context: &'ctx Context,
    builder: &Builder<'ctx>,
    _module: &Module<'ctx>,
    types: &Types<'ctx>,
) -> PointerValue<'ctx> {
    let global_string = unsafe {
        // NOTE: this segfaults if the builder is not already positioned within a basic block
        // see https://github.com/TheDan64/inkwell/issues/32
        builder.build_global_string(PARAMETER_MEMORY_REGION_NAME, "parameter_memory_region_name")
    };
    global_string.as_pointer_value().const_cast(types.string())
}

fn build_quantum_processor_id<'ctx>(
    _context: &'ctx Context,
    builder: &Builder<'ctx>,
    _module: &Module<'ctx>,
    types: &Types<'ctx>,
    target: &ExecutionTarget,
) -> Option<PointerValue<'ctx>> {
    if let ExecutionTarget::QPU(quantum_processor_id) = target {
        let global_string = unsafe {
            // NOTE: this segfaults if the builder is not already positioned within a basic block
            // see https://github.com/TheDan64/inkwell/issues/32
            builder.build_global_string(quantum_processor_id, "quantum_processor_id")
        };
        Some(global_string.as_pointer_value().const_cast(types.string()))
    } else {
        None
    }
}

fn build_wrap_in_shots_function<'ctx>(
    context: &'ctx Context,
    _builder: &Builder<'ctx>,
    module: &Module<'ctx>,
    types: &Types<'ctx>,
) -> FunctionValue<'ctx> {
    let executable_type = types.executable();
    let executable_pointer_type = executable_type.ptr_type(AddressSpace::Generic);
    let i32_type = context.i32_type();

    let wrap_in_shots_type = context.void_type().fn_type(
        &[
            BasicMetadataTypeEnum::PointerType(executable_pointer_type),
            BasicMetadataTypeEnum::IntType(i32_type),
        ],
        false,
    );

    module.add_function("wrap_in_shots", wrap_in_shots_type, Some(Linkage::External))
}

pub(crate) struct Values<'ctx> {
    executable_from_quil_function: FunctionValue<'ctx>,
    execute_on_qpu_function: FunctionValue<'ctx>,
    execute_on_qvm_function: FunctionValue<'ctx>,
    get_readout_bit_function: FunctionValue<'ctx>,
    panic_on_failure_function: FunctionValue<'ctx>,
    parameter_memory_region_name: PointerValue<'ctx>,
    quantum_processor_id: Option<PointerValue<'ctx>>,
    set_param_function: FunctionValue<'ctx>,
    wrap_in_shots_function: FunctionValue<'ctx>,
}

impl<'ctx> Values<'ctx> {
    /// Get a reference to the values's executable from quil function.
    pub fn executable_from_quil_function(&self) -> FunctionValue<'ctx> {
        self.executable_from_quil_function
    }

    /// Get a reference to the values's execute on qpu function.
    pub fn execute_on_qpu_function(&self) -> FunctionValue<'ctx> {
        self.execute_on_qpu_function
    }

    /// Get a reference to the values's execute on qvm function.
    pub fn execute_on_qvm_function(&self) -> FunctionValue<'ctx> {
        self.execute_on_qvm_function
    }

    /// Get a reference to the values's get readout bit function.
    pub fn get_readout_bit_function(&self) -> FunctionValue<'ctx> {
        self.get_readout_bit_function
    }

    pub fn new(
        context: &'ctx Context,
        builder: &Builder<'ctx>,
        module: &Module<'ctx>,
        types: &Types<'ctx>,
        target: &ExecutionTarget,
    ) -> Self {
        // To create global values, the builder must be positioned inside a basic block even if it never writes within that basic block.
        // see https://github.com/TheDan64/inkwell/issues/32
        let basic_block = get_entry_function(module)
            .expect("QIR expected entrypoint not found")
            .get_first_basic_block()
            .unwrap();
        builder.position_at_end(basic_block);
        Self {
            executable_from_quil_function: build_executable_from_quil_function(
                context, builder, module, types,
            ),
            execute_on_qpu_function: build_execute_on_qpu_function(context, builder, module, types),
            execute_on_qvm_function: build_execute_on_qvm_function(context, builder, module, types),
            get_readout_bit_function: build_get_readout_bit_function(
                context, builder, module, types,
            ),
            panic_on_failure_function: build_panic_on_failure_function(
                context, builder, module, types,
            ),
            parameter_memory_region_name: build_parameter_memory_region_name(
                context, builder, module, types,
            ),
            quantum_processor_id: build_quantum_processor_id(
                context, builder, module, types, target,
            ),
            set_param_function: build_set_param_function(context, builder, module, types),
            wrap_in_shots_function: build_wrap_in_shots_function(context, builder, module, types),
        }
    }

    /// Get a reference to the values's panic on failure function.
    pub fn panic_on_failure_function(&self) -> FunctionValue<'ctx> {
        self.panic_on_failure_function
    }

    /// Get a reference to the values's parameter memory region name.
    pub fn parameter_memory_region_name(&self) -> PointerValue<'ctx> {
        self.parameter_memory_region_name
    }

    /// Get a reference to the values's quantum processor id.
    pub fn quantum_processor_id(&self) -> Option<PointerValue<'ctx>> {
        self.quantum_processor_id
    }

    /// Get a reference to the values's set param function.
    pub fn set_param_function(&self) -> FunctionValue<'ctx> {
        self.set_param_function
    }

    /// Get a reference to the values's wrap in shots function.
    pub fn wrap_in_shots_function(&self) -> FunctionValue<'ctx> {
        self.wrap_in_shots_function
    }
}
