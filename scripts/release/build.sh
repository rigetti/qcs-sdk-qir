#!/usr/bin/env bash
set -xeo pipefail

source .release-env

# build the QIR SDK binary
if [[ $NO_QIR_SDK_BUILD -eq 0 ]]; then
    cargo build --bin ${SDK_BIN} --release --features llvm${LLVM_VERSION}-0
else 
    echo "Skipping QIR SDK build."
fi

# build the QCS C SDK shared library
if [[ $NO_C_SDK_BUILD -eq 0 ]]; then
    rm -rf ${TMP_DEPS_ABS_PATH}/qcs-sdk-c
    git clone https://github.com/rigetti/qcs-sdk-c ${TMP_DEPS_ABS_PATH}/qcs-sdk-c
    pushd ${TMP_DEPS_ABS_PATH}/qcs-sdk-c
    git fetch --all --tags
    git checkout tags/${C_SDK_VERSION} -b build-qir-sdk-${TAG}
    cargo build --release
    popd
else 
    echo "Skipping C SDK build."
fi

if [[ $NO_HELPER_LIB_BUILD -eq 0 ]]; then
# build the SDK helper library
pushd helper
cp helper.h helper.c
clang -c -o libhelper.o helper.c -fPIC
clang -shared -L${TMP_DEPS_ABS_PATH}/qcs-sdk-c/target/release -lqcs -o libhelper.$LIB_EXT libhelper.o
popd
else 
    echo "Skipping helper lib build."
fi