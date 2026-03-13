#!/usr/bin/env bash
# Java integration tests
source "$(dirname "$0")/../../common.sh"

echo "Java:"
run_test tractor test sample.java -x "method" --expect 5 -m "method declarations become method elements"
run_test tractor test sample.java -x "method[name='add']" --expect 1 -m "method names are directly queryable"
run_test tractor test sample.java -x "class[name='Sample']" --expect 1 -m "class names are directly queryable"
run_test tractor test sample.java -x "program" --expect 1 -m "program element exists"
run_test tractor test sample.java -x "static" --expect 2 -m "static modifier extracted"
run_test tractor test sample.java -x "binary[op='+']" --expect 2 -m "operators extracted to op element"
run_test tractor test sample.java -x "call" --expect 3 -m "method invocations renamed to call"
run_test tractor test sample.java -x "//method[public]" --expect 2 -m "public methods have <public/> marker"
run_test tractor test sample.java -x "//method[package-private]" --expect 2 -m "package-private methods have marker (no explicit modifier)"
run_test tractor test sample.java -x "//method[protected]" --expect 1 -m "protected methods have <protected/> marker"

report
