#!/usr/bin/env bash
# Run all integration tests

set -uo pipefail

cd "$(dirname "$0")/../.."

BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}Tractor Integration Tests${NC}"
echo ""

# Run each language test script
for lang in rust python typescript javascript go java csharp ruby xml yaml markdown; do
    bash "tests/integration/$lang/test.sh" || exit 1
    echo ""
done

echo -e "${BLUE}All tests passed!${NC}"
