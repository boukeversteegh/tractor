#!/usr/bin/env bash
# Generate XML snapshots from fixture files
# This script converts all source files in fixtures/ to XML and saves them in snapshots/

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
FIXTURES_DIR="$SCRIPT_DIR/fixtures"
SNAPSHOTS_DIR="$SCRIPT_DIR/snapshots"
TRACTOR_BIN="${TRACTOR_BIN:-$PROJECT_ROOT/target/release/tractor}"

# Build tractor if needed
if [ ! -f "$TRACTOR_BIN" ]; then
    echo "Building tractor..."
    cd "$PROJECT_ROOT"
    cargo build --release
    cd "$SCRIPT_DIR"
fi

# Create snapshots directory if it doesn't exist
mkdir -p "$SNAPSHOTS_DIR"

# Process each fixture file
for fixture in "$FIXTURES_DIR"/*; do
    if [ -f "$fixture" ]; then
        filename=$(basename "$fixture")
        snapshot="$SNAPSHOTS_DIR/${filename}.xml"

        echo "Generating snapshot for $filename..."
        "$TRACTOR_BIN" "$fixture" > "$snapshot"

        echo "  → $snapshot"
    fi
done

echo ""
echo "Snapshot generation complete!"
echo "Generated snapshots in: $SNAPSHOTS_DIR"
