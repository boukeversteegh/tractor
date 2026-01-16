#!/usr/bin/env bash
# Run all named capture tests
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "========================================="
echo "Running Named Captures Test Suite"
echo "========================================="
echo ""

TOTAL_PASSED=0
TOTAL_FAILED=0

# Run each test script
for test_script in \
    "$SCRIPT_DIR/test-named-captures.sh" \
    "$SCRIPT_DIR/test-design-rules.sh"
do
    if [ -f "$test_script" ]; then
        echo "Running: $(basename "$test_script")"
        echo "========================================="

        # Run test and capture results
        if bash "$test_script"; then
            echo "✓ $(basename "$test_script") passed"
        else
            echo "✗ $(basename "$test_script") failed"
            TOTAL_FAILED=$((TOTAL_FAILED + 1))
        fi
        echo ""
    else
        echo "⚠ Test script not found: $test_script"
        echo ""
    fi
done

echo "========================================="
echo "Named Captures Test Suite Complete"
echo "========================================="

if [ $TOTAL_FAILED -eq 0 ]; then
    echo "✓ All test suites passed!"
    exit 0
else
    echo "✗ $TOTAL_FAILED test suite(s) failed"
    exit 1
fi
