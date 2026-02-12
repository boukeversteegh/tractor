#!/usr/bin/env bash
# Markdown integration tests
source "$(dirname "$0")/../common.sh"

echo "Markdown:"
run_test tractor sample.md -x "//heading" --expect 2 -m "headings are recognized"
run_test tractor sample.md -x "//heading[h1]" --expect 1 -m "h1 heading found"
run_test tractor sample.md -x "//heading[h2]" --expect 1 -m "h2 heading found"
run_test tractor sample.md -x "//list[ordered]" --expect 1 -m "ordered list found"
run_test tractor sample.md -x "//list[unordered]" --expect 1 -m "unordered list found"
run_test tractor sample.md -x "//item" --expect 5 -m "list items found"
run_test tractor sample.md -x "//blockquote" --expect 1 -m "blockquote found"
run_test tractor sample.md -x "//code_block" --expect 3 -m "code blocks found"
run_test tractor sample.md -x "//code_block[language='python']" --expect 1 -m "python code block found"
run_test tractor sample.md -x "//code_block[language='javascript']" --expect 1 -m "javascript code block found"
run_test tractor sample.md -x "//code_block[not(language)]" --expect 1 -m "unlabeled code block found"
run_test tractor sample.md -x "//hr" --expect 1 -m "horizontal rule found"

# Round-trip test: extract code from markdown code block, parse it as the declared language
echo ""
echo "  Round-trip: extract JS code block from markdown, parse as JavaScript..."
JS_CODE=$(tractor sample.md -x "//code_block[language='javascript']/code" -o value)
JS_FUNCTIONS=$(echo "$JS_CODE" | tractor -l javascript -x "//function" -o count)
if [ "$JS_FUNCTIONS" = "1" ]; then
    echo "  ✓ round-trip: extracted JS code parses as JavaScript with 1 function"
    ((PASSED++))
else
    echo "  ✗ round-trip: expected 1 function, got '$JS_FUNCTIONS'"
    ((FAILED++))
fi

report
