%Qubit = type opaque
%Result = type opaque

declare void @__quantum__qis__h__body(%Qubit*) local_unnamed_addr

; simple function which executes a hadamard gate and that's it.
define internal fastcc void @QuantumApplication__Run__body() unnamed_addr {

entry:
    br label %body

body:
    ; shot count variable
    %0 = phi i64 [ %1, %body ], [ 1, %entry ]
    tail call void @__quantum__qis__h__body(%Qubit* nonnull inttoptr (i64 12 to %Qubit*))
    %1 = add nuw nsw i64 %0, 1
    %2 = icmp ult i64 %0, 1000
    br i1 %2, label %body, label %exit

exit:
    ret void
}