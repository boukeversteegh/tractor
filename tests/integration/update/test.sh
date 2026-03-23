#!/usr/bin/env bash
# Update command integration tests
source "$(dirname "$0")/../common.sh"

echo "Update (YAML):"

# --- Single update ---
cat > /tmp/tractor-update-test-single.yaml << 'EOF'
name: my-app
database:
  host: localhost
  port: 5432
EOF

tractor update /tmp/tractor-update-test-single.yaml -x "//database/host" --value "db.example.com" 2>/dev/null
ACTUAL=$(cat /tmp/tractor-update-test-single.yaml)
EXPECTED='name: my-app
database:
  host: db.example.com
  port: 5432'
if [ "$ACTUAL" = "$EXPECTED" ]; then
    echo "  ✓ single YAML value update"
    ((PASSED++))
else
    echo "  ✗ single YAML value update"
    echo "    expected: $EXPECTED"
    echo "    actual: $ACTUAL"
    ((FAILED++))
fi

# --- Multiple updates in same file ---
cat > /tmp/tractor-update-test-multi.yaml << 'EOF'
servers:
  - name: web-1
    port: 8080
  - name: web-2
    port: 8080
  - name: web-3
    port: 9090
EOF

tractor update /tmp/tractor-update-test-multi.yaml -x "//servers/port[.='8080']" --value "3000" 2>/dev/null
ACTUAL=$(cat /tmp/tractor-update-test-multi.yaml)
EXPECTED='servers:
  - name: web-1
    port: 3000
  - name: web-2
    port: 3000
  - name: web-3
    port: 9090'
if [ "$ACTUAL" = "$EXPECTED" ]; then
    echo "  ✓ multiple updates in same file"
    ((PASSED++))
else
    echo "  ✗ multiple updates in same file"
    echo "    expected: $EXPECTED"
    echo "    actual: $ACTUAL"
    ((FAILED++))
fi

# --- Update with --limit ---
cat > /tmp/tractor-update-test-limit.yaml << 'EOF'
items:
  - value: old
  - value: old
  - value: old
EOF

tractor update /tmp/tractor-update-test-limit.yaml -x "//items/value[.='old']" -n 1 --value "new" 2>/dev/null
ACTUAL=$(cat /tmp/tractor-update-test-limit.yaml)
EXPECTED='items:
  - value: new
  - value: old
  - value: old'
if [ "$ACTUAL" = "$EXPECTED" ]; then
    echo "  ✓ update respects --limit"
    ((PASSED++))
else
    echo "  ✗ update respects --limit"
    echo "    expected: $EXPECTED"
    echo "    actual: $ACTUAL"
    ((FAILED++))
fi

# --- Update with no matches does NOT create nodes and fails ---
cat > /tmp/tractor-update-test-nocreate.yaml << 'EOF'
name: my-app
EOF

if tractor update /tmp/tractor-update-test-nocreate.yaml -x "//database/host" --value "localhost" 2>/dev/null; then
    echo "  ✗ update with no match should fail"
    ((FAILED++))
else
    ACTUAL=$(cat /tmp/tractor-update-test-nocreate.yaml)
    EXPECTED='name: my-app'
    if [ "$ACTUAL" = "$EXPECTED" ]; then
        echo "  ✓ update with no match fails and does not create nodes"
        ((PASSED++))
    else
        echo "  ✗ update with no match fails but modified the file"
        echo "    expected: $EXPECTED"
        echo "    actual: $ACTUAL"
        ((FAILED++))
    fi
fi

# --- Update with partial path does NOT create leaf and fails ---
cat > /tmp/tractor-update-test-partial.yaml << 'EOF'
database:
  host: localhost
EOF

if tractor update /tmp/tractor-update-test-partial.yaml -x "//database/port" --value "5432" 2>/dev/null; then
    echo "  ✗ update with partial path should fail"
    ((FAILED++))
else
    ACTUAL=$(cat /tmp/tractor-update-test-partial.yaml)
    EXPECTED='database:
  host: localhost'
    if [ "$ACTUAL" = "$EXPECTED" ]; then
        echo "  ✓ update with partial path fails and does not create leaf"
        ((PASSED++))
    else
        echo "  ✗ update with partial path fails but modified the file"
        echo "    expected: $EXPECTED"
        echo "    actual: $ACTUAL"
        ((FAILED++))
    fi
fi

echo ""
echo "Update (JSON):"

# --- JSON string update ---
cat > /tmp/tractor-update-test.json << 'EOF'
{
  "database": {
    "host": "localhost",
    "port": 5432
  }
}
EOF

tractor update /tmp/tractor-update-test.json -x "//database/host" --value db.example.com 2>/dev/null
ACTUAL=$(cat /tmp/tractor-update-test.json)
EXPECTED='{
  "database": {
    "host": "db.example.com",
    "port": 5432
  }
}'
if [ "$ACTUAL" = "$EXPECTED" ]; then
    echo "  ✓ JSON string value update"
    ((PASSED++))
else
    echo "  ✗ JSON string value update"
    echo "    expected: $EXPECTED"
    echo "    actual: $ACTUAL"
    ((FAILED++))
fi

# --- JSON update with no match does NOT create and fails ---
cat > /tmp/tractor-update-test-nocreate.json << 'EOF'
{
  "name": "my-app"
}
EOF

if tractor update /tmp/tractor-update-test-nocreate.json -x "//database/host" --value "localhost" 2>/dev/null; then
    echo "  ✗ JSON update with no match should fail"
    ((FAILED++))
else
    ACTUAL=$(cat /tmp/tractor-update-test-nocreate.json)
    EXPECTED='{
  "name": "my-app"
}'
    if [ "$ACTUAL" = "$EXPECTED" ]; then
        echo "  ✓ JSON update with no match fails and does not create nodes"
        ((PASSED++))
    else
        echo "  ✗ JSON update with no match fails but modified the file"
        echo "    expected: $EXPECTED"
        echo "    actual: $ACTUAL"
        ((FAILED++))
    fi
fi

echo ""
echo "Update (error cases):"

# --- Update without XPath should fail ---
if tractor update /tmp/tractor-update-test.json --value "foo" 2>/dev/null; then
    echo "  ✗ update without xpath should fail"
    ((FAILED++))
else
    echo "  ✓ update without xpath fails"
    ((PASSED++))
fi

# --- Update with stdin should fail ---
if echo "name: test" | tractor update --lang yaml -x "//name" --value "new" 2>/dev/null; then
    echo "  ✗ update with stdin should fail"
    ((FAILED++))
else
    echo "  ✓ update with stdin fails"
    ((PASSED++))
fi

# --- Update with no matches should fail ---
if tractor update /tmp/tractor-update-test.json -x "//nonexistent" --value "x" 2>/dev/null; then
    echo "  ✗ update with no matches should fail"
    ((FAILED++))
else
    echo "  ✓ update with no matches fails"
    ((PASSED++))
fi

# Cleanup
rm -f /tmp/tractor-update-test-single.yaml /tmp/tractor-update-test-multi.yaml \
      /tmp/tractor-update-test-limit.yaml /tmp/tractor-update-test-nocreate.yaml \
      /tmp/tractor-update-test-partial.yaml /tmp/tractor-update-test.json \
      /tmp/tractor-update-test-nocreate.json

report
