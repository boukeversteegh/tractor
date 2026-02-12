#!/usr/bin/env bash
# Replace feature integration tests
source "$(dirname "$0")/../common.sh"

echo "Replace (YAML):"

# --- Single replacement ---
cat > /tmp/tractor-replace-test-single.yaml << 'EOF'
name: my-app
database:
  host: localhost
  port: 5432
EOF

tractor /tmp/tractor-replace-test-single.yaml -x "//database/host" --replace "host: db.example.com" 2>/dev/null
ACTUAL=$(cat /tmp/tractor-replace-test-single.yaml)
EXPECTED='name: my-app
database:
  host: db.example.com
  port: 5432'
if [ "$ACTUAL" = "$EXPECTED" ]; then
    echo "  ✓ single YAML value replacement"
    ((PASSED++))
else
    echo "  ✗ single YAML value replacement"
    echo "    expected: $EXPECTED"
    echo "    actual: $ACTUAL"
    ((FAILED++))
fi

# --- Multiple replacements in same file ---
cat > /tmp/tractor-replace-test-multi.yaml << 'EOF'
servers:
  - name: web-1
    port: 8080
  - name: web-2
    port: 8080
  - name: web-3
    port: 9090
EOF

tractor /tmp/tractor-replace-test-multi.yaml -x "//servers/item/port[.='8080']" --replace "port: 3000" 2>/dev/null
ACTUAL=$(cat /tmp/tractor-replace-test-multi.yaml)
EXPECTED='servers:
  - name: web-1
    port: 3000
  - name: web-2
    port: 3000
  - name: web-3
    port: 9090'
if [ "$ACTUAL" = "$EXPECTED" ]; then
    echo "  ✓ multiple replacements in same file"
    ((PASSED++))
else
    echo "  ✗ multiple replacements in same file"
    echo "    expected: $EXPECTED"
    echo "    actual: $ACTUAL"
    ((FAILED++))
fi

# --- Replace with --limit ---
cat > /tmp/tractor-replace-test-limit.yaml << 'EOF'
items:
  - value: old
  - value: old
  - value: old
EOF

tractor /tmp/tractor-replace-test-limit.yaml -x "//items/item/value[.='old']" -n 1 --replace "value: new" 2>/dev/null
ACTUAL=$(cat /tmp/tractor-replace-test-limit.yaml)
EXPECTED='items:
  - value: new
  - value: old
  - value: old'
if [ "$ACTUAL" = "$EXPECTED" ]; then
    echo "  ✓ replace respects --limit"
    ((PASSED++))
else
    echo "  ✗ replace respects --limit"
    echo "    expected: $EXPECTED"
    echo "    actual: $ACTUAL"
    ((FAILED++))
fi

echo ""
echo "Replace (JSON):"

# --- JSON string replacement ---
cat > /tmp/tractor-replace-test.json << 'EOF'
{
  "database": {
    "host": "localhost",
    "port": 5432
  }
}
EOF

tractor /tmp/tractor-replace-test.json -x "//pair[string/string_content='host']/value/string" --replace '"db.example.com"' 2>/dev/null
ACTUAL=$(cat /tmp/tractor-replace-test.json)
EXPECTED='{
  "database": {
    "host": "db.example.com",
    "port": 5432
  }
}'
if [ "$ACTUAL" = "$EXPECTED" ]; then
    echo "  ✓ JSON string value replacement"
    ((PASSED++))
else
    echo "  ✗ JSON string value replacement"
    echo "    expected: $EXPECTED"
    echo "    actual: $ACTUAL"
    ((FAILED++))
fi

echo ""
echo "Replace (error cases):"

# --- Replace without XPath should fail ---
if tractor /tmp/tractor-replace-test.json --replace "foo" 2>/dev/null; then
    echo "  ✗ replace without xpath should fail"
    ((FAILED++))
else
    echo "  ✓ replace without xpath fails"
    ((PASSED++))
fi

# --- Replace with stdin should fail ---
if echo "name: test" | tractor --lang yaml -x "//name" --replace "name: new" 2>/dev/null; then
    echo "  ✗ replace with stdin should fail"
    ((FAILED++))
else
    echo "  ✓ replace with stdin fails"
    ((PASSED++))
fi

# --- Replace with no matches should succeed ---
if tractor /tmp/tractor-replace-test.json -x "//nonexistent" --replace "x" 2>/dev/null; then
    echo "  ✓ replace with no matches succeeds"
    ((PASSED++))
else
    echo "  ✗ replace with no matches should succeed"
    ((FAILED++))
fi

# --- Replace with empty string (deletion) ---
cat > /tmp/tractor-replace-test-delete.yaml << 'EOF'
keep: yes
remove: this
also_keep: yes
EOF

tractor /tmp/tractor-replace-test-delete.yaml -x "//remove" --replace "" 2>/dev/null
ACTUAL=$(cat /tmp/tractor-replace-test-delete.yaml)
EXPECTED='keep: yes

also_keep: yes'
if [ "$ACTUAL" = "$EXPECTED" ]; then
    echo "  ✓ replace with empty string (deletion)"
    ((PASSED++))
else
    echo "  ✗ replace with empty string (deletion)"
    echo "    expected: $EXPECTED"
    echo "    actual: $ACTUAL"
    ((FAILED++))
fi

# Cleanup
rm -f /tmp/tractor-replace-test-single.yaml /tmp/tractor-replace-test-multi.yaml /tmp/tractor-replace-test-limit.yaml /tmp/tractor-replace-test.json /tmp/tractor-replace-test-delete.yaml

report
