#!/usr/bin/env bash
# TSX integration tests
source "$(dirname "$0")/../../common.sh"

echo "TSX:"
run_test tractor test sample.tsx -x "program" --expect 1 -m "program element exists"
run_test tractor test sample.tsx -x "function[name]" --expect 1 -m "function declarations become function elements"
run_test tractor test sample.tsx -x "function[name='Greeting']" --expect 1 -m "component function is recognized"
run_test tractor test sample.tsx -x "interface" --expect 1 -m "interface declarations are recognized"
run_test tractor test sample.tsx -x "variable" --expect 1 -m "variable declarations renamed to variable"

# JSX-specific nodes
run_test tractor test sample.tsx -x "//jsx_element" --expect 4 -m "JSX elements are properly parsed"
run_test tractor test sample.tsx -x "//jsx_opening_element" --expect 4 -m "JSX opening elements present"
run_test tractor test sample.tsx -x "//jsx_closing_element" --expect 4 -m "JSX closing elements present"
run_test tractor test sample.tsx -x "//jsx_attribute" --expect 2 -m "JSX attributes are recognized"
run_test tractor test sample.tsx -x "//jsx_expression" --expect 5 -m "JSX expressions are recognized"
run_test tractor test sample.tsx -x "//jsx_text" --expect 5 -m "JSX text nodes are present"

report
