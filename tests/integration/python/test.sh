#!/usr/bin/env bash
# Python integration tests
source "$(dirname "$0")/../common.sh"

echo "Python:"
run_test tractor sample.py -x "function" --expect 2 -m "def statements become function elements"
run_test tractor sample.py -x "function[name='add']" --expect 1 -m "function names are directly queryable"
run_test tractor sample.py -x "function[name='main']" --expect 1 -m "main function is recognized"
run_test tractor sample.py -x "module" --expect 1 -m "module element exists"
run_test tractor sample.py -x "return" --expect 1 -m "return_statement renamed to return"
run_test tractor sample.py -x "binary[op='+']" --expect 1 -m "operators extracted to op element"
run_test tractor sample.py -x "call" --expect 3 -m "call expressions renamed to call"

report
