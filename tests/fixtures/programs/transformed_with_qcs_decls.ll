; ModuleID = 'qcs'
source_filename = "./tests/fixtures/programs/parametric.ll"

%ExecutableCache = type opaque
%Qubit = type opaque
%Executable = type opaque
%ExecutionResult = type opaque

@executable_cache = private global %ExecutableCache* null
@parameter_memory_region_name = private unnamed_addr constant [12 x i8] c"__qir_param\00", align 1
@quil_program = private unnamed_addr constant [131 x i8] c"DECLARE __qir_param REAL[3]\0ADECLARE ro BIT[0]\0ARZ(__qir_param[0]) 0\0ARZ(__qir_param[0]) 0\0ARZ(__qir_param[1]) 0\0ARZ(__qir_param[2]) 0\0A\00", align 1

declare void @__quantum__qis__rz__body(double %0, %Qubit* %1) local_unnamed_addr

define internal fastcc void @QuantumApplication__Run__body() unnamed_addr {
entry:
  call void @populate_executable_array()
  %0 = fadd double 1.000000e+00, 2.000000e+00
  br label %body_execution

body:                                             ; preds = %body, %body_execution
  %1 = phi i64 [ %2, %body ], [ 1, %body_execution ]
  %2 = add nuw nsw i64 %1, 1
  %3 = icmp ult i64 %1, 1000
  br i1 %3, label %body, label %body_cleanup

body_cleanup:                                     ; preds = %body
  call void @free_execution_result(%ExecutionResult* %5)
  br label %exit

body_execution:                                   ; preds = %entry, <null operand!>
  %4 = call %Executable* @executable_from_quil(i8* getelementptr inbounds ([131 x i8], [131 x i8]* @quil_program, i32 0, i32 0))
  call void @wrap_in_shots(%Executable* %4, i32 1000)
  call void @set_param(%Executable* %4, i8* getelementptr inbounds ([12 x i8], [12 x i8]* @parameter_memory_region_name, i32 0, i32 0), i32 0, double %0)
  call void @set_param(%Executable* %4, i8* getelementptr inbounds ([12 x i8], [12 x i8]* @parameter_memory_region_name, i32 0, i32 0), i32 1, double 2.000000e+00)
  call void @set_param(%Executable* %4, i8* getelementptr inbounds ([12 x i8], [12 x i8]* @parameter_memory_region_name, i32 0, i32 0), i32 2, double 0x40283F35BA6E72C7)
  %5 = call %ExecutionResult* @execute_on_qvm(%Executable* %4)
  call void @panic_on_failure(%ExecutionResult* %5)
  br label %body

exit:                                             ; preds = %body_cleanup, <null operand!>
  ret void
}

declare %Executable* @executable_from_quil(i8* %0)

declare %ExecutionResult* @execute_on_qpu(%Executable* %0, i8* %1)

declare %ExecutionResult* @execute_on_qvm(%Executable* %0)

declare void @free_executable(%Executable* %0)

declare void @free_execution_result(%ExecutionResult* %0)

declare i1 @get_readout_bit(%ExecutionResult* %0, i64 %1, i64 %2)

declare void @panic_on_failure(%ExecutionResult* %0)

declare void @set_param(%Executable* %0, i8* %1, i32 %2, double %3)

declare void @wrap_in_shots(%Executable* %0, i32 %1)

declare %ExecutableCache* @create_executable_cache(i32 %0)

declare void @add_executable_cache_item(%ExecutableCache* %0, i32 %1, i8* %2)

declare %Executable* @read_from_executable_cache(%ExecutableCache* %0, i32 %1)

declare void @free_executable_cache(%ExecutableCache* %0)

define void @populate_executable_array() {
entry:
  %0 = call %ExecutableCache* @create_executable_cache(i32 0)
  store %ExecutableCache* %0, %ExecutableCache** @executable_cache, align 8
  ret void
}