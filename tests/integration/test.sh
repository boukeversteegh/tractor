#!/usr/bin/env bash
# Integration tests for tractor - Pure bash using --expect flag

set -euo pipefail

cd "$(dirname "$0")/../.."

TRACTOR="./target/release/tractor"
FIXTURES="tests/integration/fixtures"
SNAPSHOTS="tests/integration/snapshots"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

PASSED=0
FAILED=0

# Run test and check exit code
run_test() {
    local desc="$1"
    shift
    if "$@" </dev/null >/dev/null 2>&1; then
        echo -e "  ${GREEN}✓${NC} $desc"
        ((PASSED++))
    else
        echo -e "  ${RED}✗${NC} $desc"
        ((FAILED++))
    fi
}

# Build if needed
[ -f "$TRACTOR" ] || cargo build --release -q

echo -e "${BLUE}═══════════════════════════════════════${NC}"
echo -e "${BLUE}  Tractor Integration Tests${NC}"
echo -e "${BLUE}═══════════════════════════════════════${NC}"
echo ""

# Test 2: Structure Assertions using --expect flag
echo -e "${YELLOW}Structure Assertions (XPath + --expect)${NC}"
echo ""

echo "Rust:"
run_test "2 functions" "$TRACTOR" "$FIXTURES/sample.rs" -x "//function" --expect 2
run_test "'add' function" "$TRACTOR" "$FIXTURES/sample.rs" -x "//function/name[type='add']" --expect 1
run_test "'main' function" "$TRACTOR" "$FIXTURES/sample.rs" -x "//function/name[type='main']" --expect 1
run_test "+ operator" "$TRACTOR" "$FIXTURES/sample.rs" -x "//binary[@op='+']" --expect 1
echo ""

echo "Python:"
run_test "2 functions" "$TRACTOR" "$FIXTURES/sample.py" -x "//function" --expect 2
run_test "'add' function" "$TRACTOR" "$FIXTURES/sample.py" -x "//function/name[type='add']" --expect 1
run_test "'main' function" "$TRACTOR" "$FIXTURES/sample.py" -x "//function/name[type='main']" --expect 1
echo ""

echo "TypeScript:"
run_test "2 functions" "$TRACTOR" "$FIXTURES/sample.ts" -x "//function" --expect 2
run_test "'add' function" "$TRACTOR" "$FIXTURES/sample.ts" -x "//function/name[type='add']" --expect 1
run_test "'main' function" "$TRACTOR" "$FIXTURES/sample.ts" -x "//function/name[type='main']" --expect 1
echo ""

echo "JavaScript:"
run_test "2 functions" "$TRACTOR" "$FIXTURES/sample.js" -x "//function" --expect 2
run_test "'add' function" "$TRACTOR" "$FIXTURES/sample.js" -x "//function/name[type='add']" --expect 1
run_test "'main' function" "$TRACTOR" "$FIXTURES/sample.js" -x "//function/name[type='main']" --expect 1
echo ""

echo "Go:"
run_test "2 functions" "$TRACTOR" "$FIXTURES/sample.go" -x "//function" --expect 2
run_test "'add' function" "$TRACTOR" "$FIXTURES/sample.go" -x "//function/name[type='add']" --expect 1
run_test "'main' function" "$TRACTOR" "$FIXTURES/sample.go" -x "//function/name[type='main']" --expect 1
echo ""

echo "Java:"
run_test "2 methods" "$TRACTOR" "$FIXTURES/sample.java" -x "//method" --expect 2
run_test "'add' method" "$TRACTOR" "$FIXTURES/sample.java" -x "//method/name[type='add']" --expect 1
run_test "'Sample' class" "$TRACTOR" "$FIXTURES/sample.java" -x "//class/name[type='Sample']" --expect 1
echo ""

echo "C#:"
run_test "2 methods" "$TRACTOR" "$FIXTURES/sample.cs" -x "//method" --expect 2
run_test "'Add' method" "$TRACTOR" "$FIXTURES/sample.cs" -x "//method/name[type='Add']" --expect 1
run_test "'Sample' class" "$TRACTOR" "$FIXTURES/sample.cs" -x "//class/name[type='Sample']" --expect 1
echo ""

echo "Ruby:"
run_test "2 methods" "$TRACTOR" "$FIXTURES/sample.rb" -x "//method" --expect 2
run_test "'add' method" "$TRACTOR" "$FIXTURES/sample.rb" -x "//method/name[type='add']" --expect 1
run_test "'main' method" "$TRACTOR" "$FIXTURES/sample.rb" -x "//method/name[type='main']" --expect 1
echo ""

# Summary
echo -e "${BLUE}═══════════════════════════════════════${NC}"
echo -e "Passed: ${GREEN}$PASSED${NC} | Failed: ${RED}$FAILED${NC} | Total: $((PASSED + FAILED))"
echo ""

if [ "$FAILED" -eq 0 ]; then
    echo -e "${GREEN}✓ All tests passed!${NC}"
    exit 0
else
    echo -e "${RED}✗ Some tests failed${NC}"
    exit 1
fi
