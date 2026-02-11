#!/usr/bin/env bash
# YAML integration tests
source "$(dirname "$0")/../common.sh"

echo "YAML:"
run_test tractor sample.yaml -x "//name[.='my-app']" --expect 1 -m "top-level scalar values are queryable"
run_test tractor sample.yaml -x "//database/host[.='localhost']" --expect 1 -m "nested mapping values are queryable"
run_test tractor sample.yaml -x "//database/port[.='5432']" --expect 1 -m "integer values are queryable as text"
run_test tractor sample.yaml -x "//database/credentials/username" --expect 1 -m "deeply nested mappings work"
run_test tractor sample.yaml -x "//servers/item" --expect 2 -m "sequence items become item elements"
run_test tractor sample.yaml -x "//servers/item[name='web-1']" --expect 1 -m "sequence items with nested mappings are queryable"
run_test tractor sample.yaml -x "//servers/item[name='web-1']/port[.='8080']" --expect 1 -m "can query nested values within sequence items"
run_test tractor sample.yaml -x "//features/item" --expect 3 -m "scalar sequences become item elements"
run_test tractor sample.yaml -x "//features/item[.='auth']" --expect 1 -m "scalar sequence items have text content"
run_test tractor sample.yaml -x "//nested/level1/level2/value[.='deep']" --expect 1 -m "deep nesting is preserved"
run_test tractor sample.yaml -x "//flow_map/x[.='1']" --expect 1 -m "flow mappings are queryable"
run_test tractor sample.yaml -x "//flow_list/item" --expect 3 -m "flow sequences become item elements"
run_test tractor sample.yaml -x "//quoted[.='hello world']" --expect 1 -m "quoted strings have quotes stripped"
run_test tractor sample.yaml -x "//multiline[contains(.,'line one')]" --expect 1 -m "block scalars are normalized"
run_test tractor sample.yaml -x "//first_name" --expect 1 -m "keys with spaces become sanitized element names"
run_test tractor sample.yaml -x "//*[key='first name']" --expect 1 -m "original key preserved as <key> child when sanitized"
run_test tractor sample.yaml -x "//first_name[text()='Alice']" --expect 1 -m "sanitized key values queryable via text()"

report
