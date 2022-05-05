%Qubit = type opaque
%Result = type opaque

@tag = internal constant [10 x i8] c"tag_value\00"

declare void @__quantum__qis__h__body(%Qubit*) local_unnamed_addr
declare void @__quantum__qis__mz__body(%Qubit*, %Result*) local_unnamed_addr
declare void @__quantum__rt__result_record_output(%Result*, i8*) ; i8 typed pointer can be `null`
declare void @__quantum__rt__tuple_start_record_output() 
declare void @__quantum__rt__tuple_end_record_output() 
declare void @__quantum__rt__array_start_record_output() 
declare void @__quantum__rt__array_end_record_output() 

define internal fastcc void @QuantumApplication__Run__body() unnamed_addr {

entry:
    br label %body

body:
    ; shot count variable
    %0 = phi i64 [ %1, %body ], [ 1, %entry ]

    tail call void @__quantum__qis__h__body(%Qubit* nonnull inttoptr (i64 0 to %Qubit*))
    tail call void @__quantum__qis__h__body(%Qubit* nonnull inttoptr (i64 1 to %Qubit*))
    tail call void @__quantum__qis__mz__body(%Qubit* nonnull inttoptr (i64 0 to %Qubit*), %Result* nonnull inttoptr (i64 0 to %Result*))
    tail call void @__quantum__qis__mz__body(%Qubit* nonnull inttoptr (i64 1 to %Qubit*), %Result* nonnull inttoptr (i64 1 to %Result*))
    tail call void @__quantum__qis__mz__body(%Qubit* nonnull inttoptr (i64 1 to %Qubit*), %Result* nonnull inttoptr (i64 1 to %Result*))
    tail call void @__quantum__rt__result_record_output(%Result* null, i8* null)
    tail call void @__quantum__rt__result_record_output(%Result* null, i8* getelementptr inbounds ([10 x i8], [10 x i8]* @tag, i32 0, i32 0))
    tail call void @__quantum__rt__tuple_start_record_output()
    tail call void @__quantum__rt__result_record_output(%Result* nonnull inttoptr (i64 0 to %Result*), i8* null)
    tail call void @__quantum__rt__result_record_output(%Result* nonnull inttoptr (i64 1 to %Result*), i8* null)
    tail call void @__quantum__rt__tuple_end_record_output()
    tail call void @__quantum__rt__array_start_record_output()
    tail call void @__quantum__rt__result_record_output(%Result* null, i8* null)
    tail call void @__quantum__rt__result_record_output(%Result* null, i8* null)
    tail call void @__quantum__rt__array_end_record_output()

    ; shot count branch
    %1 = add nuw nsw i64 %0, 1
    %2 = icmp ult i64 %0, 42
    br i1 %2, label %body, label %exit

exit:
    ret void
}
