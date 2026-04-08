#!/usr/bin/env bash
# Update command integration tests
source "$(dirname "$0")/../common.sh"

TMPDIR="$(mktemp -d "$SCRIPT_DIR/tmp.update.XXXXXX")"
SINGLE_YAML="$TMPDIR/tractor-update-test-single.yaml"
MULTI_YAML="$TMPDIR/tractor-update-test-multi.yaml"
LIMIT_YAML="$TMPDIR/tractor-update-test-limit.yaml"
NOCREATE_YAML="$TMPDIR/tractor-update-test-nocreate.yaml"
PARTIAL_YAML="$TMPDIR/tractor-update-test-partial.yaml"
JSON_FILE="$TMPDIR/tractor-update-test.json"
NOCREATE_JSON="$TMPDIR/tractor-update-test-nocreate.json"

echo "Update (YAML):"

# --- Single update ---
cat > "$SINGLE_YAML" << 'EOF'
name: my-app
database:
  host: localhost
  port: 5432
EOF

tractor update "$(to_tractor_path "$SINGLE_YAML")" -x "//database/host" --value "db.example.com" 2>/dev/null
ACTUAL=$(cat "$SINGLE_YAML")
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
cat > "$MULTI_YAML" << 'EOF'
servers:
  - name: web-1
    port: 8080
  - name: web-2
    port: 8080
  - name: web-3
    port: 9090
EOF

tractor update "$(to_tractor_path "$MULTI_YAML")" -x "//servers/port[.='8080']" --value "3000" 2>/dev/null
ACTUAL=$(cat "$MULTI_YAML")
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
cat > "$LIMIT_YAML" << 'EOF'
items:
  - value: old
  - value: old
  - value: old
EOF

tractor update "$(to_tractor_path "$LIMIT_YAML")" -x "//items/value[.='old']" -n 1 --value "new" 2>/dev/null
ACTUAL=$(cat "$LIMIT_YAML")
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
cat > "$NOCREATE_YAML" << 'EOF'
name: my-app
EOF

if tractor update "$(to_tractor_path "$NOCREATE_YAML")" -x "//database/host" --value "localhost" 2>/dev/null; then
    echo "  ✗ update with no match should fail"
    ((FAILED++))
else
    ACTUAL=$(cat "$NOCREATE_YAML")
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
cat > "$PARTIAL_YAML" << 'EOF'
database:
  host: localhost
EOF

if tractor update "$(to_tractor_path "$PARTIAL_YAML")" -x "//database/port" --value "5432" 2>/dev/null; then
    echo "  ✗ update with partial path should fail"
    ((FAILED++))
else
    ACTUAL=$(cat "$PARTIAL_YAML")
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
cat > "$JSON_FILE" << 'EOF'
{
  "database": {
    "host": "localhost",
    "port": 5432
  }
}
EOF

tractor update "$(to_tractor_path "$JSON_FILE")" -x "//database/host" --value db.example.com 2>/dev/null
ACTUAL=$(cat "$JSON_FILE")
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
cat > "$NOCREATE_JSON" << 'EOF'
{
  "name": "my-app"
}
EOF

if tractor update "$(to_tractor_path "$NOCREATE_JSON")" -x "//database/host" --value "localhost" 2>/dev/null; then
    echo "  ✗ JSON update with no match should fail"
    ((FAILED++))
else
    ACTUAL=$(cat "$NOCREATE_JSON")
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
if tractor update "$(to_tractor_path "$JSON_FILE")" --value "foo" 2>/dev/null; then
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
if tractor update "$(to_tractor_path "$JSON_FILE")" -x "//nonexistent" --value "x" 2>/dev/null; then
    echo "  ✗ update with no matches should fail"
    ((FAILED++))
else
    echo "  ✓ update with no matches fails"
    ((PASSED++))
fi

# Cleanup
rm -rf "$TMPDIR"

report
