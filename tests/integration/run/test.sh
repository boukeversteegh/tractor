#!/usr/bin/env bash
# Integration tests for `tractor run` (batch execution)
source "$(dirname "$0")/../common.sh"

FIXTURE_DIR="$SCRIPT_DIR"

# Helper: run tractor run, capture stdout+stderr (merged),
# normalize absolute paths to relative for stable comparison.
run_and_check() {
    local desc="$1"
    local expected_exit="$2"
    local expected_output="$3"
    shift 3
    local args=("$@")

    local actual
    actual=$(tractor run "${args[@]}" --no-color 2>&1)
    local actual_exit=$?

    # Normalize absolute paths to fixture-relative
    actual=$(echo "$actual" | sed "s|$FIXTURE_DIR/||g")

    if [ "$actual_exit" -ne "$expected_exit" ]; then
        echo "  ✗ $desc (expected exit $expected_exit, got $actual_exit)"
        echo "    output: $actual"
        ((FAILED++))
        return
    fi

    if [ "$actual" = "$expected_output" ]; then
        echo "  ✓ $desc"
        ((PASSED++))
    else
        echo "  ✗ $desc"
        echo "    expected:"
        echo "$expected_output" | sed 's/^/      /'
        echo "    actual:"
        echo "$actual" | sed 's/^/      /'
        ((FAILED++))
    fi
}

# Helper: run tractor run on a copy of fixtures (for set operations that modify files)
run_set_and_check() {
    local desc="$1"
    local expected_exit="$2"
    local expected_output="$3"
    local config="$4"
    shift 4
    local extra_args=("$@")

    # Copy fixtures to temp dir so set operations don't clobber originals
    local tmpdir
    tmpdir=$(mktemp -d)
    cp "$FIXTURE_DIR"/*.json "$FIXTURE_DIR"/*.yaml "$tmpdir/" 2>/dev/null
    # Copy the config and adjust file paths (configs reference relative files)
    cp "$FIXTURE_DIR/$config" "$tmpdir/"

    local actual
    actual=$(tractor run "$tmpdir/$config" "${extra_args[@]}" --no-color 2>&1)
    local actual_exit=$?

    # Normalize temp paths
    actual=$(echo "$actual" | sed "s|$tmpdir/||g")

    rm -rf "$tmpdir"

    if [ "$actual_exit" -ne "$expected_exit" ]; then
        echo "  ✗ $desc (expected exit $expected_exit, got $actual_exit)"
        echo "    output: $actual"
        ((FAILED++))
        return
    fi

    if [ "$actual" = "$expected_output" ]; then
        echo "  ✓ $desc"
        ((PASSED++))
    else
        echo "  ✗ $desc"
        echo "    expected:"
        echo "$expected_output" | sed 's/^/      /'
        echo "    actual:"
        echo "$actual" | sed 's/^/      /'
        ((FAILED++))
    fi
}

echo "Run (check operations):"

run_and_check "multirule check finds violations with correct severity" \
    1 \
    "$(printf 'settings.yaml:3:10: error: debug should be disabled in production\n3 |   debug: true\n             ^~~~\n\nsettings.yaml:4:14: warning: log level should not be debug in production\n4 |   log_level: debug\n                 ^~~~~\n\n\n1 check violation')" \
    "$FIXTURE_DIR/check-multirule.yaml"

run_and_check "multifile check scans multiple files" \
    1 \
    "$(printf 'settings.yaml:3:10: error: debug mode must be disabled\n3 |   debug: true\n             ^~~~\n\n\n1 check violation')" \
    "$FIXTURE_DIR/check-multifile.yaml"

echo ""
echo "Run (set operations):"

run_set_and_check "set applies mappings to files" \
    0 \
    "$(printf 'app-config.json: updated\n\nupdated 1 file')" \
    "set-config.yaml"

run_set_and_check "set with --verbose reports updated files" \
    0 \
    "$(printf 'app-config.json: updated\n\nupdated 1 file')" \
    "set-config.yaml" --verbose

echo ""
echo "Run (verify mode):"

run_set_and_check "verify passes when values are in sync" \
    0 \
    "$(printf 'app-config.json: unchanged')" \
    "verify-config.yaml" --verify

run_set_and_check "verify detects drift" \
    1 \
    "$(printf 'app-config.json: updated\n\n1 file out of sync')" \
    "set-config.yaml" --verify

echo ""
echo "Run (mixed operations):"

run_set_and_check "mixed check+set succeeds when check passes" \
    0 \
    "$(printf 'app-config.json: updated\n\nupdated 1 file')" \
    "mixed-ops.yaml"

report
