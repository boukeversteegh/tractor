#!/usr/bin/env bash
# Simple integration test

set -euo pipefail

cd /home/user/tractor

echo "Test 1: Generate XML from Rust file"
./target/release/tractor tests/integration/fixtures/sample.rs > /tmp/test_rs.xml
echo "  ✓ Generated XML"

echo "Test 2: Query functions in Rust file"
count=$(./target/release/tractor tests/integration/fixtures/sample.rs -x "//function" --output count)
echo "  Found $count functions"
if [ "$count" = "2" ]; then
    echo "  ✓ Correct count"
else
    echo "  ✗ Expected 2, got $count"
    exit 1
fi

echo "Test 3: Compare with snapshot (ignoring paths)"
# Normalize paths by removing the File path attribute before comparing
sed 's|<File path="[^"]*">|<File>|' tests/integration/snapshots/sample.rs.xml > /tmp/snapshot_normalized.xml
sed 's|<File path="[^"]*">|<File>|' /tmp/test_rs.xml > /tmp/test_normalized.xml
if diff /tmp/snapshot_normalized.xml /tmp/test_normalized.xml > /dev/null 2>&1; then
    echo "  ✓ Matches snapshot"
else
    echo "  ✗ Differs from snapshot"
    diff /tmp/snapshot_normalized.xml /tmp/test_normalized.xml | head -20
    exit 1
fi

echo ""
echo "All tests passed!"
