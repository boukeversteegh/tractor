#!/usr/bin/env bash
# Common setup for integration tests - source from language test scripts

set -uo pipefail

# Determine paths based on caller's location
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[1]}")" && pwd)"
# Derive REPO_ROOT from common.sh's location so it works at any directory depth
_COMMON_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$_COMMON_DIR/../.." && pwd)"
unset _COMMON_DIR

to_display_path() {
    local path="$1"
    if command -v cygpath &>/dev/null; then
        cygpath -ml "$path"
    elif command -v wslpath &>/dev/null && [[ "$TRACTOR_BIN" == *.exe ]]; then
        wslpath -m "$path"
    else
        printf '%s\n' "$path"
    fi
}

to_tractor_path() {
    local path="$1"
    if [[ "$path" != /* ]] && [[ ! "$path" =~ ^[A-Za-z]:[\\/] ]]; then
        printf '%s\n' "$path"
        return
    fi

    if command -v cygpath &>/dev/null; then
        cygpath -m "$path"
    elif command -v wslpath &>/dev/null && [[ "$TRACTOR_BIN" == *.exe ]] && [[ "$path" = /* ]]; then
        wslpath -m "$path"
    else
        printf '%s\n' "$path"
    fi
}

# Keep execution paths in shell-native format. A Windows-style path like
# D:/repo/target/release cannot be prepended to PATH safely because ':' is the
# PATH separator in bash.
TRACTOR_BIN="$REPO_ROOT/target/release/tractor"
if [ ! -f "$TRACTOR_BIN" ] && [ -f "$REPO_ROOT/target/release/tractor.exe" ]; then
    TRACTOR_BIN="$REPO_ROOT/target/release/tractor.exe"
fi

CARGO_BIN="$(command -v cargo || command -v cargo.exe || true)"
if [ -z "$CARGO_BIN" ]; then
    CARGO_BIN="$HOME/.cargo/bin/cargo"
fi

cd "$SCRIPT_DIR"
[ -f "$TRACTOR_BIN" ] || (cd "$REPO_ROOT" && "$CARGO_BIN" build --release -q)
if [ ! -f "$TRACTOR_BIN" ] && [ -f "$REPO_ROOT/target/release/tractor.exe" ]; then
    TRACTOR_BIN="$REPO_ROOT/target/release/tractor.exe"
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
