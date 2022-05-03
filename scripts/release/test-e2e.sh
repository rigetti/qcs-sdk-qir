#!/usr/bin/env bash
set -eo pipefail

if [ ! -f .release-env ]; then
    echo "no .release-env setup file, please run:"
    echo "$ cargo make release-setup"
fi

source .release-env

if [ ! -d "$DIST_DIR" ]; then 
    echo "no release available to test, please run:"
    echo "$ cargo make release-quick"
fi

case $1 in
    "")
esac

TEST_TARGET=${1:-"qvm"}
TEST_INPUT=${2:-"tests/fixtures/programs/reduction.bc"}

case $TEST_TARGET in 
    "Aspen-11"|"Aspen-M-1"|"qvm")
    ;;

    *)
        echo "Invalid target '${TEST_TARGET}'. Use one of 'Aspen-11', 'Aspen-M-1', or 'qvm' (default)."
        exit 1
    ;;
esac

echo "Targeting: '${TEST_TARGET}'..."

case $TEST_TARGET in 
    "Aspen-11"|"Aspen-M-1")
        echo "Ensure your networking configuration allows connectivity to QPUs."
    ;;
esac

echo "Transforming '${TEST_INPUT}', compiling into executable."

case $OS in
    "darwin"|"linux")
        if [ $OS = "darwin" ]; then
            export DYLD_LIBRARY_PATH=$DYLD_LIBRARY_PATH:./lib
        else
            export LD_LIBRARY_PATH=$LD_LIBRARY_PATH:./lib
        fi

        pushd $DIST_DIR
        ./qcs-sdk-qir transform --add-main-entrypoint --target $TEST_TARGET ../../$TEST_INPUT output.bc
        clang -Llib -lqcs -Llib -lhelper output.bc -o program
        ./program
        popd
    ;;

    *)
        echo "Unsupported system platform: '${OS}'"
    ;;
esac