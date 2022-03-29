# `transpile-to-quil`

To transpile an input QIR program to Quil, run the CLI as shown here, following the LLVM-related instructions above. Note that this command only works for "simple" QIR modules which satisfy the following:

- All quantum instructions are contained within a single basic block, labeled `body`, within the entrypoint function.
- That function itself makes no function calls within the `body` block.
- The `body` basic block satisfies the _basic block preconditions_ described above in [QIR Preconditions](#qir-preconditions).

```
$ qcs-sdk-qir transpile-to-quil tests/fixtures/programs/bell_state.bc
shot count: 42
program: DECLARE ro BIT[2]
H 0
CNOT 0 1
MEASURE 0 ro[0]
MEASURE 1 ro[1]



```
