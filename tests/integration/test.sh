#!/usr/bin/env bash
# Run all integration tests

set -uo pipefail

cd "$(dirname "$0")/../.."

BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}Tractor Integration Tests${NC}"
echo ""

# Language parse/query tests
for suite in rust python typescript javascript go java csharp ruby xml yaml markdown tsql ini toml; do
    bash "tests/integration/languages/$suite/test.sh" || exit 1
    echo ""
done

# Feature tests
for suite in string-input replace update xpath-expressions view-modifiers; do
    bash "tests/integration/$suite/test.sh" || exit 1
    echo ""
done

# Batch execution tests (tractor run)
bash "tests/integration/run/test.sh" || exit 1
echo ""

# Format snapshot tests
bash "tests/integration/formats/set/test.sh" || exit 1
echo ""

echo -e "${BLUE}All tests passed!${NC}"
