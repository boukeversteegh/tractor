#!/usr/bin/env bash
# TOML integration tests
source "$(dirname "$0")/../common.sh"

echo "TOML:"
run_test tractor sample.toml -x "//title[.='My App']" --expect 1 -m "top-level string values are queryable"
run_test tractor sample.toml -x "//version[.='1.0.0']" --expect 1 -m "top-level values are queryable"
run_test tractor sample.toml -x "//database/host[.='localhost']" --expect 1 -m "table values are queryable"
run_test tractor sample.toml -x "//database/port[.='5432']" --expect 1 -m "integer values are queryable as text"
run_test tractor sample.toml -x "//database/enabled[.='true']" --expect 1 -m "boolean values are queryable as text"
run_test tractor sample.toml -x "//database/credentials/username" --expect 1 -m "dotted table keys create nested elements"
run_test tractor sample.toml -x "//database/credentials/password[.='secret']" --expect 1 -m "dotted table values are queryable"
run_test tractor sample.toml -x "//servers/item" --expect 2 -m "table array elements become item elements"
run_test tractor sample.toml -x "//servers/item[name='web-1']" --expect 1 -m "table array items with nested values are queryable"
run_test tractor sample.toml -x "//servers/item[name='web-1']/port[.='8080']" --expect 1 -m "can query nested values within table array items"
run_test tractor sample.toml -x "//features/item" --expect 3 -m "array values become item elements"
run_test tractor sample.toml -x "//features/item[.='auth']" --expect 1 -m "array items have text content"
run_test tractor sample.toml -x "//inline/x[.='1']" --expect 1 -m "inline tables are queryable"
run_test tractor sample.toml -x "//inline/y[.='2']" --expect 1 -m "inline table values are queryable"
run_test tractor sample.toml -x "//quoted[.='hello world']" --expect 1 -m "quoted strings have quotes stripped"
run_test tractor sample.toml -x "//first_name" --expect 1 -m "quoted keys become sanitized element names"
run_test tractor sample.toml -x "//*[@key='first name']" --expect 1 -m "original key preserved as @key attribute when sanitized"
run_test tractor sample.toml -x "//nested/level1/level2/value[.='deep']" --expect 1 -m "deeply dotted table keys create nested elements"
run_test tractor sample.toml -x "//document" --expect 1 -m "document root element is present"

report
