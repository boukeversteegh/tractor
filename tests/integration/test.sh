#!/usr/bin/env bash
# Run all integration tests

set -uo pipefail

cd "$(dirname "$0")/../.."

BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}Tractor Integration Tests${NC}"
echo ""

# Run each test suite
for suite in rust python typescript javascript go java csharp ruby xml yaml markdown string-input replace; do
    bash "tests/integration/$suite/test.sh" || exit 1
    echo ""
done

echo -e "${BLUE}All tests passed!${NC}"
