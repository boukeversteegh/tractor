#!/usr/bin/env bash
# Snapshot test - regenerates snapshots and fails if they differ from committed versions
# This ensures XML output changes are intentional and committed

set -euo pipefail

cd "$(dirname "$0")/../.."

TESTS_DIR="tests/integration"

# Language folders to check
LANGUAGES="rust python typescript javascript go java csharp ruby"

# Regenerate snapshots
bash tests/integration/generate_snapshots.sh > /dev/null

# Check each language folder for differences (suppress CRLF warnings)
HAS_CHANGES=0
for lang in $LANGUAGES; do
    if ! git -c core.safecrlf=false diff --quiet "$TESTS_DIR/$lang" 2>/dev/null; then
        HAS_CHANGES=1
        break
    fi
done

if [ "$HAS_CHANGES" -eq 0 ]; then
    echo -e "\033[32m✓\033[0m Snapshots match"
    exit 0
else
    echo -e "\033[31m✗\033[0m Snapshots have changed:"
    echo ""
    for lang in $LANGUAGES; do
        git -c core.safecrlf=false diff --stat "$TESTS_DIR/$lang" 2>/dev/null || true
    done
    echo ""
    echo "If this change is intentional, commit the updated snapshots."
    exit 1
fi
