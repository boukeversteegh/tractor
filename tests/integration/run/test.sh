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
        diff <(echo "$expected_output") <(echo "$actual") --color=always -u --label expected --label actual | sed 's/^/      /'
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
    # Normalize path for Windows (gitbash /tmp/... → C:/Users/.../Temp/...)
    if command -v cygpath &>/dev/null; then tmpdir="$(cygpath -m "$tmpdir")"; fi
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
        diff <(echo "$expected_output") <(echo "$actual") --color=always -u --label expected --label actual | sed 's/^/      /'
        ((FAILED++))
    fi
}

echo "Run (check operations):"

run_and_check "multirule check finds violations with correct severity" \
    1 \
    "$(printf 'settings.yaml:3:10: error: debug should be disabled in production\n3 |   debug: true\n             ^~~~\n\nsettings.yaml:4:14: warning: log level should not be debug in production\n4 |   log_level: debug\n                 ^~~~~\n\n1 error in 1 file')" \
    "$FIXTURE_DIR/check-multirule.yaml"

run_and_check "multifile check scans multiple files" \
    1 \
    "$(printf 'settings.yaml:3:10: error: debug mode must be disabled\n3 |   debug: true\n             ^~~~\n\n1 error in 1 file')" \
    "$FIXTURE_DIR/check-multifile.yaml"

echo ""
echo "Run (set operations):"

run_set_and_check "set applies mappings to files" \
    0 \
    "$(printf 'app-config.json: updated\nupdated 1 file')" \
    "set-config.yaml"

run_set_and_check "set applies mappings (verbose)" \
    0 \
    "$(printf 'app-config.json: updated\nupdated 1 file')" \
    "set-config.yaml"

echo ""
echo "Run (scope intersection):"

SCOPE_DIR="$FIXTURE_DIR/scope-intersection"

run_and_check "root ∩ operation narrows to intersection" \
    0 \
    "$(printf 'scope-intersection/frontend/config.yml:1:8: warning: debug must be disabled\n1 | debug: true\n           ^~~~\n\n1 warning in 1 file')" \
    "$SCOPE_DIR/intersect-narrow.yaml"

run_and_check "root used as base when operation has no files" \
    0 \
    "" \
    "$SCOPE_DIR/intersect-fallback.yaml"

run_and_check "disjoint root and operation yields empty set" \
    0 \
    "" \
    "$SCOPE_DIR/intersect-disjoint.yaml"

echo ""
echo "Run (mixed operations):"

run_set_and_check "mixed check+set succeeds when check passes" \
    0 \
    "$(printf 'app-config.json: updated\nupdated 1 file')" \
    "mixed-ops.yaml"

echo ""
echo "Run (absolute CLI paths):"

ABS_DIR="$FIXTURE_DIR/absolute-paths"
ABS_FILE="$ABS_DIR/config.yml"

run_and_check "absolute CLI path + per-rule include matches" \
    0 \
    "$(printf 'absolute-paths/config.yml:1:8: warning: debug must be disabled\n1 | debug: true\n           ^~~~\n\n1 warning in 1 file')" \
    "$ABS_DIR/check-per-rule-include.yaml" "$ABS_FILE"

run_and_check "absolute CLI path + per-rule exclude filters out" \
    0 \
    "" \
    "$ABS_DIR/check-per-rule-exclude.yaml" "$ABS_FILE"

run_and_check "absolute CLI path + root files intersection works" \
    0 \
    "$(printf 'absolute-paths/config.yml:1:8: warning: debug must be disabled\n1 | debug: true\n           ^~~~\n\n1 warning in 1 file')" \
    "$ABS_DIR/check-root-files.yaml" "$ABS_FILE"

run_and_check "absolute CLI path + root exclude filters out" \
    0 \
    "" \
    "$ABS_DIR/check-root-exclude.yaml" "$ABS_FILE"

echo ""
echo "Run (mixed language rules):"

# Test mixed-language rules - JavaScript and Markdown rules in same config
MIXED_LANG_DIR="$FIXTURE_DIR/mixed-language"

run_and_check "mixed-language: both JS and MD rules find violations" \
    1 \
    "$(printf 'mixed-language/sample.js:1:1: error: TODO comment found\n1 | // TODO: Fix this code\n    ^~~~~~~~~~~~~~~~~~~~~~\n\nmixed-language/todo-doc.md:3:1: warning: TODO comment found\n3 >| <!-- TODO: Complete this section -->\n4 >| \n\n1 error in 2 files')" \
    "$MIXED_LANG_DIR/mixed-rules.yaml"

run_and_check "mixed-language: JS-only rules skip MD files" \
    1 \
    "$(printf 'mixed-language/sample.js:1:1: error: TODO comment found\n1 | // TODO: Fix this code\n    ^~~~~~~~~~~~~~~~~~~~~~\n\n1 error in 1 file')" \
    "$MIXED_LANG_DIR/js-only-rules.yaml"

run_and_check "mixed-language: MD-only rules skip JS files" \
    0 \
    "$(printf 'mixed-language/todo-doc.md:3:1: warning: TODO comment found\n3 >| <!-- TODO: Complete this section -->\n4 >| \n\n1 warning in 1 file')" \
    "$MIXED_LANG_DIR/md-only-rules.yaml"

run_and_check "mixed-language: auto-detect uses file extension" \
    1 \
    "$(printf 'mixed-language/sample.js:1:1: error: TODO comment found\n1 | // TODO: Fix this code\n    ^~~~~~~~~~~~~~~~~~~~~~\n\n1 error in 1 file')" \
    "$MIXED_LANG_DIR/auto-detect.yaml"

run_and_check "mixed-language: multiple rules for same language" \
    1 \
    "$(printf 'mixed-language/sample.js:1:1: error: TODO comment found\n1 | // TODO: Fix this code\n    ^~~~~~~~~~~~~~~~~~~~~~\n\nmixed-language/sample.js:3:5: warning: No console.log calls allowed\n3 |     console.log(\"Hello\");\n        ^~~~~~~~~~~~~~~~~~~~\n\nmixed-language/sample.js:7:5: warning: No console.log calls allowed\n7 |     console.log(\"Goodbye\");\n        ^~~~~~~~~~~~~~~~~~~~~~\n\n1 error in 1 file')" \
    "$MIXED_LANG_DIR/same-lang-rules.yaml"

run_and_check "mixed-language: language alias (js -> javascript)" \
    1 \
    "$(printf 'mixed-language/sample.js:1:1: error: TODO comment found\n1 | // TODO: Fix this code\n    ^~~~~~~~~~~~~~~~~~~~~~\n\n1 error in 1 file')" \
    "$MIXED_LANG_DIR/lang-alias.yaml"

run_and_check "mixed-language: three different languages" \
    1 \
    "$(printf 'mixed-language/config.yaml:3:10: error: Debug mode must be disabled\n3 |   debug: true\n             ^~~~\n\nmixed-language/sample.js:1:1: error: TODO comment found\n1 | // TODO: Fix this code\n    ^~~~~~~~~~~~~~~~~~~~~~\n\nmixed-language/todo-doc.md:3:1: warning: TODO comment found\n3 >| <!-- TODO: Complete this section -->\n4 >| \n\n2 errors in 3 files')" \
    "$MIXED_LANG_DIR/three-langs.yaml"

report
