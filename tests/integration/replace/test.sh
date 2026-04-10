#!/usr/bin/env bash
# Set (--set) feature integration tests
source "$(dirname "$0")/../common.sh"

TMPDIR="$(mktemp -d "$SCRIPT_DIR/tmp.replace.XXXXXX")"
SINGLE_YAML="$TMPDIR/tractor-set-test-single.yaml"
MULTI_YAML="$TMPDIR/tractor-set-test-multi.yaml"
LIMIT_YAML="$TMPDIR/tractor-set-test-limit.yaml"
JSON_FILE="$TMPDIR/tractor-set-test.json"
STDOUT_YAML="$TMPDIR/tractor-set-stdout.yaml"

echo "Set (YAML):"

# --- Single replacement ---
cat > "$SINGLE_YAML" << 'EOF'
name: my-app
database:
  host: localhost
  port: 5432
EOF

tractor set "$(to_tractor_path "$SINGLE_YAML")" -x "//database/host" --value "db.example.com" 2>/dev/null
ACTUAL=$(cat "$SINGLE_YAML")
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
cat > "$MULTI_YAML" << 'EOF'
servers:
  - name: web-1
    port: 8080
  - name: web-2
    port: 8080
  - name: web-3
    port: 9090
EOF

tractor set "$(to_tractor_path "$MULTI_YAML")" -x "//servers/port[.='8080']" --value "3000" 2>/dev/null
ACTUAL=$(cat "$MULTI_YAML")
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

# --- Path expression predicates should filter set targets ---
cat > "$MULTI_YAML" << 'EOF'
servers:
  - host: localhost
    port: 5432
  - host: prod-db
    port: 5432
EOF

tractor set "$(to_tractor_path "$MULTI_YAML")" "servers[host='localhost']/port" --value "5433" 2>/dev/null
ACTUAL=$(cat "$MULTI_YAML")
EXPECTED='servers:
  - host: localhost
    port: 5433
  - host: prod-db
    port: 5432'
if [ "$ACTUAL" = "$EXPECTED" ]; then
    echo "  OK path-expression predicates filter set targets"
    ((PASSED++))
else
    echo "  FAIL path-expression predicates should filter set targets"
    echo "    expected: $EXPECTED"
    echo "    actual: $ACTUAL"
    ((FAILED++))
fi

# --- Set with --limit ---
cat > "$LIMIT_YAML" << 'EOF'
items:
  - value: old
  - value: old
  - value: old
EOF

tractor set "$(to_tractor_path "$LIMIT_YAML")" -x "//items/value[.='old']" -n 1 --value "new" 2>/dev/null
ACTUAL=$(cat "$LIMIT_YAML")
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
cat > "$JSON_FILE" << 'EOF'
{
  "database": {
    "host": "localhost",
    "port": 5432
  }
}
EOF

tractor set "$(to_tractor_path "$JSON_FILE")" -x "//database/host" --value db.example.com 2>/dev/null
ACTUAL=$(cat "$JSON_FILE")
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
echo "Set (stdout mode):"

# --- stdin with --lang writes to stdout (implicit stdout mode) ---
RESULT=$(echo "name: test" | tractor set -l yaml -x "//name" --value "newvalue" 2>/dev/null)
EXPECTED="name: newvalue"
if [ "$RESULT" = "$EXPECTED" ]; then
    echo "  ✓ set with stdin writes to stdout (implicit stdout mode)"
    ((PASSED++))
else
    echo "  ✗ set with stdin should write to stdout"
    echo "    expected: $EXPECTED"
    echo "    actual: $RESULT"
    ((FAILED++))
fi

# --- explicit --stdout flag writes to stdout without modifying file ---
cat > "$STDOUT_YAML" << 'EOF'
host: localhost
EOF

RESULT=$(tractor set "$(to_tractor_path "$STDOUT_YAML")" -x "//host" --value "example.com" --stdout 2>/dev/null)
ORIGINAL=$(cat "$STDOUT_YAML")
if [ "$RESULT" = "host: example.com" ] && [ "$ORIGINAL" = "host: localhost" ]; then
    echo "  ✓ --stdout outputs to stdout without modifying file"
    ((PASSED++))
else
    echo "  ✗ --stdout should output to stdout without modifying file"
    echo "    result: $RESULT"
    echo "    original (should be unchanged): $ORIGINAL"
    ((FAILED++))
fi
rm -f "$STDOUT_YAML"

echo ""
echo "Set (error cases):"

# --- Set without XPath should fail ---
if tractor set "$(to_tractor_path "$JSON_FILE")" --value "foo" 2>/dev/null; then
    echo "  ✗ set without xpath should fail"
    ((FAILED++))
else
    echo "  ✓ set without xpath fails"
    ((PASSED++))
fi

# --- Set with no matches should succeed ---
if tractor set "$(to_tractor_path "$JSON_FILE")" -x "//nonexistent" --value "x" 2>/dev/null; then
    echo "  ✓ set with no matches succeeds"
    ((PASSED++))
else
    echo "  ✗ set with no matches should succeed"
    ((FAILED++))
fi

# Cleanup
rm -rf "$TMPDIR"

report
