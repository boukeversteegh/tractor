#!/usr/bin/env bash
# Common setup for integration tests - source from language test scripts

set -uo pipefail

# Determine paths based on caller's location
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[1]}")" && pwd)"
# Derive REPO_ROOT from common.sh's location so it works at any directory depth
_COMMON_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$_COMMON_DIR/../.." && pwd)"
unset _COMMON_DIR

# Add release binary to PATH and build if needed (before cygpath conversion).
export PATH="$REPO_ROOT/target/release:$PATH"
cd "$SCRIPT_DIR"
[ -f "$REPO_ROOT/target/release/tractor" ] || [ -f "$REPO_ROOT/target/release/tractor.exe" ] || (cd "$REPO_ROOT" && cargo build --release -q)

# On Windows (gitbash/MSYS2), shell paths use /D/... but tractor outputs D:/...
# Convert to the mixed format so sed replacements match tractor output.
# This must happen AFTER the PATH/build setup above, which needs Unix-style paths.
if command -v cygpath &>/dev/null; then
    SCRIPT_DIR="$(cygpath -m "$SCRIPT_DIR")"
    REPO_ROOT="$(cygpath -m "$REPO_ROOT")"
fi

PASSED=0 FAILED=0
run_test() {
    if "$@"; then
        ((PASSED++))
    else
        ((FAILED++))
    fi
}
report() { echo ""; echo "Passed: $PASSED | Failed: $FAILED"; [ "$FAILED" -eq 0 ]; }
