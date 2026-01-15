#!/usr/bin/env bash
# Go integration tests
source "$(dirname "$0")/../common.sh"

echo "Go:"
run_test tractor sample.go -x "function" --expect 2 -m "func declarations become function elements"
run_test tractor sample.go -x "function[name='add']" --expect 1 -m "function names are directly queryable"
run_test tractor sample.go -x "function[name='main']" --expect 1 -m "main function is recognized"
run_test tractor sample.go -x "file" --expect 1 -m "source_file renamed to file"
run_test tractor sample.go -x "package" --expect 1 -m "package_clause renamed to package"
run_test tractor sample.go -x "binary[op='+']" --expect 1 -m "operators extracted to op element"
run_test tractor sample.go -x "call" --expect 2 -m "call expressions renamed to call"

report
