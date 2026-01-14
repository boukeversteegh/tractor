#!/usr/bin/env bash
# Generate XML snapshots from fixture files
# This script converts all source files in fixtures/ to XML and saves them in snapshots/
# Generates both semantic (default) and raw TreeSitter XML
# Paths are normalized to be relative for reproducible snapshots across environments

set -euo pipefail

cd "$(dirname "$0")/../.."

FIXTURES_DIR="tests/integration/fixtures"
SNAPSHOTS_DIR="tests/integration/snapshots"
RAW_SNAPSHOTS_DIR="tests/integration/snapshots/raw"
TRACTOR_BIN="${TRACTOR_BIN:-./target/release/tractor}"

# Build tractor if needed
if [ ! -f "$TRACTOR_BIN" ]; then
    echo "Building tractor..."
    cargo build --release
fi

# Create snapshots directories
mkdir -p "$SNAPSHOTS_DIR"
mkdir -p "$RAW_SNAPSHOTS_DIR"

# Process each fixture file (using relative paths for reproducible snapshots)
for fixture in "$FIXTURES_DIR"/*; do
    if [ -f "$fixture" ]; then
        filename=$(basename "$fixture")

        # Semantic XML (default)
        echo "Generating snapshot for $filename..."
        "$TRACTOR_BIN" "$fixture" > "$SNAPSHOTS_DIR/${filename}.xml"

        # Raw TreeSitter XML
        "$TRACTOR_BIN" "$fixture" --raw > "$RAW_SNAPSHOTS_DIR/${filename}.xml"

        echo "  → $SNAPSHOTS_DIR/${filename}.xml"
        echo "  → $RAW_SNAPSHOTS_DIR/${filename}.xml"
    fi
done

echo ""
echo "Snapshot generation complete!"
echo "Generated semantic snapshots in: $SNAPSHOTS_DIR"
echo "Generated raw snapshots in: $RAW_SNAPSHOTS_DIR"
