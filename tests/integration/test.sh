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
run_test "$TRACTOR" "$FIXTURES/sample.rs" -x "function" --expect 2 -m "Has 2 functions"
run_test "$TRACTOR" "$FIXTURES/sample.rs" -x "function/name[.='add']" --expect 1 -m "Has 'add' function"
run_test "$TRACTOR" "$FIXTURES/sample.rs" -x "function/name[.='main']" --expect 1 -m "Has 'main' function"
run_test "$TRACTOR" "$FIXTURES/sample.rs" -x "binary[@op='+']" --expect 1 -m "Has + operator"
echo ""

# Python
echo "Python:"
run_test "$TRACTOR" "$FIXTURES/sample.py" -x "function" --expect 2 -m "Has 2 functions"
run_test "$TRACTOR" "$FIXTURES/sample.py" -x "function/name[.='add']" --expect 1 -m "Has 'add' function"
run_test "$TRACTOR" "$FIXTURES/sample.py" -x "function/name[.='main']" --expect 1 -m "Has 'main' function"
echo ""

# TypeScript
echo "TypeScript:"
run_test "$TRACTOR" "$FIXTURES/sample.ts" -x "function" --expect 2 -m "Has 2 functions"
run_test "$TRACTOR" "$FIXTURES/sample.ts" -x "function/name[.='add']" --expect 1 -m "Has 'add' function"
run_test "$TRACTOR" "$FIXTURES/sample.ts" -x "function/name[.='main']" --expect 1 -m "Has 'main' function"
echo ""

# JavaScript
echo "JavaScript:"
run_test "$TRACTOR" "$FIXTURES/sample.js" -x "function" --expect 2 -m "Has 2 functions"
run_test "$TRACTOR" "$FIXTURES/sample.js" -x "function/name[.='add']" --expect 1 -m "Has 'add' function"
run_test "$TRACTOR" "$FIXTURES/sample.js" -x "function/name[.='main']" --expect 1 -m "Has 'main' function"
echo ""

# Go
echo "Go:"
run_test "$TRACTOR" "$FIXTURES/sample.go" -x "function" --expect 2 -m "Has 2 functions"
run_test "$TRACTOR" "$FIXTURES/sample.go" -x "function/name[.='add']" --expect 1 -m "Has 'add' function"
run_test "$TRACTOR" "$FIXTURES/sample.go" -x "function/name[.='main']" --expect 1 -m "Has 'main' function"
echo ""

# Java
echo "Java:"
run_test "$TRACTOR" "$FIXTURES/sample.java" -x "method" --expect 2 -m "Has 2 methods"
run_test "$TRACTOR" "$FIXTURES/sample.java" -x "method/name[.='add']" --expect 1 -m "Has 'add' method"
run_test "$TRACTOR" "$FIXTURES/sample.java" -x "class/name[.='Sample']" --expect 1 -m "Has 'Sample' class"
echo ""

# C#
echo "C#:"
run_test "$TRACTOR" "$FIXTURES/sample.cs" -x "method" --expect 2 -m "Has 2 methods"
run_test "$TRACTOR" "$FIXTURES/sample.cs" -x "method/name[.='Add']" --expect 1 -m "Has 'Add' method"
run_test "$TRACTOR" "$FIXTURES/sample.cs" -x "class/name[.='Sample']" --expect 1 -m "Has 'Sample' class"
echo ""

# Ruby
echo "Ruby:"
run_test "$TRACTOR" "$FIXTURES/sample.rb" -x "method" --expect 2 -m "Has 2 methods"
run_test "$TRACTOR" "$FIXTURES/sample.rb" -x "method/name[.='add']" --expect 1 -m "Has 'add' method"
run_test "$TRACTOR" "$FIXTURES/sample.rb" -x "method/name[.='main']" --expect 1 -m "Has 'main' method"
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
