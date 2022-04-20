%Qubit = type opaque
%Result = type opaque

declare void @__quantum__qis__rz__body(i32, %Qubit*) local_unnamed_addr

; function that executes an RZ gate on a single qubit, parameterized by 2 values
define internal fastcc void @QuantumApplication__Run__body() unnamed_addr {

entry:
    ; how do I just declare a double const? ie %0 = 3.0
    %0 = fadd double 1.000000e+00, 2.000000e+00
    br label %body

body:
    ; shot count variable
    %1 = phi i64 [ %2, %body ], [ 1, %entry ]
    tail call void @__quantum__qis__rz__body(i32 42, %Qubit* null)
    %2 = add nuw nsw i64 %1, 1
    %3 = icmp ult i64 %1, 1000
    br i1 %3, label %body, label %exit

exit:
    ret void
}