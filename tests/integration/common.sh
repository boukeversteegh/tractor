#!/usr/bin/env bash
# Common setup for integration tests - source from language test scripts

set -uo pipefail

# Determine paths based on caller's location
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[1]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
export PATH="$REPO_ROOT/target/release:$PATH"

cd "$SCRIPT_DIR"
[ -f "$REPO_ROOT/target/release/tractor" ] || (cd "$REPO_ROOT" && cargo build --release -q)

PASSED=0 FAILED=0
run_test() {
    if "$@"; then
        ((PASSED++))
    else
        ((FAILED++))
    fi
}
report() { echo ""; echo "Passed: $PASSED | Failed: $FAILED"; [ "$FAILED" -eq 0 ]; }
