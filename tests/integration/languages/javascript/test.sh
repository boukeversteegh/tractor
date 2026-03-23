#!/usr/bin/env bash
# JavaScript integration tests
source "$(dirname "$0")/../../common.sh"

echo "JavaScript:"
run_test tractor test sample.js -x "function[name]" --expect 2 -m "function declarations become function elements"
run_test tractor test sample.js -x "function[name='add']" --expect 1 -m "function names are directly queryable"
run_test tractor test sample.js -x "function[name='main']" --expect 1 -m "main function is recognized"
run_test tractor test sample.js -x "program" --expect 1 -m "program element exists"
run_test tractor test sample.js -x "call" --expect 3 -m "call expressions renamed to call"

# Call structure: function references should not be <type>
run_test tractor test sample.js -x "call/function" --expect 3 -m "call has function child for callee"
run_test tractor test sample.js -x "call/function[ref]" --expect 2 -m "direct calls have ref marker"
run_test tractor test sample.js -x "call/function/member" --expect 1 -m "member call has member inside function"

# Member expression structure: object/property roles should be distinct
run_test tractor test sample.js -x "member/object" --expect 1 -m "member has object child"
run_test tractor test sample.js -x "member/property" --expect 1 -m "member has property child"

report
