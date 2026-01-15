#!/usr/bin/env bash
# TypeScript integration tests
source "$(dirname "$0")/../common.sh"

echo "TypeScript:"
run_test tractor sample.ts -x "function" --expect 2 -m "function declarations become function elements"
run_test tractor sample.ts -x "function[name='add']" --expect 1 -m "function names are directly queryable"
run_test tractor sample.ts -x "function[name='main']" --expect 1 -m "main function is recognized"
run_test tractor sample.ts -x "program" --expect 1 -m "program element exists"
run_test tractor sample.ts -x "variable" --expect 1 -m "variable declarations renamed to variable"
run_test tractor sample.ts -x "binary[op='+']" --expect 1 -m "operators extracted to op element"
run_test tractor sample.ts -x "call" --expect 3 -m "call expressions renamed to call"

report
