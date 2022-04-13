%Qubit = type opaque
%Result = type opaque

declare void @__quantum__qis__toffoli__body(%Qubit*, %Qubit*, %Qubit*) local_unnamed_addr
declare void @__quantum__qis__mz__body(%Qubit*, %Result*) local_unnamed_addr
declare i1 @__quantum__qis__read_result__body(%Result*) local_unnamed_addr

define internal fastcc void @QuantumApplication__Run__body() unnamed_addr {

entry:
    br label %body

body:
    ; shot count variable
    %0 = phi i64 [ %2, %body ], [ 1, %entry ]

    tail call void @__quantum__qis__toffoli__body(%Qubit* nonnull inttoptr (i64 0 to %Qubit*), %Qubit* nonnull inttoptr (i64 1 to %Qubit*), %Qubit* nonnull inttoptr (i64 2 to %Qubit*))
    tail call void @__quantum__qis__mz__body(%Qubit* nonnull inttoptr (i64 2 to %Qubit*), %Result* null)
    %1 = tail call i1 @__quantum__qis__read_result__body(%Result* null)

    %2 = add nuw nsw i64 %0, 1
    %3 = icmp ult i64 %0, 1000
    br i1 %3, label %body, label %exit

    br label %exit

exit:
    ret void
}



