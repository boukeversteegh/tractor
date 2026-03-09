#!/usr/bin/env bash
# Python integration tests
source "$(dirname "$0")/../common.sh"

echo "Python:"
run_test tractor test sample.py -x "function" --expect 3 -m "def statements become function elements"
run_test tractor test sample.py -x "function[name='add']" --expect 1 -m "function names are directly queryable"
run_test tractor test sample.py -x "function[name='main']" --expect 1 -m "main function is recognized"
run_test tractor test sample.py -x "module" --expect 1 -m "module element exists"
run_test tractor test sample.py -x "return" --expect 2 -m "return_statement renamed to return"
run_test tractor test sample.py -x "binary[op='+']" --expect 1 -m "operators extracted to op element"
run_test tractor test sample.py -x "call" --expect 3 -m "call expressions renamed to call"
run_test tractor test sample.py -x "function[async]" --expect 1 -m "async functions have <async/> marker"

# Test multiline string with trailing newlines - newlines must be preserved in XPath matching
# Note: tree-sitter normalizes CRLF to LF, so both files match with \n
run_test tractor test multiline-string-lf.py -x $'//string_content[.="hello\n\n"]' --expect 1 -m "can match multiline string with exact LF newlines"
run_test tractor test multiline-string-crlf.py -x $'//string_content[.="hello\n\n"]' --expect 1 -m "CRLF source normalized to LF by tree-sitter"

report
