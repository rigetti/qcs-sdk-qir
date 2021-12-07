#!/bin/sh

cp helper.h helper.c
clang -L../../qcs-sdk-c -lqcs -dynamiclib helper.c -current_version 1.0 -compatibility_version 1.0 -o libhelper.dylib
rm helper.c