#!/usr/bin/env bash
# XML passthrough integration tests
source "$(dirname "$0")/../common.sh"

echo "XML Passthrough:"
# Note: Use queries without // prefix - tractor auto-adds it, and mingw mangles //
run_test tractor sample.xml -x "item" --expect 3 -m "finds all item elements"
run_test tractor sample.xml -x "item[@type='feature']" --expect 2 -m "filters by attribute"
run_test tractor sample.xml -x "item[@type='bug']" --expect 1 -m "finds bug items"
run_test tractor sample.xml -x "setting" --expect 2 -m "finds setting elements"
run_test tractor sample.xml -x "item[status='complete']" --expect 1 -m "filters by child element"
run_test tractor sample.xml -x "project/@name" --expect 1 -m "queries attributes"
run_test tractor sample.xml -x "name" --expect 3 -m "finds all name elements"
run_test tractor sample.xml -x "item/name" -o value --expect some -m "value output works"

report
