#!/usr/bin/env bash
# INI integration tests
source "$(dirname "$0")/../common.sh"

echo "INI:"
run_test tractor sample.ini -x "//name[.='my-app']" --expect 1 -m "global settings are queryable"
run_test tractor sample.ini -x "//version[.='1.0.0']" --expect 1 -m "global values are queryable"
run_test tractor sample.ini -x "//database/host[.='localhost']" --expect 1 -m "section settings are queryable"
run_test tractor sample.ini -x "//database/port[.='5432']" --expect 1 -m "numeric values are queryable as text"
run_test tractor sample.ini -x "//database/enabled[.='true']" --expect 1 -m "boolean values are queryable as text"
run_test tractor sample.ini -x "//database.credentials/username[.='admin']" --expect 1 -m "dotted section names are preserved"
run_test tractor sample.ini -x "//database.credentials/password[.='secret']" --expect 1 -m "dotted section values are queryable"
run_test tractor sample.ini -x "//servers/count[.='2']" --expect 1 -m "simple sections are queryable"
run_test tractor sample.ini -x "//paths/home[.='/usr/local']" --expect 1 -m "path values are preserved"
run_test tractor sample.ini -x "//paths/temp[.='/tmp']" --expect 1 -m "multiple settings in section are queryable"
run_test tractor sample.ini -x "//comment" --expect 2 -m "comments are preserved"
run_test tractor sample.ini -x "//comment[.='Global settings']" --expect 1 -m "comment text is queryable"
run_test tractor sample.ini -x "//document" --expect 1 -m "document root element is present"

report
