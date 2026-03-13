#!/usr/bin/env bash
# T-SQL integration tests
source "$(dirname "$0")/../../common.sh"

echo "T-SQL:"

# Basic structure
run_test tractor test sample.sql --lang tsql -x "file" --expect 1 -m "program renamed to file"
run_test tractor test sample.sql --lang tsql -x "statement" --expect 24 -m "statements"

# DML statements
run_test tractor test sample.sql --lang tsql -x "select" --expect 17 -m "SELECT (including subqueries, CTEs, unions)"
run_test tractor test sample.sql --lang tsql -x "insert" --expect 1 -m "INSERT statement"
run_test tractor test sample.sql --lang tsql -x "delete" --expect 1 -m "DELETE statement"
run_test tractor test sample.sql --lang tsql -x "update" --expect 3 -m "UPDATE statements (basic + transaction)"

# Clauses
run_test tractor test sample.sql --lang tsql -x "where" --expect 14 -m "WHERE clauses"
run_test tractor test sample.sql --lang tsql -x "order_by" --expect 3 -m "ORDER BY clauses"
run_test tractor test sample.sql --lang tsql -x "group_by" --expect 1 -m "GROUP BY clause"
run_test tractor test sample.sql --lang tsql -x "having" --expect 1 -m "HAVING clause"

# JOINs
run_test tractor test sample.sql --lang tsql -x "join" --expect 2 -m "JOIN clauses (INNER + LEFT)"

# Subqueries and CTEs
run_test tractor test sample.sql --lang tsql -x "subquery" --expect 2 -m "subqueries (IN + EXISTS)"
run_test tractor test sample.sql --lang tsql -x "exists" --expect 1 -m "EXISTS predicate"
run_test tractor test sample.sql --lang tsql -x "cte" --expect 1 -m "CTE (WITH ... AS)"
run_test tractor test sample.sql --lang tsql -x "union" --expect 1 -m "UNION ALL"

# Expressions
run_test tractor test sample.sql --lang tsql -x "case" --expect 1 -m "CASE WHEN expression"
run_test tractor test sample.sql --lang tsql -x "between" --expect 1 -m "BETWEEN expression"
run_test tractor test sample.sql --lang tsql -x "compare[op='>']" --expect 4 -m "comparison > operator"
run_test tractor test sample.sql --lang tsql -x "compare[op='>=']" --expect 1 -m "comparison >= operator"

# Functions
run_test tractor test sample.sql --lang tsql -x "call" --expect 9 -m "function calls (GETDATE, COUNT, AVG, etc.)"
run_test tractor test sample.sql --lang tsql -x "cast" --expect 1 -m "CAST expression"
run_test tractor test sample.sql --lang tsql -x "window" --expect 1 -m "window function (ROW_NUMBER OVER)"
run_test tractor test sample.sql --lang tsql -x "partition_by" --expect 1 -m "PARTITION BY"
run_test tractor test sample.sql --lang tsql -x "star" --expect 2 -m "* wildcard (COUNT(*))"

# Identifiers
run_test tractor test sample.sql --lang tsql -x "alias" --expect 17 -m "aliases (AS + table aliases)"
run_test tractor test sample.sql --lang tsql -x "schema" --expect 4 -m "schema references (dbo)"
run_test tractor test sample.sql --lang tsql -x "var" --expect 6 -m "@variables"
run_test tractor test sample.sql --lang tsql -x "temp_ref" --expect 1 -m "#TempUsers temp table"
run_test tractor test sample.sql --lang tsql -x "direction" --expect 2 -m "sort direction (DESC)"

# DDL
run_test tractor test sample.sql --lang tsql -x "create_table" --expect 1 -m "CREATE TABLE"
run_test tractor test sample.sql --lang tsql -x "col_def" --expect 3 -m "column definitions"
run_test tractor test sample.sql --lang tsql -x "create_function" --expect 1 -m "CREATE FUNCTION"

# DML advanced
run_test tractor test sample.sql --lang tsql -x "assign" --expect 4 -m "assignments (UPDATE SET + MERGE)"
run_test tractor test sample.sql --lang tsql -x "when" --expect 2 -m "MERGE WHEN clauses"

# Transaction and batching
run_test tractor test sample.sql --lang tsql -x "transaction" --expect 1 -m "BEGIN TRANSACTION / COMMIT"
run_test tractor test sample.sql --lang tsql -x "set" --expect 1 -m "SET @variable"
run_test tractor test sample.sql --lang tsql -x "go" --expect 1 -m "GO batch separator"
run_test tractor test sample.sql --lang tsql -x "exec" --expect 1 -m "EXEC stored procedure"

# Comments
run_test tractor test sample.sql --lang tsql -x "comment" --expect 20 -m "SQL comments"

report
