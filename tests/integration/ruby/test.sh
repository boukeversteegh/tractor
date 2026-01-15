#!/usr/bin/env bash
# Ruby integration tests
source "$(dirname "$0")/../common.sh"

echo "Ruby:"
run_test tractor sample.rb -x "method" --expect 2 -m "def statements become method elements"
run_test tractor sample.rb -x "method[name='add']" --expect 1 -m "method names are directly queryable"
run_test tractor sample.rb -x "method[name='main']" --expect 1 -m "main method is recognized"
run_test tractor sample.rb -x "call" --expect 2 -m "method calls renamed to call"

report
