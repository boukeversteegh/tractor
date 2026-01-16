#!/usr/bin/env bash
# Named captures integration tests
source "$(dirname "$0")/../common.sh"

echo "Named Captures Feature Tests:"

# Create test C# file with violations
cat > test-named-captures.cs << 'EOF'
namespace MyApp.Models
{
    public class CustomerModel
    {
        public Guid CustomerId { get; set; }  // Should be Guid?
        public string Name { get; set; }
    }

    public class OrderModel
    {
        public Guid? OrderId { get; set; }  // Correct!
    }
}
EOF

# Test 1: Basic named capture with //name
echo "Test: Basic XPath placeholder {//name}"
OUTPUT=$(tractor test-named-captures.cs -x "//property[type='Guid']" --error "Property '{//name}' should be nullable" --expect none 2>&1 | grep "error:")
if echo "$OUTPUT" | grep -q "Property 'CustomerId' should be nullable"; then
    echo "✓ Basic XPath placeholder works"
    ((PASSED++))
else
    echo "✗ Basic XPath placeholder failed"
    echo "  Output: $OUTPUT"
    ((FAILED++))
fi

# Test 2: Multiple XPath placeholders
echo "Test: Multiple XPath placeholders {//name} and {//type}"
OUTPUT=$(tractor test-named-captures.cs -x "//property[type='Guid']" --error "Property '{//name}' has type '{//type}'" --expect none 2>&1 | grep "error:")
if echo "$OUTPUT" | grep -q "Property 'CustomerId' has type 'Guid'"; then
    echo "✓ Multiple XPath placeholders work"
    ((PASSED++))
else
    echo "✗ Multiple XPath placeholders failed"
    echo "  Output: $OUTPUT"
    ((FAILED++))
fi

# Test 3: Combined standard and XPath placeholders
echo "Test: Combined {file}:{line} with {//name}"
OUTPUT=$(tractor test-named-captures.cs -x "//property[type='Guid']" --error "{file}:{line}: Property '{//name}'" --expect none 2>&1 | grep "error:")
if echo "$OUTPUT" | grep -q "test-named-captures.cs:[0-9]*: Property 'CustomerId'"; then
    echo "✓ Combined standard and XPath placeholders work"
    ((PASSED++))
else
    echo "✗ Combined placeholders failed"
    echo "  Output: $OUTPUT"
    ((FAILED++))
fi

# Test 4: Non-matching XPath expression (should leave placeholder unchanged)
echo "Test: Non-matching XPath {//nonexistent}"
OUTPUT=$(tractor test-named-captures.cs -x "//property[type='Guid']" --error "Field '{//nonexistent}'" --expect none 2>&1 | grep "error:")
if echo "$OUTPUT" | grep -q "Field '{//nonexistent}'"; then
    echo "✓ Non-matching XPath leaves placeholder unchanged"
    ((PASSED++))
else
    echo "✗ Non-matching XPath handling failed"
    echo "  Output: $OUTPUT"
    ((FAILED++))
fi

# Test 5: Using with --warning flag (should not fail exit code)
echo "Test: Named captures with --warning flag"
if tractor test-named-captures.cs -x "//property[type='Guid']" --error "Property '{//name}' issue" --expect none --warning >/dev/null 2>&1; then
    echo "✓ Named captures work with --warning flag"
    ((PASSED++))
else
    echo "✗ Named captures with --warning flag failed"
    ((FAILED++))
fi

# Test 6: Extract nested type element
echo "Test: Extract nested type element"
OUTPUT=$(tractor test-named-captures.cs -x "//property[type='string']" --error "String property: '{//name}'" --expect none 2>&1 | grep "error:")
if echo "$OUTPUT" | grep -q "String property: 'Name'"; then
    echo "✓ Nested element extraction works"
    ((PASSED++))
else
    echo "✗ Nested element extraction failed"
    echo "  Output: $OUTPUT"
    ((FAILED++))
fi

# Test 7: All standard placeholders work with XPath
echo "Test: All placeholders {file}, {line}, {col}, {value}, {//text()}"
OUTPUT=$(tractor test-named-captures.cs -x "//property/name" --error "{file}:{line}:{col} value='{value}' xpath='{//text()}'" --expect 1 2>&1 | grep "error:" | head -1)
if echo "$OUTPUT" | grep -q "test-named-captures.cs:[0-9]*:[0-9]* value="; then
    echo "✓ All placeholder types work together"
    ((PASSED++))
else
    echo "✗ Combined placeholders failed (expected 1, got 3 property names)"
    echo "  Output: $OUTPUT"
    ((FAILED++))
fi

# Cleanup
rm -f test-named-captures.cs

report
