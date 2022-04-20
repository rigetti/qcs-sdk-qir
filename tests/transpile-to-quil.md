# `transpile-to-quil`

To transpile an input QIR program to Quil, run the CLI as shown here:

```
$ qcs-sdk-qir transpile-to-quil tests/fixtures/programs/bell_state.bc
{
  "program": "DECLARE ro BIT[2]/nH 0/nCNOT 0 1/nMEASURE 0 ro[0]/nMEASURE 1 ro[1]/n",
  "shot_count": 42,
  "recorded_output": [
    "shot_start",
    "shot_end"
  ]
}

```

Note that this command only works for "simple" QIR modules which satisfy the following:

- All quantum instructions are contained within a single basic block, labeled `body`, within the entrypoint function.
- That function itself makes no function calls within the `body` block.
- The `body` basic block satisfies the _basic block preconditions_ described above in [QIR Preconditions](#qir-preconditions).

Providing an invalid QIR program will result in an error:

```
$ qcs-sdk-qir transpile-to-quil tests/fixtures/programs/non-unitary.bc
? failed
Error: Encountered invalid parameter(s)[..]
...
```
