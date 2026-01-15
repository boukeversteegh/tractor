#!/usr/bin/env bash
# JavaScript integration tests
source "$(dirname "$0")/../common.sh"

echo "JavaScript:"
run_test tractor sample.js -x "function" --expect 2 -m "function declarations become function elements"
run_test tractor sample.js -x "function[name='add']" --expect 1 -m "function names are directly queryable"
run_test tractor sample.js -x "function[name='main']" --expect 1 -m "main function is recognized"
run_test tractor sample.js -x "program" --expect 1 -m "program element exists"
run_test tractor sample.js -x "call" --expect 3 -m "call expressions renamed to call"

report
