#!/usr/bin/env bash
# Snapshot tests for set command output formats
source "$(dirname "$0")/../../common.sh"

SNAPSHOT_DIR="$REPO_ROOT/tests/integration/formats/set"

# Helper: run set on a copy of sample.yaml, compare stdout to snapshot
check_set_snapshot() {
    local desc="$1"
    local snapshot="$2"
    shift 2
    local args=("$@")

    # Copy sample so the in-place mode doesn't clobber it
    cp "$SNAPSHOT_DIR/sample.yaml" /tmp/tractor-set-snap.yaml

    # Run tractor set, capture stdout, normalise the temp path back to the
    # repo-relative path used in the committed snapshot
    local actual
    actual=$(tractor set /tmp/tractor-set-snap.yaml "${args[@]}" 2>/dev/null \
        | sed "s|/tmp/tractor-set-snap.yaml|tests/integration/formats/set/sample.yaml|g")

    local expected
    expected=$(cat "$snapshot")

    if [ "$actual" = "$expected" ]; then
        echo "  ✓ $desc"
        ((PASSED++))
    else
        echo "  ✗ $desc"
        diff <(echo "$expected") <(echo "$actual") --color=always -u --label expected --label actual | sed 's/^/      /'
        ((FAILED++))
    fi
    rm -f /tmp/tractor-set-snap.yaml
}

echo "Set (snapshot: text format):"

check_set_snapshot "text default (file:line + status + summary)" \
    "$SNAPSHOT_DIR/set.txt" \
    -x "//database/host" --value "db.example.com" --no-color

check_set_snapshot "text unchanged (value already set)" \
    "$SNAPSHOT_DIR/set-unchanged.txt" \
    -x "//database/host" --value "localhost" --no-color

echo ""
echo "Set (snapshot: text stdout mode):"

# Stdout mode: compare raw output (no path mangling needed — file is not modified)
check_set_snapshot "text stdout mode" \
    "$SNAPSHOT_DIR/set-stdout.txt" \
    -x "//database/host" --value "db.example.com" --stdout --no-color

echo ""
echo "Set (snapshot: json format):"

check_set_snapshot "json default" \
    "$SNAPSHOT_DIR/set.json" \
    -x "//database/host" --value "db.example.com" -f json

echo ""
echo "Set (snapshot: xml format):"

check_set_snapshot "xml default" \
    "$SNAPSHOT_DIR/set.xml" \
    -x "//database/host" --value "db.example.com" -f xml --no-color

report
