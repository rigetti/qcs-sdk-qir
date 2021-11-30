%Qubit = type opaque
%Result = type opaque

declare void @__quantum__qis__h__body(%Qubit*) local_unnamed_addr

; simple function which executes a hadamard gate on qubit 0 and that's it.
define internal fastcc void @QuantumApplication__Run__body() unnamed_addr {

body:
    tail call void @__quantum__qis__h__body(%Qubit* null)
    ret void
}