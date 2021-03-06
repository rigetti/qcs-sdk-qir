#!/usr/bin/env bash
set -eo pipefail

# initial validation for setup environment variables and a release to use for the test
if [ ! -f .release-env ]; then
    echo "no .release-env setup file, please run:"
    echo "$ cargo make release-setup"
    exit 1
fi

source .release-env

if [ ! -d "$DIST_DIR" ]; then 
    echo "no release available to test, please run:"
    echo "$ cargo make release-quick"
    exit 1
fi

# set script variables or use defaults
TEST_TARGET=${1:-"qvm"}
TEST_INPUT=${2:-"tests/fixtures/programs/reduction.bc"}

# consider that the input path may be relative, and if so, always prefix it with two levels up
if [[ "$TEST_INPUT" != /* ]]; then
    TEST_INPUT="../../${TEST_INPUT}"
fi

# validate target is an actual option
case $TEST_TARGET in 
    "Aspen-11"|"qvm")
    ;;

    *)
        echo "Invalid target '${TEST_TARGET}'. Use one of 'Aspen-11' or 'qvm' (default)."
        exit 1
    ;;
esac

echo "Targeting: '${TEST_TARGET}'..."

case $TEST_TARGET in 
    "Aspen-11")
        echo "Ensure your networking configuration allows connectivity to QPUs."
    ;;
esac

# transform and compile the program, then run against specified target
case $OS in
    "darwin"|"linux")
        if [ $OS = "darwin" ]; then
            RESTORE_LIBRARY_PATH=$DYLD_LIBRARY_PATH
            export DYLD_LIBRARY_PATH=$DYLD_LIBRARY_PATH:./lib
        else
            RESTORE_LIBRARY_PATH=$LD_LIBRARY_PATH
            export LD_LIBRARY_PATH=$LD_LIBRARY_PATH:./lib
        fi
                
        pushd $DIST_DIR
        echo "Transforming: '${TEST_INPUT}' -> '${DIST_DIR}/output.bc'"
        ./qcs-sdk-qir transform --add-main-entrypoint --target $TEST_TARGET $TEST_INPUT output.bc
        echo "Compiling: '${DIST_DIR}/program'"
        clang -Llib -lqcs -Llib -lhelper output.bc -o program
        echo "Executing: '${DIST_DIR}/program'"
        ./program
        popd
        
        
        if [ $OS = "darwin" ]; then
            export DYLD_LIBRARY_PATH=$RESTORE_LIBRARY_PATH
        else
            export LD_LIBRARY_PATH=$RESTORE_LIBRARY_PATH
        fi
    ;;

    *)
        echo "Unsupported system platform: '${OS}'"
    ;;
esac