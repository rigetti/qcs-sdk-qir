%Qubit = type opaque
%Result = type opaque

declare void @__quantum__qis__rx__body(double, %Qubit*) local_unnamed_addr
declare void @__quantum__qis__ry__body(double, %Qubit*) local_unnamed_addr
declare void @__quantum__qis__rz__body(double, %Qubit*) local_unnamed_addr

define internal fastcc void @QuantumApplication__Run__body() unnamed_addr {

entry:
    br label %body

body:
    ; shot count variable
    %0 = phi i64 [ %1, %body ], [ 1, %entry ]

    tail call void @__quantum__qis__rx__body(double 2.0, %Qubit* null)
    tail call void @__quantum__qis__ry__body(double 2.0, %Qubit* null)
    tail call void @__quantum__qis__rz__body(double 2.0, %Qubit* null)

    %1 = add nuw nsw i64 %0, 1
    %2 = icmp ult i64 %0, 1000
    br i1 %2, label %body, label %exit

    br label %exit

exit:
    ret void
}



