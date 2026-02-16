#!/usr/bin/env bash
# T-SQL integration tests
source "$(dirname "$0")/../common.sh"

echo "T-SQL:"
run_test tractor sample.sql --lang tsql -x "file" --expect 1 -m "program renamed to file"
run_test tractor sample.sql --lang tsql -x "statement" --expect 3 -m "three statements (SELECT, INSERT, DELETE)"
run_test tractor sample.sql --lang tsql -x "select" --expect 1 -m "SELECT statement"
run_test tractor sample.sql --lang tsql -x "insert" --expect 1 -m "INSERT statement"
run_test tractor sample.sql --lang tsql -x "delete" --expect 1 -m "DELETE statement"
run_test tractor sample.sql --lang tsql -x "where" --expect 2 -m "WHERE clauses"
run_test tractor sample.sql --lang tsql -x "compare[op='>']" --expect 1 -m "comparison operator extracted"
run_test tractor sample.sql --lang tsql -x "column" --expect 5 -m "column references"
run_test tractor sample.sql --lang tsql -x "comment" --expect 1 -m "SQL comment"
run_test tractor sample.sql --lang tsql -x "call" --expect 1 -m "function call (GETDATE)"

report
