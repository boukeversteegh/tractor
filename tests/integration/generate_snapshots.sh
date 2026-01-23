#!/usr/bin/env bash
# Generate XML snapshots from fixture files
# Each language has its own folder with sample.x, sample.x.xml, sample.x.raw.xml

set -euo pipefail

cd "$(dirname "$0")/../.."

TESTS_DIR="tests/integration"

# Detect Windows and add .exe suffix
if [[ "$OSTYPE" == "msys" || "$OSTYPE" == "cygwin" || "$OSTYPE" == "win32" ]]; then
    TRACTOR_BIN="${TRACTOR_BIN:-./target/release/tractor.exe}"
else
    TRACTOR_BIN="${TRACTOR_BIN:-./target/release/tractor}"
fi

# Build tractor if needed
if [ ! -f "$TRACTOR_BIN" ]; then
    echo "Building tractor..."
    cargo build --release
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
)

# Process each language folder
for lang in "${!LANGUAGES[@]}"; do
    ext="${LANGUAGES[$lang]}"
    lang_dir="$TESTS_DIR/$lang"

    if [ -d "$lang_dir" ]; then
        # Find all source files in the language folder
        for fixture in "$lang_dir"/*."$ext"; do
            if [ -f "$fixture" ]; then
                filename=$(basename "$fixture")

                echo "Generating snapshots for $lang/$filename..."

                # Semantic XML (default)
                "$TRACTOR_BIN" "$fixture" > "$lang_dir/${filename}.xml"

                # Raw TreeSitter XML
                "$TRACTOR_BIN" "$fixture" --raw > "$lang_dir/${filename}.raw.xml"

                echo "  → $lang_dir/${filename}.xml"
                echo "  → $lang_dir/${filename}.raw.xml"
            fi
        done
    fi
done

echo ""
echo "Snapshot generation complete!"
