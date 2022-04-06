# Rigetti QIR SDK

Thank you for downloading the qcs-sdk-qir toolkit. In this release, you should find the following files:

- `qcs-sdk-qir`: an executable binary used to transform QIR programs
- `lib/libhelper.{dylib,so}`: a shared library to ease the use of the QCS SDK 
- `lib/libqcs.{dylib,so}`: a shared library to handle communication between your QIR program and Rigetti's Quantum Cloud Services

## Usage

In order to transform QIR programs, please follow these steps (take note of platform-specificity):

### Linux

```bash 
# verify the download:
shasum -c qcs-sdk-qir-llvm12-darwin-x86_64-v0.0.0-local.checksum.txt

# exract the compressed archive
tar xzf qcs-sdk-qir-llvm12-darwin-x86_64-v0.0.0-local.tar.gz

# enter the dist directory to find the release artifacts
cd qcs-sdk-qir-llvm12-darwin-x86_64-v0.0.0-local/dist

# export the library paths so the linker can find them:
export LD_LIBRARY_PATH=$LD_LIBRARY_PATH:./lib

# transform a QIR program to contain Rigetti's requirements:
# hint: qcs-sdk-qir --help (for all options)
qcs-sdk-qir transform --add-main-entrypoint input.bc output.bc

# compile the transformed program to an executable
clang -Llib -lqcs -Llib -lhelper output.bc -o program

# execute your program
./program
```

### MacOS

```bash
# verify the download:
shasum -c qcs-sdk-qir-llvm12-darwin-x86_64-v0.0.0-local.checksum.txt

# exract the compressed archive
tar xzf qcs-sdk-qir-llvm12-darwin-x86_64-v0.0.0-local.tar.gz

# enter the dist directory to find the release artifacts
cd qcs-sdk-qir-llvm12-darwin-x86_64-v0.0.0-local/dist

# you may need to remove the quarantined attribute from the binary and shared libraries
sudo xattr -r -d com.apple.quarantine qcs-sdk-qir lib/*

# export the library paths so the linker can find them:
export DYLD_LIBRARY_PATH=$DYLD_LIBRARY_PATH:./lib

# transform a QIR program to contain Rigetti's requirements:
# hint: qcs-sdk-qir --help (for all options)
qcs-sdk-qir transform --add-main-entrypoint input.bc output.bc

# compile the transformed program to an executable
clang -Llib -lqcs -Llib -lhelper output.bc -o program

# execute your program
./program
```