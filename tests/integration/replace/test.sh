#!/usr/bin/env bash
# Set (--set) feature integration tests
source "$(dirname "$0")/../common.sh"

echo "Set (YAML):"

# --- Single replacement ---
cat > /tmp/tractor-set-test-single.yaml << 'EOF'
name: my-app
database:
  host: localhost
  port: 5432
EOF

tractor /tmp/tractor-set-test-single.yaml -x "//database/host" --set "db.example.com" 2>/dev/null
ACTUAL=$(cat /tmp/tractor-set-test-single.yaml)
EXPECTED='name: my-app
database:
  host: db.example.com
  port: 5432'
if [ "$ACTUAL" = "$EXPECTED" ]; then
    echo "  ✓ single YAML value set"
    ((PASSED++))
else
    echo "  ✗ single YAML value set"
    echo "    expected: $EXPECTED"
    echo "    actual: $ACTUAL"
    ((FAILED++))
fi

# --- Multiple replacements in same file ---
cat > /tmp/tractor-set-test-multi.yaml << 'EOF'
servers:
  - name: web-1
    port: 8080
  - name: web-2
    port: 8080
  - name: web-3
    port: 9090
EOF

tractor /tmp/tractor-set-test-multi.yaml -x "//servers/port[.='8080']" --set "3000" 2>/dev/null
ACTUAL=$(cat /tmp/tractor-set-test-multi.yaml)
EXPECTED='servers:
  - name: web-1
    port: 3000
  - name: web-2
    port: 3000
  - name: web-3
    port: 9090'
if [ "$ACTUAL" = "$EXPECTED" ]; then
    echo "  ✓ multiple sets in same file"
    ((PASSED++))
else
    echo "  ✗ multiple sets in same file"
    echo "    expected: $EXPECTED"
    echo "    actual: $ACTUAL"
    ((FAILED++))
fi

# --- Set with --limit ---
cat > /tmp/tractor-set-test-limit.yaml << 'EOF'
items:
  - value: old
  - value: old
  - value: old
EOF

tractor /tmp/tractor-set-test-limit.yaml -x "//items/value[.='old']" -n 1 --set "new" 2>/dev/null
ACTUAL=$(cat /tmp/tractor-set-test-limit.yaml)
EXPECTED='items:
  - value: new
  - value: old
  - value: old'
if [ "$ACTUAL" = "$EXPECTED" ]; then
    echo "  ✓ set respects --limit"
    ((PASSED++))
else
    echo "  ✗ set respects --limit"
    echo "    expected: $EXPECTED"
    echo "    actual: $ACTUAL"
    ((FAILED++))
fi

echo ""
echo "Set (JSON):"

# --- JSON string replacement ---
cat > /tmp/tractor-set-test.json << 'EOF'
{
  "database": {
    "host": "localhost",
    "port": 5432
  }
}
EOF

tractor /tmp/tractor-set-test.json -x "//database/host" --set '"db.example.com"' 2>/dev/null
ACTUAL=$(cat /tmp/tractor-set-test.json)
EXPECTED='{
  "database": {
    "host": "db.example.com",
    "port": 5432
  }
}'
if [ "$ACTUAL" = "$EXPECTED" ]; then
    echo "  ✓ JSON string value set"
    ((PASSED++))
else
    echo "  ✗ JSON string value set"
    echo "    expected: $EXPECTED"
    echo "    actual: $ACTUAL"
    ((FAILED++))
fi

echo ""
echo "Set (error cases):"

# --- Set without XPath should fail ---
if tractor /tmp/tractor-set-test.json --set "foo" 2>/dev/null; then
    echo "  ✗ set without xpath should fail"
    ((FAILED++))
else
    echo "  ✓ set without xpath fails"
    ((PASSED++))
fi

# --- Set with stdin should fail ---
if echo "name: test" | tractor --lang yaml -x "//name" --set "name: new" 2>/dev/null; then
    echo "  ✗ set with stdin should fail"
    ((FAILED++))
else
    echo "  ✓ set with stdin fails"
    ((PASSED++))
fi

# --- Set with no matches should succeed ---
if tractor /tmp/tractor-set-test.json -x "//nonexistent" --set "x" 2>/dev/null; then
    echo "  ✓ set with no matches succeeds"
    ((PASSED++))
else
    echo "  ✗ set with no matches should succeed"
    ((FAILED++))
fi

# Cleanup
rm -f /tmp/tractor-set-test-single.yaml /tmp/tractor-set-test-multi.yaml /tmp/tractor-set-test-limit.yaml /tmp/tractor-set-test.json

report
