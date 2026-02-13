#!/usr/bin/env bash
# Tests for --string / -s flag (pass source code as argument instead of stdin)
source "$(dirname "$0")/../common.sh"

echo "String input (--string / -s):"

# Basic --string usage with different languages
run_test tractor --string "fn add(a: i32, b: i32) -> i32 { a + b }" -l rust -x "function" --expect 1 -m "string flag works with rust"
run_test tractor --string "def hello(): pass" -l python -x "function" --expect 1 -m "string flag works with python"
run_test tractor --string "public class Foo { public void Bar() {} }" -l csharp -x "class" --expect 1 -m "string flag works with csharp"
run_test tractor --string "function greet() { return 'hi'; }" -l javascript -x "function" --expect 1 -m "string flag works with javascript"
run_test tractor --string "const greet = (): string => 'hi';" -l typescript -x "lambda" --expect 1 -m "string flag works with typescript"

# Short flag -s
run_test tractor -s "fn main() {}" -l rust -x "function" --expect 1 -m "short flag -s works"

# --expect integration
run_test tractor -s "fn a() {} fn b() {}" -l rust -x "function" --expect 2 -m "expect exact count with -s"
run_test tractor -s "fn a() {} fn b() {}" -l rust -x "function" --expect some -m "expect some with -s"
run_test tractor -s "let x = 1;" -l rust -x "function" --expect none -m "expect none with -s"

# Output formats
run_test tractor -s "class Foo { }" -l csharp -x "class/name" -o value --expect 1 -m "output value with -s"
run_test tractor -s "class Foo { }" -l csharp -x "class" -o count --expect 1 -m "output count with -s"
run_test tractor -s "class Foo { }" -l csharp -x "class" -o gcc --expect 1 -m "output gcc with -s"

# Without xpath (full AST output) - should succeed without error
run_test tractor -s "let x = 1;" -l rust -o count --expect 1 -m "string without xpath outputs AST"

# Error: --string without --lang should fail
if tractor --string "let x = 1;" 2>/dev/null; then
    echo "  ✗ --string without --lang should fail"
    ((FAILED++))
else
    echo "  ✓ --string without --lang correctly fails"
    ((PASSED++))
fi

report
