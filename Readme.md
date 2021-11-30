# QCS QIR SDK

Compile & run Quantum Intermediate Representation (QIR) programs on Rigetti QCS.

## Examples

Given an input QIR program that might look like this:

```LLVM
%Qubit = type opaque
%Result = type opaque

declare void @__quantum__qis__h__body(%Qubit*) local_unnamed_addr
declare void @__quantum__qis__mz__body(%Qubit*, %Result*) local_unnamed_addr
declare i1 @__quantum__qir__read_result(%Result*) local_unnamed_addr

; simple function which measures a single qubit and that's it.
define internal fastcc void @QuantumApplication__Run__body() unnamed_addr {

entry:
    br label %body

body:
    ; shot count variable
    %0 = phi i64 [ %2, %body ], [ 1, %entry ]

    ; measure a given qubit index
    tail call void @__quantum__qis__mz__body(%Qubit* nonnull inttoptr (i64 1 to %Qubit*), %Result* null)
    %1 = tail call i1 @__quantum__qir__read_result(%Result* null)

    ; shot count branch
    %2 = add nuw nsw i64 %0, 1
    %3 = icmp ult i64 %0, 42
    br i1 %3, label %body, label %exit

exit:
    ret void
}
```

This library will:

- Read and parse the LLVM bitcode
- Recurse through all functions called from the entrypoint. Within each function body:
  - Identify a shot-count loop with quantum instructions within any basic blocks traversed
  - Transform those basic blocks to instead send an equivalent Quil program for execution using the [QCS C SDK](https://github.com/rigetti/qcs-sdk-c).
- Output the resulting LLVM IR, which - if the input program matched the assumptions made by this library - no longer contains calls to QIR intrinsics and thus may be compiled for any supported architecture using QIR-unaware compilers such as `gcc` and `clang`.

After this process is complete, the above snippet might look like this (once disassembled):

```LLVM
; ModuleID = 'program.bc'
source_filename = "./test/fixtures/programs/measure.ll"

%Qubit = type opaque
%Result = type opaque
%Executable = type opaque
%ExecutionResult = type opaque

@parameter_memory_region_name = private unnamed_addr constant [12 x i8] c"__qir_param\00", align 1
@quantum_processor_id = private unnamed_addr constant [9 x i8] c"Aspen-10\00", align 1
@quil_program = private unnamed_addr constant [35 x i8] c"DECLARE ro BIT[0]\0AMEASURE 1 ro[0]\0A\00", align 1

declare void @__quantum__qis__h__body(%Qubit*) local_unnamed_addr

declare void @__quantum__qis__mz__body(%Qubit*, %Result*) local_unnamed_addr

declare i1 @__quantum__qir__read_result(%Result*) local_unnamed_addr

define internal fastcc void @QuantumApplication__Run__body() unnamed_addr {
entry:
  br label %body_execution

body:                                             ; preds = %body_execution, %body
  %0 = phi i64 [ %2, %body ], [ 1, %body_execution ]
  %1 = call i1 @get_readout_bit(%ExecutionResult* %5, i64 %0, i64 0)
  %2 = add nuw nsw i64 %0, 1
  %3 = icmp ult i64 %0, 42
  br i1 %3, label %body, label %exit

body_execution:                                   ; preds = %entry
  %4 = call %Executable* @executable_from_quil(i8* getelementptr inbounds ([35 x i8], [35 x i8]* @quil_program, i32 0, i32 0))
  call void @wrap_in_shots(%Executable* %4, i32 42)
  %5 = call %ExecutionResult* @execute_on_qpu(%Executable* %4, i8* getelementptr inbounds ([9 x i8], [9 x i8]* @quantum_processor_id, i32 0, i32 0))
  call void @panic_on_failure(%ExecutionResult* %5)
  br label %body

exit:                                             ; preds = %body
  ret void
}

declare %Executable* @executable_from_quil(i8*)

declare %ExecutionResult* @execute_on_qpu(%Executable*, i8*)

declare i1 @get_readout_bit(%ExecutionResult*, i64, i64)

declare void @panic_on_failure(%ExecutionResult*)

declare void @set_param(%Executable*, i8*, i32, double)

declare void @wrap_in_shots(%Executable*, i32)

define i32 @main() {
entry:
  call void @QuantumApplication__Run__body()
  ret i32 0
}
```

## Setup

This crate has the following external dependencies:

* [cargo-make](https://github.com/sagiegurari/cargo-make) as a task runner - install that with `cargo install cargo-make`.
* LLVM 11 installed and available on your `PATH`. If you can't run `llvm-config`, then you will be unable to build this crate. 
* A C compiler, such as `gcc` or `clang`, which supports LLVM 11 at a minimum. For OSX users, this means XCode version >= 12.5.
* The QCS SDK shared library, which may be built or downloaded as described [here](https://github.com/rigetti/qcs-sdk-c). (**IMPORTANT**: while in development, use the `7-execution-result-pointer` branch).

You'll also need to compile the shared "helper" library contained in the `helper` directory. This small shared library is used to reduce the complexity required within this crate's LLVM transformations.

```sh
cd helper
./build.sh
```

Build the CLI using `cargo build --bin`.

## Transform QIR

To transpile an input QIR program, run the CLI:

```
cargo run transform path/to/input.bc path/to/output.bc --main-entrypoint
```

Use the `--help` flag to view all options:

```
cargo run transform --help
```

## Run Your Transformed QIR

Now that you've got an output `.bc` file, you can inspect it by disassembling it (optional): `llvm-dis output.bc`, assuming that's what you named your output file in the CLI command above.

Next, compile it by linking to the two shared libraries in use: the helper library (here in this repo) and the QCS SDK (installed as part of [Setup](#setup)). Here's an example using `gcc`:

```sh
gcc -Lpath/to/qcs-sdk-c-shared-lib  -lqcs -L./helper -lhelper output.bc -o program
```

That produces an executable `program` which you can then run to execute your program on QCS. Happy computing!

## Troubleshooting

### Logging

Run the CLI with `RUST_LOG` set in order to view logs emitted during the transformation process. Values include the verbose `debug` as well as `info`, `warning`, and `error`.

### Rust compilation error: "No suitable version of LLVM..."

Example:

```
   Compiling llvm-sys v110.0.2
error: No suitable version of LLVM was found system-wide or pointed
       to by LLVM_SYS_110_PREFIX.

       Consider using `llvmenv` to compile an appropriate copy of LLVM, and
       refer to the llvm-sys documentation for more information.

       llvm-sys: https://crates.io/crates/llvm-sys
       llvmenv: https://crates.io/crates/llvmenv
   --> .../.cargo/registry/src/github.com-1ecc6299db9ec823/llvm-sys-110.0.2/src/lib.rs:486:1
    |
486 | / std::compile_error!(concat!(
487 | |     "No suitable version of LLVM was found system-wide or pointed
488 | |        to by LLVM_SYS_",
489 | |     env!("CARGO_PKG_VERSION_MAJOR"),
...   |
496 | |        llvmenv: https://crates.io/crates/llvmenv"
497 | | ));
    | |___^

```

First, make sure you do in fact have LLVM 11 installed and on your `PATH`. Run `llvm-config --version` to confirm. If not, that needs fixing first.

If you do, perhaps you first tried to build the crate before LLVM was installed and configured. Run `cargo clean -p llvm-sys` to clear the build and then retry.

### gcc compilation error: ld: library not found for -lhelper

```
$ gcc -L../qcs-sdk-c  -lqcs -L./helper -lhelper program.bc -o program
ld: library not found for -lhelper
```

Did you build the helper library in `./helper`? See [Setup](#setup).

### Runtime error: Library not loaded, image not found

On OSX, it might look like this:

```
$ ./program
dyld: Library not loaded: libhelper.dylib
  Referenced from: .../qcs-qir-sdk/./program
  Reason: image not found
```

The fix: set your `LD_LIBRARY_PATH` (and `DYLD_LIBRARY_PATH`) to include the relevant irectory:

```sh
export LD_LIBRARY_PATH=$LD_LIBRARY_PATH:/path/to/quil-qir/helper

# on OSX
export DYLD_LIBRARY_PATH=$LD_LIBRARY_PATH
```

### Runtime error: program hangs for 30 seconds on start

Make sure you're running the dependencies specified by the QCS SDK, namely `quilc`.