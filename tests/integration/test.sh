#!/usr/bin/env bash
# Integration tests for tractor - Uses tractor's native test output (--expect --message)

set -uo pipefail

cd "$(dirname "$0")/../.."

TRACTOR="./target/release/tractor"
FIXTURES="tests/integration/fixtures"

BLUE='\033[0;34m'
NC='\033[0m'

PASSED=0
FAILED=0

# Run tractor with test output and track results
run_test() {
    if "$@" 2>/dev/null; then
        ((PASSED++))
    else
        ((FAILED++))
    fi
}

# Build if needed
[ -f "$TRACTOR" ] || cargo build --release -q

echo -e "${BLUE}Tractor Integration Tests${NC}"
echo ""

# Rust
echo "Rust:"
run_test "$TRACTOR" "$FIXTURES/sample.rs" -x "function" --expect 2 -m "function elements exist for fn declarations"
run_test "$TRACTOR" "$FIXTURES/sample.rs" -x "function[name='add']" --expect 1 -m "function names are directly queryable"
run_test "$TRACTOR" "$FIXTURES/sample.rs" -x "function[name='main']" --expect 1 -m "main function is recognized"
run_test "$TRACTOR" "$FIXTURES/sample.rs" -x "file" --expect 1 -m "source_file renamed to file"
run_test "$TRACTOR" "$FIXTURES/sample.rs" -x "let" --expect 1 -m "let_declaration renamed to let"
run_test "$TRACTOR" "$FIXTURES/sample.rs" -x "binary[op='+']" --expect 1 -m "operators extracted to op element"
run_test "$TRACTOR" "$FIXTURES/sample.rs" -x "call" --expect 1 -m "call_expression renamed to call"
run_test "$TRACTOR" "$FIXTURES/sample.rs" -x "macro" --expect 1 -m "macro_invocation renamed to macro"
echo ""

# Python
echo "Python:"
run_test "$TRACTOR" "$FIXTURES/sample.py" -x "function" --expect 2 -m "def statements become function elements"
run_test "$TRACTOR" "$FIXTURES/sample.py" -x "function[name='add']" --expect 1 -m "function names are directly queryable"
run_test "$TRACTOR" "$FIXTURES/sample.py" -x "function[name='main']" --expect 1 -m "main function is recognized"
run_test "$TRACTOR" "$FIXTURES/sample.py" -x "module" --expect 1 -m "module element exists"
run_test "$TRACTOR" "$FIXTURES/sample.py" -x "return" --expect 1 -m "return_statement renamed to return"
run_test "$TRACTOR" "$FIXTURES/sample.py" -x "binary[op='+']" --expect 1 -m "operators extracted to op element"
run_test "$TRACTOR" "$FIXTURES/sample.py" -x "call" --expect 3 -m "call expressions renamed to call"
echo ""

# TypeScript
echo "TypeScript:"
run_test "$TRACTOR" "$FIXTURES/sample.ts" -x "function" --expect 2 -m "function declarations become function elements"
run_test "$TRACTOR" "$FIXTURES/sample.ts" -x "function[name='add']" --expect 1 -m "function names are directly queryable"
run_test "$TRACTOR" "$FIXTURES/sample.ts" -x "function[name='main']" --expect 1 -m "main function is recognized"
run_test "$TRACTOR" "$FIXTURES/sample.ts" -x "program" --expect 1 -m "program element exists"
run_test "$TRACTOR" "$FIXTURES/sample.ts" -x "variable" --expect 1 -m "variable declarations renamed to variable"
run_test "$TRACTOR" "$FIXTURES/sample.ts" -x "binary[op='+']" --expect 1 -m "operators extracted to op element"
run_test "$TRACTOR" "$FIXTURES/sample.ts" -x "call" --expect 3 -m "call expressions renamed to call"
echo ""

# JavaScript
echo "JavaScript:"
run_test "$TRACTOR" "$FIXTURES/sample.js" -x "function" --expect 2 -m "function declarations become function elements"
run_test "$TRACTOR" "$FIXTURES/sample.js" -x "function[name='add']" --expect 1 -m "function names are directly queryable"
run_test "$TRACTOR" "$FIXTURES/sample.js" -x "function[name='main']" --expect 1 -m "main function is recognized"
run_test "$TRACTOR" "$FIXTURES/sample.js" -x "program" --expect 1 -m "program element exists"
run_test "$TRACTOR" "$FIXTURES/sample.js" -x "call" --expect 3 -m "call expressions renamed to call"
echo ""

# Go
echo "Go:"
run_test "$TRACTOR" "$FIXTURES/sample.go" -x "function" --expect 2 -m "func declarations become function elements"
run_test "$TRACTOR" "$FIXTURES/sample.go" -x "function[name='add']" --expect 1 -m "function names are directly queryable"
run_test "$TRACTOR" "$FIXTURES/sample.go" -x "function[name='main']" --expect 1 -m "main function is recognized"
run_test "$TRACTOR" "$FIXTURES/sample.go" -x "file" --expect 1 -m "source_file renamed to file"
run_test "$TRACTOR" "$FIXTURES/sample.go" -x "package" --expect 1 -m "package_clause renamed to package"
run_test "$TRACTOR" "$FIXTURES/sample.go" -x "binary[op='+']" --expect 1 -m "operators extracted to op element"
run_test "$TRACTOR" "$FIXTURES/sample.go" -x "call" --expect 2 -m "call expressions renamed to call"
echo ""

# Java
echo "Java:"
run_test "$TRACTOR" "$FIXTURES/sample.java" -x "method" --expect 2 -m "method declarations become method elements"
run_test "$TRACTOR" "$FIXTURES/sample.java" -x "method[name='add']" --expect 1 -m "method names are directly queryable"
run_test "$TRACTOR" "$FIXTURES/sample.java" -x "class[name='Sample']" --expect 1 -m "class names are directly queryable"
run_test "$TRACTOR" "$FIXTURES/sample.java" -x "program" --expect 1 -m "program element exists"
run_test "$TRACTOR" "$FIXTURES/sample.java" -x "static" --expect 2 -m "static modifier extracted"
run_test "$TRACTOR" "$FIXTURES/sample.java" -x "binary[op='+']" --expect 2 -m "operators extracted to op element"
run_test "$TRACTOR" "$FIXTURES/sample.java" -x "call" --expect 2 -m "method invocations renamed to call"
echo ""

# C#
echo "C#:"
run_test "$TRACTOR" "$FIXTURES/sample.cs" -x "method" --expect 2 -m "method declarations become method elements"
run_test "$TRACTOR" "$FIXTURES/sample.cs" -x "method[name='Add']" --expect 1 -m "method names are directly queryable"
run_test "$TRACTOR" "$FIXTURES/sample.cs" -x "class[name='Sample']" --expect 1 -m "class names are directly queryable"
run_test "$TRACTOR" "$FIXTURES/sample.cs" -x "unit" --expect 1 -m "compilation_unit renamed to unit"
run_test "$TRACTOR" "$FIXTURES/sample.cs" -x "static" --expect 2 -m "static modifier extracted"
run_test "$TRACTOR" "$FIXTURES/sample.cs" -x "binary[op='+']" --expect 1 -m "operators extracted to op element"
run_test "$TRACTOR" "$FIXTURES/sample.cs" -x "call" --expect 2 -m "invocation expressions renamed to call"
run_test "$TRACTOR" "$FIXTURES/sample.cs" -x "int" --expect 2 -m "integer literals renamed to int"
echo ""

# Ruby
echo "Ruby:"
run_test "$TRACTOR" "$FIXTURES/sample.rb" -x "method" --expect 2 -m "def statements become method elements"
run_test "$TRACTOR" "$FIXTURES/sample.rb" -x "method[name='add']" --expect 1 -m "method names are directly queryable"
run_test "$TRACTOR" "$FIXTURES/sample.rb" -x "method[name='main']" --expect 1 -m "main method is recognized"
run_test "$TRACTOR" "$FIXTURES/sample.rb" -x "call" --expect 2 -m "method calls renamed to call"
echo ""

# Summary
echo -e "${BLUE}───────────────────────────────────────${NC}"
echo "Passed: $PASSED | Failed: $FAILED | Total: $((PASSED + FAILED))"
echo ""

if [ "$FAILED" -eq 0 ]; then
    exit 0
else
    exit 1
fi
