%Qubit = type opaque
%Result = type opaque

declare void @__quantum__qis__h__body(%Qubit*) local_unnamed_addr
declare void @__quantum__qis__cnot__body(%Qubit*, %Qubit*) local_unnamed_addr
declare void @__quantum__qis__mz__body(%Qubit*, %Result*) local_unnamed_addr
declare i1 @__quantum__qis__read_result__body(%Result*) local_unnamed_addr

; simple function which measures a single qubit and that's it.
define internal fastcc void @QuantumApplication__Run__body() unnamed_addr {

entry:
    br label %body

body:
    ; shot count variable
    %0 = phi i64 [ %2, %body ], [ 1, %entry ]

    ; result counter
    %count = phi i64 [ %select, %body ], [ 0, %entry ]

    tail call void @__quantum__qis__h__body(%Qubit* null)
    tail call void @__quantum__qis__cnot__body(%Qubit* null, %Qubit* nonnull inttoptr (i64 1 to %Qubit*))
    tail call void @__quantum__qis__mz__body(%Qubit* nonnull inttoptr (i64 0 to %Qubit*), %Result* null)
    tail call void @__quantum__qis__mz__body(%Qubit* nonnull inttoptr (i64 1 to %Qubit*), %Result* nonnull inttoptr (i64 1 to %Result*))
    %1 = tail call i1 @__quantum__qis__read_result__body(%Result* null)

    ; reduction operation: increment a counter
    %2 = zext i1 %1 to i64
    %select = add i64 %count, %2

    ; shot count branch
    %3 = add nuw nsw i64 %0, 1
    %4 = icmp ult i64 %0, 42
    br i1 %4, label %body, label %exit

exit:
    ret void
}



