#!/usr/bin/env bash

DELETE=(
    "dist" 
    "tmp-deps-build" 
    ".release-env"
    $(echo qcs-sdk-qir-llvm*) 
    $(echo helper/libhelper.*) 
    "helper/helper.c"
)

echo "The following will be deleted:"
echo ""
printf "    %s\n" "${DELETE[@]}"
echo ""
echo "Continue? (y/n)"
read CONFIRM

if [[ $CONFIRM = "y" ]]; then
    for f in "${DELETE[@]}"; do
        rm -rf $f
    done
else 
    echo "Clean cancelled."
fi