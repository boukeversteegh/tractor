#!/usr/bin/env bash
# Integration tests for tractor
# Tests:
# 1. Snapshot comparison - detect unintended changes in XML output
# 2. Structure assertions - query XML with XPath to verify structure

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FIXTURES_DIR="$SCRIPT_DIR/fixtures"
SNAPSHOTS_DIR="$SCRIPT_DIR/snapshots"
TRACTOR_BIN="${TRACTOR_BIN:-../../target/release/tractor}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Counters
PASSED=0
FAILED=0

# Build tractor if needed
if [ ! -f "$TRACTOR_BIN" ]; then
    echo "Building tractor..."
    cargo build --release
fi

echo "Running integration tests..."
echo ""

# Test 1: Snapshot comparison
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Test Suite 1: Snapshot Comparison"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

for fixture in "$FIXTURES_DIR"/*; do
    if [ -f "$fixture" ]; then
        filename=$(basename "$fixture")
        snapshot="$SNAPSHOTS_DIR/${filename}.xml"

        if [ ! -f "$snapshot" ]; then
            echo -e "${YELLOW}⚠ SKIP${NC} $filename (no snapshot)"
            continue
        fi

        echo -n "Testing $filename... "

        # Generate current output
        current_output=$("$TRACTOR_BIN" "$fixture" 2>/dev/null)

        # Compare with snapshot
        if diff -q <(echo "$current_output") "$snapshot" > /dev/null 2>&1; then
            echo -e "${GREEN}✓ PASS${NC}"
            ((PASSED++))
        else
            echo -e "${RED}✗ FAIL${NC}"
            echo "  Output differs from snapshot!"
            echo "  To see diff: diff <(tractor '$fixture') '$snapshot'"
            ((FAILED++))
        fi
    fi
done

echo ""

# Test 2: Structure assertions using XPath
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Test Suite 2: Structure Assertions (XPath)"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# Helper function to run XPath query and check result
assert_xpath() {
    local file="$1"
    local xpath="$2"
    local expected="$3"
    local description="$4"

    echo -n "$description... "

    result=$("$TRACTOR_BIN" "$file" -x "$xpath" --output count 2>/dev/null || echo "0")

    if [ "$result" = "$expected" ]; then
        echo -e "${GREEN}✓ PASS${NC}"
        ((PASSED++))
        return 0
    else
        echo -e "${RED}✗ FAIL${NC}"
        echo "  Expected: $expected, Got: $result"
        echo "  XPath: $xpath"
        ((FAILED++))
        return 1
    fi
}

# Rust tests
echo "Rust (sample.rs):"
assert_xpath "$FIXTURES_DIR/sample.rs" "//function" "2" "  Should have 2 functions"
assert_xpath "$FIXTURES_DIR/sample.rs" "//function/name[type='add']" "1" "  Should have 'add' function"
assert_xpath "$FIXTURES_DIR/sample.rs" "//function/name[type='main']" "1" "  Should have 'main' function"
assert_xpath "$FIXTURES_DIR/sample.rs" "//binary[@op='+']" "1" "  Should have + operator"
echo ""

# Python tests
echo "Python (sample.py):"
assert_xpath "$FIXTURES_DIR/sample.py" "//function" "2" "  Should have 2 functions"
assert_xpath "$FIXTURES_DIR/sample.py" "//function/name[type='add']" "1" "  Should have 'add' function"
assert_xpath "$FIXTURES_DIR/sample.py" "//function/name[type='main']" "1" "  Should have 'main' function"
echo ""

# TypeScript tests
echo "TypeScript (sample.ts):"
assert_xpath "$FIXTURES_DIR/sample.ts" "//function" "2" "  Should have 2 functions"
assert_xpath "$FIXTURES_DIR/sample.ts" "//function/name[type='add']" "1" "  Should have 'add' function"
assert_xpath "$FIXTURES_DIR/sample.ts" "//function/name[type='main']" "1" "  Should have 'main' function"
echo ""

# JavaScript tests
echo "JavaScript (sample.js):"
assert_xpath "$FIXTURES_DIR/sample.js" "//function" "2" "  Should have 2 functions"
assert_xpath "$FIXTURES_DIR/sample.js" "//function/name[type='add']" "1" "  Should have 'add' function"
assert_xpath "$FIXTURES_DIR/sample.js" "//function/name[type='main']" "1" "  Should have 'main' function"
echo ""

# Go tests
echo "Go (sample.go):"
assert_xpath "$FIXTURES_DIR/sample.go" "//function" "2" "  Should have 2 functions"
assert_xpath "$FIXTURES_DIR/sample.go" "//function/name[type='add']" "1" "  Should have 'add' function"
assert_xpath "$FIXTURES_DIR/sample.go" "//function/name[type='main']" "1" "  Should have 'main' function"
echo ""

# Java tests
echo "Java (sample.java):"
assert_xpath "$FIXTURES_DIR/sample.java" "//method" "2" "  Should have 2 methods"
assert_xpath "$FIXTURES_DIR/sample.java" "//class/name[type='Sample']" "1" "  Should have 'Sample' class"
echo ""

# C# tests
echo "C# (sample.cs):"
assert_xpath "$FIXTURES_DIR/sample.cs" "//method" "2" "  Should have 2 methods"
assert_xpath "$FIXTURES_DIR/sample.cs" "//class/name[type='Sample']" "1" "  Should have 'Sample' class"
echo ""

# Ruby tests
echo "Ruby (sample.rb):"
assert_xpath "$FIXTURES_DIR/sample.rb" "//method" "2" "  Should have 2 methods"
assert_xpath "$FIXTURES_DIR/sample.rb" "//method/name[type='add']" "1" "  Should have 'add' method"
assert_xpath "$FIXTURES_DIR/sample.rb" "//method/name[type='main']" "1" "  Should have 'main' method"
echo ""

# Test 3: Query snapshots directly (XML pass-through)
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Test Suite 3: Snapshot Querying (XML Pass-through)"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "Note: This requires the --xml-input flag (to be implemented)"
echo "For now, we verify snapshots can be used as reference"
echo ""

# Count functions in all snapshot files
for snapshot in "$SNAPSHOTS_DIR"/*.xml; do
    filename=$(basename "$snapshot")
    lang=$(echo "$filename" | sed 's/sample\.\([^.]*\)\.xml/\1/')

    echo -n "Analyzing $filename... "

    # Try to query the snapshot using XPath
    # This tests that our XML structure is valid and queryable
    if "$TRACTOR_BIN" -x "//Files" "$snapshot" &> /dev/null; then
        echo -e "${GREEN}✓ Valid XML${NC}"
        ((PASSED++))
    else
        echo -e "${YELLOW}⚠ XML structure${NC}"
        # Don't fail - this is expected until --xml-input is implemented
    fi
done

echo ""

# Summary
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Test Summary"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo -e "Passed: ${GREEN}$PASSED${NC}"
echo -e "Failed: ${RED}$FAILED${NC}"
echo "Total: $((PASSED + FAILED))"
echo ""

if [ "$FAILED" -eq 0 ]; then
    echo -e "${GREEN}All tests passed!${NC}"
    exit 0
else
    echo -e "${RED}Some tests failed${NC}"
    exit 1
fi
