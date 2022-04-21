%Qubit = type opaque
%Result = type opaque

declare void @__quantum__qis__h__body(%Qubit*) local_unnamed_addr
declare void @__quantum__qis__cnot__body(%Qubit*, %Qubit*) local_unnamed_addr
declare void @__quantum__qis__mz__body(%Qubit*, %Result*) local_unnamed_addr
declare i1 @__quantum__qis__read_result__body(%Result*) local_unnamed_addr

define internal fastcc i1 @main() unnamed_addr "EntryPoint" {

entry:
    tail call void @__quantum__qis__h__body(%Qubit* null)
    tail call void @__quantum__qis__cnot__body(%Qubit* null, %Qubit* nonnull inttoptr (i64 1 to %Qubit*))
    tail call void @__quantum__qis__mz__body(%Qubit* nonnull inttoptr (i64 0 to %Qubit*), %Result* null)
    tail call void @__quantum__qis__mz__body(%Qubit* nonnull inttoptr (i64 1 to %Qubit*), %Result* nonnull inttoptr (i64 1 to %Result*))
    %0 = tail call i1 @__quantum__qis__read_result__body(%Result* null)
    ret i1 false
}
