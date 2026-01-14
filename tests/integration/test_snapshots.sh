#!/usr/bin/env bash
# Snapshot test - regenerates snapshots and fails if they differ from committed versions
# This ensures XML output changes are intentional and committed

set -euo pipefail

cd "$(dirname "$0")/../.."

SNAPSHOTS_DIR="tests/integration/snapshots"

# Regenerate snapshots
bash tests/integration/generate_snapshots.sh > /dev/null

# Check for differences (suppress CRLF warnings)
if git -c core.safecrlf=false diff --quiet "$SNAPSHOTS_DIR" 2>/dev/null; then
    echo -e "\033[32m✓\033[0m Snapshots match"
    exit 0
else
    echo -e "\033[31m✗\033[0m Snapshots have changed:"
    echo ""
    git -c core.safecrlf=false diff --stat "$SNAPSHOTS_DIR" 2>/dev/null
    echo ""
    echo "If this change is intentional, commit the updated snapshots."
    exit 1
fi
