#!/usr/bin/env bash
# Snapshot check — verifies current snapshots match what tractor would generate.
# Does NOT modify the working tree. Use `task test:snapshots:update` to regenerate.
#
# Generates fresh snapshots into a temp directory and compares against committed files.

set -euo pipefail

cd "$(dirname "$0")/../.."

TESTS_DIR="tests/integration"

# Detect Windows and add .exe suffix
if [[ "$OSTYPE" == "msys" || "$OSTYPE" == "cygwin" || "$OSTYPE" == "win32" ]]; then
    TRACTOR_BIN="${TRACTOR_BIN:-./target/release/tractor.exe}"
else
    TRACTOR_BIN="${TRACTOR_BIN:-./target/release/tractor}"
fi

if [ ! -f "$TRACTOR_BIN" ]; then
    echo "Error: tractor binary not found at $TRACTOR_BIN (run 'task build' first)"
    exit 1
fi

# Language folders and their file extensions
declare -A LANGUAGES=(
    ["rust"]="rs"
    ["python"]="py"
    ["typescript"]="ts"
    ["javascript"]="js"
    ["go"]="go"
    ["java"]="java"
    ["csharp"]="cs"
    ["ruby"]="rb"
    ["yaml"]="yaml"
    ["markdown"]="md"
)

TMPDIR=$(mktemp -d)
trap "rm -rf $TMPDIR" EXIT

HAS_CHANGES=0
MISMATCHES=""

for lang in "${!LANGUAGES[@]}"; do
    ext="${LANGUAGES[$lang]}"
    lang_dir="$TESTS_DIR/$lang"

    if [ -d "$lang_dir" ]; then
        mkdir -p "$TMPDIR/$lang"
        for fixture in "$lang_dir"/*."$ext"; do
            if [ -f "$fixture" ]; then
                filename=$(basename "$fixture")

                # Check semantic XML
                expected="$lang_dir/${filename}.xml"
                if [ -f "$expected" ]; then
                    "$TRACTOR_BIN" "$fixture" > "$TMPDIR/$lang/${filename}.xml" 2>/dev/null
                    if ! diff -q "$TMPDIR/$lang/${filename}.xml" "$expected" > /dev/null 2>&1; then
                        HAS_CHANGES=1
                        MISMATCHES="$MISMATCHES  $expected\n"
                    fi
                fi

                # Check raw XML
                expected_raw="$lang_dir/${filename}.raw.xml"
                if [ -f "$expected_raw" ]; then
                    "$TRACTOR_BIN" "$fixture" --raw > "$TMPDIR/$lang/${filename}.raw.xml" 2>/dev/null
                    if ! diff -q "$TMPDIR/$lang/${filename}.raw.xml" "$expected_raw" > /dev/null 2>&1; then
                        HAS_CHANGES=1
                        MISMATCHES="$MISMATCHES  $expected_raw\n"
                    fi
                fi
            fi
        done
    fi
done

if [ "$HAS_CHANGES" -eq 0 ]; then
    echo -e "\033[32m✓\033[0m Snapshots match"
    exit 0
else
    echo -e "\033[31m✗\033[0m Snapshot mismatch:"
    echo ""
    echo -e "$MISMATCHES"
    echo "If intentional, run 'task test:snapshots:update' to regenerate."
    exit 1
fi
