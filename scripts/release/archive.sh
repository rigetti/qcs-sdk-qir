#!/usr/bin/env bash
set -xeo pipefail

source .release-env

# collect the QIR SDK binary
cp target/release/${SDK_BIN} ${DIST_ABS_PATH}

# collect the licenses & README
cp LICENSE ${DIST_ABS_PATH}
cp scripts/release/README.release.md scripts/release/README.release.md.local
sed -i -- "s/#TAG#/${TAG}/g" scripts/release/README.release.md
sed -i -- "s/#LLVM_VERSION#/${LLVM_VERSION}/g" scripts/release/README.release.md
cp scripts/release/README.release.md ${DIST_ABS_PATH}/README.md
rm -f scripts/release/README.release.md--
mv scripts/release/README.release.md.local scripts/release/README.release.md

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