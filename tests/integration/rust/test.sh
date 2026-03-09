#!/usr/bin/env bash
# Rust integration tests
source "$(dirname "$0")/../common.sh"

echo "Rust:"
run_test tractor sample.rs -x "function" --expect 4 -m "function elements exist for fn declarations"
run_test tractor sample.rs -x "function[name='add']" --expect 1 -m "function names are directly queryable"
run_test tractor sample.rs -x "function[name='main']" --expect 1 -m "main function is recognized"
run_test tractor sample.rs -x "file" --expect 1 -m "source_file renamed to file"
run_test tractor sample.rs -x "let" --expect 1 -m "let_declaration renamed to let"
run_test tractor sample.rs -x "binary[op='+']" --expect 1 -m "operators extracted to op element"
run_test tractor sample.rs -x "call" --expect 1 -m "call_expression renamed to call"
run_test tractor sample.rs -x "macro" --expect 1 -m "macro_invocation renamed to macro"
run_test tractor sample.rs -x "function[pub]" --expect 3 -m "all public functions have a <pub> marker"
run_test tractor sample.rs -x "function[pub[not(*)]]" --expect 1 -m "plain pub functions have empty <pub/>"
run_test tractor sample.rs -x "function[pub[crate]]" --expect 1 -m "pub(crate) functions have <pub><crate/></pub>"
run_test tractor sample.rs -x "function[private]" --expect 1 -m "private functions have <private/> marker"

report
