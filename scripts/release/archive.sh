#!/usr/bin/env bash
set -xeo pipefail

source .release-env

# collect the QIR SDK binary
cp target/release/${SDK_BIN} ${DIST_ABS_PATH}

# collect the licenses & README
cp LICENSE ${DIST_ABS_PATH}
cat scripts/release/README.release.md | \
    sed "s/#TAG#/${TAG}/g" | \
    sed "s/#LLVM_VERSION#/${LLVM_VERSION}/g" \
    > ${DIST_ABS_PATH}/README.md

# collect the QCS C SDK shared library
pushd ${TMP_DEPS_ABS_PATH}/qcs-sdk-c
cp target/release/libqcs.${LIB_EXT} ${DIST_ABS_PATH}/lib
popd

# collect the SDK helper library
pushd helper
cp libhelper.${LIB_EXT} ${DIST_ABS_PATH}/lib
popd

# create the archive
tar -cvzf ${ARCHIVE} ${DIST_DIR}
ls -ll ${ARCHIVE}
shasum -a 256 ${ARCHIVE} > ${CHECKSUM}

# print location of pre-archive local release:
if [[ $CI != "true" ]]; then
    set +x
    echo "------------------------------------------"
    echo "Release build complete! See contents here:"
    echo "./${DIST_DIR}"
    echo ""
    echo "Archived contents here: ./${ARCHIVE}"
fi