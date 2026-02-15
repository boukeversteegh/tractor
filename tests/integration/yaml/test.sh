#!/usr/bin/env bash
# YAML integration tests
source "$(dirname "$0")/../common.sh"

echo "YAML:"
run_test tractor sample.yaml -x "//data/name[.='my-app']" --expect 1 -m "top-level scalar values are queryable"
run_test tractor sample.yaml -x "//data/database/host[.='localhost']" --expect 1 -m "nested mapping values are queryable"
run_test tractor sample.yaml -x "//data/database/port[.='5432']" --expect 1 -m "integer values are queryable as text"
run_test tractor sample.yaml -x "//data/database/credentials/username" --expect 1 -m "deeply nested mappings work"
run_test tractor sample.yaml -x "//data/servers" --expect 2 -m "sequences repeat parent key element"
run_test tractor sample.yaml -x "//data/servers[name='web-1']" --expect 1 -m "sequence items with nested mappings are queryable"
run_test tractor sample.yaml -x "//data/servers[name='web-1']/port[.='8080']" --expect 1 -m "can query nested values within sequence items"
run_test tractor sample.yaml -x "//data/features" --expect 3 -m "scalar sequences repeat parent key element"
run_test tractor sample.yaml -x "//data/features[.='auth']" --expect 1 -m "scalar sequence items have text content"
run_test tractor sample.yaml -x "//data/nested/level1/level2/value[.='deep']" --expect 1 -m "deep nesting is preserved"
run_test tractor sample.yaml -x "//data/flow_map/x[.='1']" --expect 1 -m "flow mappings are queryable"
run_test tractor sample.yaml -x "//data/flow_list" --expect 3 -m "flow sequences repeat parent key element"
run_test tractor sample.yaml -x "//data/quoted[.='hello world']" --expect 1 -m "quoted strings have quotes stripped"
run_test tractor sample.yaml -x "//data/multiline[contains(.,'line one')]" --expect 1 -m "block scalars are normalized"
run_test tractor sample.yaml -x "//data/first_name" --expect 1 -m "keys with spaces become sanitized element names"
run_test tractor sample.yaml -x "//data//*[key='first name']" --expect 1 -m "original key preserved as <key> child when sanitized"
run_test tractor sample.yaml -x "//data/first_name[text()='Alice']" --expect 1 -m "sanitized key values queryable via text()"

echo ""
echo "YAML (multi-document):"
run_test tractor multi.yaml -x "//data/document" --expect 3 -m "multi-document YAML creates separate document elements"
run_test tractor multi.yaml -x "//data/document[1]/name[.='doc1']" --expect 1 -m "first document content is queryable by position"
run_test tractor multi.yaml -x "//data/document[2]/name[.='doc2']" --expect 1 -m "second document content is queryable by position"
run_test tractor multi.yaml -x "//data/document[3]/value[.='three']" --expect 1 -m "third document values are queryable"
run_test tractor multi.yaml -x "//data//name" --expect 3 -m "descendant queries span all documents"

echo ""
echo "YAML (syntax branch):"
run_test tractor sample.yaml -x "//syntax/document/object" --expect 1 -m "syntax branch has document/object root"
run_test tractor sample.yaml -x "//syntax//property[key/string='name']/value/string[.='my-app']" --expect 1 -m "syntax key/value structure is queryable"

report
