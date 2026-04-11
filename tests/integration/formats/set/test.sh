#!/usr/bin/env bash
# Snapshot tests for set command output formats.
#
# Default: run each case, diff stdout against the committed snapshot.
# UPDATE=1: overwrite each snapshot file with the observed output
#           (invoked by `task test:snapshots:update`).
source "$(dirname "$0")/../../common.sh"

SNAPSHOT_DIR="$REPO_ROOT/tests/integration/formats/set"
UPDATE_MODE="${UPDATE:-0}"

# Compare (or overwrite) a snapshot file against observed output.
# Centralises the UPDATE=1 vs check-mode split so individual cases don't
# duplicate the logic.
compare_or_update() {
    local desc="$1"
    local snapshot="$2"
    local actual="$3"

    if [ "$UPDATE_MODE" = "1" ]; then
        printf '%s' "$actual" > "$snapshot"
        echo "  ✎ $desc (updated $(basename "$snapshot"))"
        ((PASSED++))
        return
    fi

    local expected
    expected=$(cat "$snapshot" 2>/dev/null || echo "")

    if [ "$actual" = "$expected" ]; then
        echo "  ✓ $desc"
        ((PASSED++))
    else
        echo "  ✗ $desc"
        diff <(echo "$expected") <(echo "$actual") --color=always -u --label expected --label actual | sed 's/^/      /'
        ((FAILED++))
    fi
}

# Helper: run set on a copy of sample.yaml, compare stdout to snapshot.
check_set_snapshot() {
    local desc="$1"
    local snapshot="$2"
    shift 2
    local args=("$@")

    # Copy sample so the in-place mode doesn't clobber it
    local tmpfile
    tmpfile="$(mktemp "$SNAPSHOT_DIR/tmp.XXXXXX.yaml")"
    cp "$SNAPSHOT_DIR/sample.yaml" "$tmpfile"
    local tmpfile_display
    tmpfile_display="$(to_display_path "$tmpfile")"

    # Run tractor set, capture stdout, normalise the temp path back to the
    # repo-relative path used in the committed snapshot
    local actual
    actual=$(tractor set "$(to_tractor_path "$tmpfile")" "${args[@]}" 2>/dev/null \
        | sed "s|$tmpfile_display|tests/integration/formats/set/sample.yaml|g")

    compare_or_update "$desc" "$snapshot" "$actual"
    rm -f "$tmpfile"
}

echo "Set (snapshot: text format):"

check_set_snapshot "text default (file:line + status + summary)" \
    "$SNAPSHOT_DIR/set.txt" \
    -x "//database/host" --value "db.example.com" --no-color

check_set_snapshot "text declarative mode" \
    "$SNAPSHOT_DIR/set-declarative.txt" \
    "database[host='db.example.com']" --no-color

check_set_snapshot "text unchanged (value already set)" \
    "$SNAPSHOT_DIR/set-unchanged.txt" \
    -x "//database/host" --value "localhost" --no-color

echo ""
echo "Set (snapshot: text stdout mode):"

# Stdout mode: compare raw output (no path mangling needed — file is not modified)
check_set_snapshot "text stdout mode" \
    "$SNAPSHOT_DIR/set-stdout.txt" \
    -x "//database/host" --value "db.example.com" --stdout --no-color

# Stdin capture path: no files, declarative expression, executor capture mode
stdin_actual=$(printf 'database:\n  host: localhost\n  port: 5432\n' \
    | tractor set -l yaml "database[host='db.example.com']" --stdout --no-color 2>/dev/null)
compare_or_update "text stdout mode from stdin" \
    "$SNAPSHOT_DIR/set-stdin-stdout.txt" "$stdin_actual"

# Multi-file stdout should stay structured and include file headers.
tmpfile_a="$(mktemp "$SNAPSHOT_DIR/tmp-a.XXXXXX.yaml")"
tmpfile_b="$(mktemp "$SNAPSHOT_DIR/tmp-b.XXXXXX.yaml")"
cp "$SNAPSHOT_DIR/sample.yaml" "$tmpfile_a"
cp "$SNAPSHOT_DIR/sample.yaml" "$tmpfile_b"
tmpfile_a_display="$(to_display_path "$tmpfile_a")"
tmpfile_b_display="$(to_display_path "$tmpfile_b")"
multi_actual=$(tractor set "$(to_tractor_path "$tmpfile_a")" "$(to_tractor_path "$tmpfile_b")" \
    -x "//database/host" --value "db.example.com" --stdout --no-color 2>/dev/null \
    | sed "s|$tmpfile_a_display|tests/integration/formats/set/sample-a.yaml|g" \
    | sed "s|$tmpfile_b_display|tests/integration/formats/set/sample-b.yaml|g")
compare_or_update "text stdout mode with multiple files" \
    "$SNAPSHOT_DIR/set-stdout-multi.txt" "$multi_actual"
rm -f "$tmpfile_a" "$tmpfile_b"

echo ""
echo "Set (snapshot: json format):"

check_set_snapshot "json default" \
    "$SNAPSHOT_DIR/set.json" \
    -x "//database/host" --value "db.example.com" -f json

echo ""
echo "Set (snapshot: xml format):"

# Helper: run `tractor run <config>` in a temp dir containing both the
# config and fixture, capture stdout, normalise the temp path to the
# committed snapshot's repo-relative form. Used for multi-mapping config
# cases that `tractor set` can't express inline.
check_run_config_snapshot() {
    local desc="$1"
    local snapshot="$2"
    local config="$3"
    local fixture="$4"
    shift 4
    local args=("$@")

    local tmpdir
    tmpdir="$(mktemp -d "$SNAPSHOT_DIR/tmp.XXXXXX")"
    cp "$SNAPSHOT_DIR/$config" "$SNAPSHOT_DIR/$fixture" "$tmpdir/"
    local tmpdir_display
    tmpdir_display="$(to_display_path "$tmpdir")"

    local actual
    actual=$((cd "$tmpdir" && tractor run "$config" "${args[@]}") 2>/dev/null \
        | sed "s|$tmpdir_display/|tests/integration/formats/set/|g")

    compare_or_update "$desc" "$snapshot" "$actual"
    rm -rf "$tmpdir"
}

# Multi-mapping in-place update via `tractor run <config>`. Exercises the
# honest XML rendering of a batch set operation — two matches nested under
# file → command groups, no <outputs> (in-place writes produce none).
check_run_config_snapshot "xml multi-mapping in-place (run set-config)" \
    "$SNAPSHOT_DIR/set.xml" \
    "set-config.yaml" "sample.yaml" \
    -f xml --no-color

# Capture / --stdout mode: the modified file content is carried as a
# ReportOutput that `with_grouping` moves into the file group, surfacing
# as <outputs><output>...</output></outputs> under the group element.
check_set_snapshot "xml stdout capture (outputs moved into file group)" \
    "$SNAPSHOT_DIR/set-stdout.xml" \
    -x "//database/host" --value "db.example.com" --stdout -f xml --no-color

report
