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
use inkwell::builder::BuilderError;
use inkwell::{
    builder::Builder,
    context::Context,
    module::{Linkage, Module},
    types::BasicMetadataTypeEnum,
    values::{FunctionValue, GlobalValue, PointerValue},
    AddressSpace,
};

use crate::interop::entrypoint::get_entry_function;
use crate::transform::PARAMETER_MEMORY_REGION_NAME;

use super::{target::ExecutionTarget, types::Types};

fn build_executable_from_quil_function<'ctx>(
    _context: &'ctx Context,
    _builder: &Builder<'ctx>,
    module: &Module<'ctx>,
    types: &Types<'ctx>,
) -> FunctionValue<'ctx> {
    const FN_NAME_EXECUTABLE_FROM_QUIL: &str = "executable_from_quil";

    if let Some(existing_function) = module.get_function(FN_NAME_EXECUTABLE_FROM_QUIL) {
        existing_function
    } else {
        let string_type = types.string();
        let executable_type = types.executable(module);
        let executable_pointer_type = executable_type.ptr_type(AddressSpace::default());

        let executable_from_quil_type = executable_pointer_type
            .fn_type(&[BasicMetadataTypeEnum::PointerType(string_type)], false);
        module.add_function(
            FN_NAME_EXECUTABLE_FROM_QUIL,
            executable_from_quil_type,
            Some(Linkage::External),
        )
    }
}

fn build_execute_on_qpu_function<'ctx>(
    _context: &'ctx Context,
    _builder: &Builder<'ctx>,
    module: &Module<'ctx>,
    types: &Types<'ctx>,
) -> FunctionValue<'ctx> {
    const FN_NAME_EXECUTE_ON_QPU: &str = "execute_on_qpu";

    if let Some(existing_function) = module.get_function(FN_NAME_EXECUTE_ON_QPU) {
        existing_function
    } else {
        let string_type = types.string();
        let executable_type = types.executable(module);
        let executable_pointer_type = executable_type.ptr_type(AddressSpace::default());

        let execution_result_type = types.execution_result(module);
        let execution_result_pointer_type = execution_result_type.ptr_type(AddressSpace::default());
        let execute_on_qpu_type = execution_result_pointer_type.fn_type(
            &[
                BasicMetadataTypeEnum::PointerType(executable_pointer_type),
                BasicMetadataTypeEnum::PointerType(string_type),
            ],
            false,
        );
        module.add_function(
            FN_NAME_EXECUTE_ON_QPU,
            execute_on_qpu_type,
            Some(Linkage::External),
        )
    }
}

fn build_execute_on_qvm_function<'ctx>(
    _context: &'ctx Context,
    _builder: &Builder<'ctx>,
    module: &Module<'ctx>,
    types: &Types<'ctx>,
) -> FunctionValue<'ctx> {
    const FN_NAME_EXECUTE_ON_QVM: &str = "execute_on_qvm";

    if let Some(existing_function) = module.get_function(FN_NAME_EXECUTE_ON_QVM) {
        existing_function
    } else {
        let executable_type = types.executable(module);
        let executable_pointer_type = executable_type.ptr_type(AddressSpace::default());

        let execution_result_type = types.execution_result(module);
        let execution_result_pointer_type = execution_result_type.ptr_type(AddressSpace::default());
        let execute_on_qvm_type = execution_result_pointer_type.fn_type(
            &[BasicMetadataTypeEnum::PointerType(executable_pointer_type)],
            false,
        );
        module.add_function(
            FN_NAME_EXECUTE_ON_QVM,
            execute_on_qvm_type,
            Some(Linkage::External),
        )
    }
}

fn build_free_executable_function<'ctx>(
    context: &'ctx Context,
    _builder: &Builder<'ctx>,
    module: &Module<'ctx>,
    types: &Types<'ctx>,
) -> FunctionValue<'ctx> {
    const FN_NAME_FREE_EXECUTABLE: &str = "free_executable";

    if let Some(existing_function) = module.get_function(FN_NAME_FREE_EXECUTABLE) {
        existing_function
    } else {
        let executable_type = types.executable(module);
        let executable_pointer_type = executable_type.ptr_type(AddressSpace::default());

        let free_executable_type = context.void_type().fn_type(
            &[BasicMetadataTypeEnum::PointerType(executable_pointer_type)],
            false,
        );
        module.add_function(
            FN_NAME_FREE_EXECUTABLE,
            free_executable_type,
            Some(Linkage::External),
        )
    }
}

fn build_free_execution_result_function<'ctx>(
    context: &'ctx Context,
    _builder: &Builder<'ctx>,
    module: &Module<'ctx>,
    types: &Types<'ctx>,
) -> FunctionValue<'ctx> {
    const FN_NAME_FREE_EXECUTION_RESULT: &str = "free_execution_result";

    if let Some(existing_function) = module.get_function(FN_NAME_FREE_EXECUTION_RESULT) {
        existing_function
    } else {
        let execution_result_type = types.execution_result(module);
        let execution_result_pointer_type = execution_result_type.ptr_type(AddressSpace::default());
        let free_execution_result_type = context.void_type().fn_type(
            &[BasicMetadataTypeEnum::PointerType(
                execution_result_pointer_type,
            )],
            false,
        );
        module.add_function(
            FN_NAME_FREE_EXECUTION_RESULT,
            free_execution_result_type,
            Some(Linkage::External),
        )
    }
}

fn build_create_executable_cache_function<'ctx>(
    context: &'ctx Context,
    _builder: &Builder<'ctx>,
    module: &Module<'ctx>,
    types: &Types<'ctx>,
) -> FunctionValue<'ctx> {
    const FN_NAME_CREATE_EXECUTABLE_CACHE: &str = "create_executable_cache";

    if let Some(existing_function) = module.get_function(FN_NAME_CREATE_EXECUTABLE_CACHE) {
        existing_function
    } else {
        module.add_function(
            FN_NAME_CREATE_EXECUTABLE_CACHE,
            types
                .executable_cache(module)
                .ptr_type(AddressSpace::default())
                .fn_type(&[context.i32_type().into()], false),
            None,
        )
    }
}

fn build_add_executable_cache_item_function<'ctx>(
    context: &'ctx Context,
    _builder: &Builder<'ctx>,
    module: &Module<'ctx>,
    types: &Types<'ctx>,
) -> FunctionValue<'ctx> {
    const FN_NAME_ADD_EXECUTABLE_CACHE_ITEM: &str = "add_executable_cache_item";

    if let Some(existing_function) = module.get_function(FN_NAME_ADD_EXECUTABLE_CACHE_ITEM) {
        existing_function
    } else {
        module.add_function(
            FN_NAME_ADD_EXECUTABLE_CACHE_ITEM,
            context.void_type().fn_type(
                &[
                    types
                        .executable_cache(module)
                        .ptr_type(AddressSpace::default())
                        .into(),
                    context.i32_type().into(),
                    types.string().into(),
                ],
                false,
            ),
            None,
        )
    }
}

fn build_read_from_executable_cache_function<'ctx>(
    context: &'ctx Context,
    _builder: &Builder<'ctx>,
    module: &Module<'ctx>,
    types: &Types<'ctx>,
) -> FunctionValue<'ctx> {
    const FN_NAME_READ_FROM_EXECTUABLE_CACHE: &str = "read_from_executable_cache";

    if let Some(existing_function) = module.get_function(FN_NAME_READ_FROM_EXECTUABLE_CACHE) {
        existing_function
    } else {
        module.add_function(
            FN_NAME_READ_FROM_EXECTUABLE_CACHE,
            types
                .executable(module)
                .ptr_type(AddressSpace::default())
                .fn_type(
                    &[
                        types
                            .executable_cache(module)
                            .ptr_type(AddressSpace::default())
                            .into(),
                        context.i32_type().into(),
                    ],
                    false,
                ),
            None,
        )
    }
}

fn build_free_executable_cache_function<'ctx>(
    context: &'ctx Context,
    _builder: &Builder<'ctx>,
    module: &Module<'ctx>,
    types: &Types<'ctx>,
) -> FunctionValue<'ctx> {
    const FN_NAME_FREE_EXECUTABLE_CACHE: &str = "free_executable_cache";

    if let Some(existing_function) = module.get_function(FN_NAME_FREE_EXECUTABLE_CACHE) {
        existing_function
    } else {
        module.add_function(
            FN_NAME_FREE_EXECUTABLE_CACHE,
            context.void_type().fn_type(
                &[types
                    .executable_cache(module)
                    .ptr_type(AddressSpace::default())
                    .into()],
                false,
            ),
            None,
        )
    }
}

fn build_get_readout_bit_function<'ctx>(
    context: &'ctx Context,
    _builder: &Builder<'ctx>,
    module: &Module<'ctx>,
    types: &Types<'ctx>,
) -> FunctionValue<'ctx> {
    const FN_NAME_GET_READOUT_BIT: &str = "get_readout_bit";

    if let Some(existing_function) = module.get_function(FN_NAME_GET_READOUT_BIT) {
        existing_function
    } else {
        let execution_result_type = types.execution_result(module);
        let execution_result_pointer_type = execution_result_type.ptr_type(AddressSpace::default());

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
            FN_NAME_GET_READOUT_BIT,
            get_readout_bit_type,
            Some(Linkage::External),
        )
    }
}

fn build_set_param_function<'ctx>(
    context: &'ctx Context,
    _builder: &Builder<'ctx>,
    module: &Module<'ctx>,
    types: &Types<'ctx>,
) -> FunctionValue<'ctx> {
    const FN_NAME_SET_PARAM: &str = "set_param";

    if let Some(existing_function) = module.get_function(FN_NAME_SET_PARAM) {
        existing_function
    } else {
        let executable_type = types.executable(module);
        let executable_pointer_type = executable_type.ptr_type(AddressSpace::default());

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

        module.add_function(FN_NAME_SET_PARAM, set_param_type, Some(Linkage::External))
    }
}

fn build_panic_on_failure_function<'ctx>(
    context: &'ctx Context,
    _builder: &Builder<'ctx>,
    module: &Module<'ctx>,
    types: &Types<'ctx>,
) -> FunctionValue<'ctx> {
    const FN_NAME_PANIC_ON_FAILURE: &str = "panic_on_failure";

    if let Some(existing_function) = module.get_function(FN_NAME_PANIC_ON_FAILURE) {
        existing_function
    } else {
        let execution_result_type = types.execution_result(module);
        let execution_result_pointer_type = execution_result_type.ptr_type(AddressSpace::default());

        let panic_type = context.void_type().fn_type(
            &[BasicMetadataTypeEnum::PointerType(
                execution_result_pointer_type,
            )],
            false,
        );

        module.add_function(
            FN_NAME_PANIC_ON_FAILURE,
            panic_type,
            Some(Linkage::External),
        )
    }
}

fn build_parameter_memory_region_name<'ctx>(
    _context: &'ctx Context,
    builder: &Builder<'ctx>,
    module: &Module<'ctx>,
    types: &Types<'ctx>,
) -> Result<PointerValue<'ctx>, BuilderError> {
    const GLOBAL_NAME_PARAM_MEM_REGION: &str = "parameter_memory_region_name";

    let global_string = match module.get_global(GLOBAL_NAME_PARAM_MEM_REGION) {
        Some(gv) => gv,
        None => unsafe {
            // NOTE: this segfaults if the builder is not already positioned within a basic block
            // see https://github.com/TheDan64/inkwell/issues/32
            builder.build_global_string(PARAMETER_MEMORY_REGION_NAME, GLOBAL_NAME_PARAM_MEM_REGION)?
        },
    };

    Ok(global_string.as_pointer_value().const_cast(types.string()))
}

fn build_quantum_processor_id<'ctx>(
    _context: &'ctx Context,
    builder: &Builder<'ctx>,
    module: &Module<'ctx>,
    types: &Types<'ctx>,
    target: &ExecutionTarget,
) -> Result<Option<PointerValue<'ctx>>, BuilderError> {
    if let ExecutionTarget::Qpu(quantum_processor_id) = target {
        const GLOBAL_NAME_QPU_ID: &str = "quantum_processor_id";
        let global_string = match module.get_global(GLOBAL_NAME_QPU_ID) {
            Some(gv) => gv,
            None => unsafe {
                // NOTE: this segfaults if the builder is not already positioned within a basic block
                // see https://github.com/TheDan64/inkwell/issues/32
                builder.build_global_string(quantum_processor_id, GLOBAL_NAME_QPU_ID)?
            },
        };

        Ok(Some(global_string.as_pointer_value().const_cast(types.string())))
    } else {
        Ok(None)
    }
}

fn build_wrap_in_shots_function<'ctx>(
    context: &'ctx Context,
    _builder: &Builder<'ctx>,
    module: &Module<'ctx>,
    types: &Types<'ctx>,
) -> FunctionValue<'ctx> {
    const FN_NAME_WRAP_IN_SHOTS: &str = "wrap_in_shots";

    if let Some(existing_function) = module.get_function(FN_NAME_WRAP_IN_SHOTS) {
        existing_function
    } else {
        let executable_type = types.executable(module);
        let executable_pointer_type = executable_type.ptr_type(AddressSpace::default());
        let i32_type = context.i32_type();

        let wrap_in_shots_type = context.void_type().fn_type(
            &[
                BasicMetadataTypeEnum::PointerType(executable_pointer_type),
                BasicMetadataTypeEnum::IntType(i32_type),
            ],
            false,
        );

        module.add_function(
            FN_NAME_WRAP_IN_SHOTS,
            wrap_in_shots_type,
            Some(Linkage::External),
        )
    }
}

pub(crate) struct Values<'ctx> {
    free_executable_function: FunctionValue<'ctx>,
    free_execution_result_function: FunctionValue<'ctx>,
    executable_from_quil_function: FunctionValue<'ctx>,
    execute_on_qpu_function: FunctionValue<'ctx>,
    execute_on_qvm_function: FunctionValue<'ctx>,
    get_readout_bit_function: FunctionValue<'ctx>,
    panic_on_failure_function: FunctionValue<'ctx>,
    parameter_memory_region_name: PointerValue<'ctx>,
    quantum_processor_id: Option<PointerValue<'ctx>>,
    set_param_function: FunctionValue<'ctx>,
    wrap_in_shots_function: FunctionValue<'ctx>,

    executable_cache: GlobalValue<'ctx>,
    create_executable_cache: FunctionValue<'ctx>,
    add_executable_cache_item: FunctionValue<'ctx>,
    read_from_executable_cache: FunctionValue<'ctx>,
    free_executable_cache: FunctionValue<'ctx>,
}

impl<'ctx> Values<'ctx> {
    /// Get a reference to the values's executable from quil function.
    #[allow(dead_code)]
    pub(crate) fn executable_from_quil_function(&self) -> FunctionValue<'ctx> {
        self.executable_from_quil_function
    }

    /// Get a reference to the values's execute on qpu function.
    pub(crate) fn execute_on_qpu_function(&self) -> FunctionValue<'ctx> {
        self.execute_on_qpu_function
    }

    /// Get a reference to the values's execute on qvm function.
    pub(crate) fn execute_on_qvm_function(&self) -> FunctionValue<'ctx> {
        self.execute_on_qvm_function
    }

    /// Get a reference to the values's get readout bit function.
    pub(crate) fn get_readout_bit_function(&self) -> FunctionValue<'ctx> {
        self.get_readout_bit_function
    }

    pub(crate) fn new(
        context: &'ctx Context,
        builder: &Builder<'ctx>,
        module: &Module<'ctx>,
        types: &Types<'ctx>,
        target: &ExecutionTarget,
    ) -> Result<Self> {
        // To create global values, the builder must be positioned inside a basic block even if it never writes within that basic block.
        // see https://github.com/TheDan64/inkwell/issues/32
        let basic_block = get_entry_function(module)
            .and_then(FunctionValue::get_first_basic_block)
            .ok_or_else(|| eyre!("QIR expected entrypoint not found"))?;
        builder.position_at_end(basic_block);

        let executable_cache = module
            .get_global("executable_cache")
            .or_else(|| {
                Some(
                    module.add_global(
                        types
                            .executable_cache(module)
                            .ptr_type(AddressSpace::default()),
                        None,
                        "executable_cache",
                    ),
                )
            })
            .ok_or_else(|| eyre::eyre!("no executable_cache found or added"))?;

        executable_cache.set_linkage(Linkage::Private);
        executable_cache.set_externally_initialized(false);
        let initializer = types
            .executable_cache(module)
            .ptr_type(AddressSpace::default())
            .const_zero();
        executable_cache.set_initializer(&initializer);

        Ok(Self {
            executable_from_quil_function: build_executable_from_quil_function(
                context, builder, module, types,
            ),
            execute_on_qpu_function: build_execute_on_qpu_function(context, builder, module, types),
            execute_on_qvm_function: build_execute_on_qvm_function(context, builder, module, types),
            free_executable_function: build_free_executable_function(
                context, builder, module, types,
            ),
            free_execution_result_function: build_free_execution_result_function(
                context, builder, module, types,
            ),
            get_readout_bit_function: build_get_readout_bit_function(
                context, builder, module, types,
            ),
            panic_on_failure_function: build_panic_on_failure_function(
                context, builder, module, types,
            ),
            parameter_memory_region_name: build_parameter_memory_region_name(
                context, builder, module, types,
            )?,
            quantum_processor_id: build_quantum_processor_id(
                context, builder, module, types, target,
            )?,
            set_param_function: build_set_param_function(context, builder, module, types),
            wrap_in_shots_function: build_wrap_in_shots_function(context, builder, module, types),

            executable_cache,
            create_executable_cache: build_create_executable_cache_function(
                context, builder, module, types,
            ),
            add_executable_cache_item: build_add_executable_cache_item_function(
                context, builder, module, types,
            ),
            read_from_executable_cache: build_read_from_executable_cache_function(
                context, builder, module, types,
            ),
            free_executable_cache: build_free_executable_cache_function(
                context, builder, module, types,
            ),
        })
    }

    /// Get a reference to the values's panic on failure function.
    pub(crate) fn panic_on_failure_function(&self) -> FunctionValue<'ctx> {
        self.panic_on_failure_function
    }

    /// Get a reference to the values's parameter memory region name.
    pub(crate) fn parameter_memory_region_name(&self) -> PointerValue<'ctx> {
        self.parameter_memory_region_name
    }

    /// Get a reference to the values's quantum processor id.
    pub(crate) fn quantum_processor_id(&self) -> Option<PointerValue<'ctx>> {
        self.quantum_processor_id
    }

    /// Get a reference to the values's set param function.
    pub(crate) fn set_param_function(&self) -> FunctionValue<'ctx> {
        self.set_param_function
    }

    /// Get a reference to the values's wrap in shots function.
    pub(crate) fn wrap_in_shots_function(&self) -> FunctionValue<'ctx> {
        self.wrap_in_shots_function
    }

    /// Get a reference to the values's free executable function.
    #[allow(dead_code)]
    pub(crate) fn free_executable_function(&self) -> FunctionValue<'ctx> {
        self.free_executable_function
    }

    /// Get a reference to the values's free execution result function.
    pub(crate) fn free_execution_result_function(&self) -> FunctionValue<'ctx> {
        self.free_execution_result_function
    }

    /// Get a reference to the values's create executable cache.
    pub(crate) fn create_executable_cache(&self) -> FunctionValue<'ctx> {
        self.create_executable_cache
    }

    /// Get a reference to the values's add executable cache item.
    pub(crate) fn add_executable_cache_item(&self) -> FunctionValue<'ctx> {
        self.add_executable_cache_item
    }

    /// Get a reference to the values's read from executable cache.
    pub(crate) fn read_from_executable_cache(&self) -> FunctionValue<'ctx> {
        self.read_from_executable_cache
    }

    /// Get a reference to the values's free executable cache.
    #[allow(dead_code)]
    pub(crate) fn free_executable_cache(&self) -> FunctionValue<'ctx> {
        self.free_executable_cache
    }

    /// Get a reference to the values's executable cache.
    pub(crate) fn executable_cache(&self) -> GlobalValue<'ctx> {
        self.executable_cache
    }
}
