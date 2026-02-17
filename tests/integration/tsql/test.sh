#!/usr/bin/env bash
# T-SQL integration tests
source "$(dirname "$0")/../common.sh"

echo "T-SQL:"
run_test tractor sample.sql --lang tsql -x "file" --expect 1 -m "program renamed to file"
run_test tractor sample.sql --lang tsql -x "statement" --expect 5 -m "five statements (SELECT, INSERT, DELETE, SELECT, UPDATE)"
run_test tractor sample.sql --lang tsql -x "select" --expect 2 -m "SELECT statements"
run_test tractor sample.sql --lang tsql -x "insert" --expect 1 -m "INSERT statement"
run_test tractor sample.sql --lang tsql -x "delete" --expect 1 -m "DELETE statement"
run_test tractor sample.sql --lang tsql -x "update" --expect 1 -m "UPDATE statement"
run_test tractor sample.sql --lang tsql -x "where" --expect 4 -m "WHERE clauses"
run_test tractor sample.sql --lang tsql -x "compare[op='>']" --expect 1 -m "comparison operator extracted"
run_test tractor sample.sql --lang tsql -x "comment" --expect 3 -m "SQL comments"
run_test tractor sample.sql --lang tsql -x "call" --expect 1 -m "function call (GETDATE)"
run_test tractor sample.sql --lang tsql -x "alias" --expect 2 -m "column alias (AS UserName) and table alias (u)"
run_test tractor sample.sql --lang tsql -x "schema" --expect 3 -m "schema references (dbo)"
run_test tractor sample.sql --lang tsql -x "var" --expect 3 -m "@variables (@StartDate, @NewValue, @KeyName)"
run_test tractor sample.sql --lang tsql -x "between" --expect 1 -m "BETWEEN expression"
run_test tractor sample.sql --lang tsql -x "assign" --expect 1 -m "assignment (SET Value = @NewValue)"

report
