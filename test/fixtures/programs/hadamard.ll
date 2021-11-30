%Qubit = type opaque
%Result = type opaque

declare void @__quantum__qis__h__body(%Qubit*) local_unnamed_addr

; simple function which executes a hadamard gate and that's it.
define internal fastcc void @QuantumApplication__Run__body() unnamed_addr {

body:
    tail call void @__quantum__qis__h__body(%Qubit* nonnull inttoptr (i64 12 to %Qubit*))
    ret void
}