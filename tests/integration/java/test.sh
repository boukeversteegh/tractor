#!/usr/bin/env bash
# Java integration tests
source "$(dirname "$0")/../common.sh"

echo "Java:"
run_test tractor sample.java -x "method" --expect 2 -m "method declarations become method elements"
run_test tractor sample.java -x "method[name='add']" --expect 1 -m "method names are directly queryable"
run_test tractor sample.java -x "class[name='Sample']" --expect 1 -m "class names are directly queryable"
run_test tractor sample.java -x "program" --expect 1 -m "program element exists"
run_test tractor sample.java -x "static" --expect 2 -m "static modifier extracted"
run_test tractor sample.java -x "binary[op='+']" --expect 2 -m "operators extracted to op element"
run_test tractor sample.java -x "call" --expect 2 -m "method invocations renamed to call"

report
