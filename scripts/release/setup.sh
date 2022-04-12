#!/usr/bin/env bash
set -xeo pipefail

# set defaults for dynamic CI variables when local/testing
C_SDK_VERSION="${C_SDK_VERSION:-v0.1.0}"
LLVM_VERSION="${LLVM_VERSION:-13}"
LLVM_FULL_VERSION="${LLVM_FULL_VERSION:-0.1}"
TESTING_TAG="${TESTING_TAG:-v0.0.0-local}"
TAG=${TAG:-$TESTING_TAG}
TAG=${TAG/\//-} # replace occurances of "/" with "-" (for test builds with unideal branch name)
CI=${CI:-false}

# assign variables, will be exported to CI env, or sourced to local shell
SDK_BIN=qcs-sdk-qir
ARCH=$(uname -m)
OS=$(uname -s | awk '{print tolower($0)}')
RELEASE=${SDK_BIN}-llvm${LLVM_VERSION}-${OS}-${ARCH}-${TAG}  
DIST_DIR=${RELEASE}/dist
DIST_ABS_PATH=$(pwd)/${DIST_DIR}
TMP_DEPS_ABS_PATH=$(pwd)/tmp-deps-build
ARCHIVE=${RELEASE}.tar.gz
CHECKSUM=${RELEASE}.checksum.txt

case $OS in
    "darwin")
        LIB_EXT="dylib"
        ;;
    "linux")
        LIB_EXT="so"
        ;;
    *)
        echo "ERROR: unable to build release artifacts for OS: ${OS}"
        exit 1
        ;;
esac

# create paths to build deps
mkdir -p ${TMP_DEPS_ABS_PATH}

# create paths to collect artifacts
mkdir -p ${DIST_ABS_PATH}/lib

rm -f .release-env # remove file if it exists
echo "C_SDK_VERSION=${C_SDK_VERSION}" >> .release-env
echo "LLVM_VERSION=${LLVM_VERSION}" >> .release-env
echo "TESTING_TAG=${TESTING_TAG}" >> .release-env
echo "TAG=${TAG}" >> .release-env
echo "SDK_BIN=${SDK_BIN}" >> .release-env
echo "ARCH=${ARCH}" >> .release-env
echo "OS=${OS}" >> .release-env
echo "RELEASE=${RELEASE}" >> .release-env
echo "DIST_DIR=${DIST_DIR}" >> .release-env
echo "DIST_ABS_PATH=${DIST_ABS_PATH}" >> .release-env
echo "TMP_DEPS_ABS_PATH=${TMP_DEPS_ABS_PATH}" >> .release-env
echo "ARCHIVE=${ARCHIVE}" >> .release-env
echo "CHECKSUM=${CHECKSUM}" >> .release-env
echo "LIB_EXT=${LIB_EXT}" >> .release-env

# special treatment for macos-latest github runner clang environment
if [[ $OS = "darwin" && $CI = "true" ]]; then
    echo "SDKROOT=$(xcrun --sdk macosx --show-sdk-path)" >> .release-env
    echo "CPATH=$LLVM_PATH/lib/clang/$LLVM_FULL_VERSION/include/" >> .release-env
    echo "LDFLAGS=-L$LLVM_PATH/lib" >> .release-env
    echo "CPPFLAGS=-I$LLVM_PATH/include" >> .release-env
    echo "CC=$LLVM_PATH/bin/clang" >> .release-env
    echo "CXX=$LLVM_PATH/bin/clang++" >> .release-env
fi

# print out all env variables
cat .release-env
