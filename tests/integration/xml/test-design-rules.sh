#!/usr/bin/env bash
# Design rules linting tests (from docs/usecase-integration-test-framework-linting.md)
source "$(dirname "$0")/../common.sh"

echo "Design Rules Linting Tests:"

# Create test C# file with various violations
cat > test-design-rules.cs << 'EOF'
namespace MyApp.Models
{
    // Rule M1 violation: non-nullable Guid
    public class CustomerModel
    {
        public Guid CustomerId { get; set; }  // VIOLATION
        public string Name { get; set; }
        public List<string> Tags { get; set; }  // Rule M7 violation: uninitialized collection
    }

    // Compliant model
    public class OrderModel
    {
        public Guid? OrderId { get; set; }  // Correct!
        public List<OrderItem> Items { get; set; } = [];  // Correct!
    }
}

namespace MyApp.Builders
{
    // Rule B6 violation: accepts Guid parameter
    public class OrderBuilder : Builder
    {
        public OrderBuilder ForCustomer(Guid customerId)  // VIOLATION
        {
            return this;
        }

        // Correct: accepts Model
        public OrderBuilder ForProduct(ProductModel product)
        {
            return this;
        }
    }

    // Rule B7 violation: stores builder reference
    public class ShipmentBuilder : Builder
    {
        private OrderBuilder _orderBuilder;  // VIOLATION

        // Correct: stores Model reference
        private OrderModel _order;
    }

    // Rule B11 violation: void return type
    public class ProductBuilder : Builder
    {
        public void WithName(string name)  // VIOLATION
        {
            _name = name;
        }

        // Correct: returns builder
        public ProductBuilder WithPrice(decimal price)
        {
            _price = price;
            return this;
        }
    }
}
EOF

# Test Rule M1: Model IDs must be nullable
echo "Test: Rule M1 - Non-nullable Guid detection"
OUTPUT=$(tractor test-design-rules.cs \
    -x "//property[type[not(nullable) and .='Guid']]" \
    --error "Property '{//name}' in {file}:{line} should be Guid?" \
    --expect none 2>&1 | grep "CustomerId")
if echo "$OUTPUT" | grep -q "Property 'CustomerId'"; then
    echo "✓ Rule M1 detection works with named captures"
    ((PASSED++))
else
    echo "✗ Rule M1 detection failed"
    echo "  Output: $OUTPUT"
    ((FAILED++))
fi

# Test Rule M7: Collections must be initialized
echo "Test: Rule M7 - Uninitialized collection detection"
COUNT=$(tractor test-design-rules.cs \
    -x "//property[type[contains(.,'List')] and not(value)]" \
    -o count 2>/dev/null)
if [ "$COUNT" = "1" ]; then
    echo "✓ Rule M7 detection works (found 1 violation)"
    ((PASSED++))
else
    echo "✗ Rule M7 detection failed (expected 1, got $COUNT)"
    ((FAILED++))
fi

# Test Rule M7 with named captures
echo "Test: Rule M7 with named captures"
OUTPUT=$(tractor test-design-rules.cs \
    -x "//property[type[contains(.,'List')] and not(value)]" \
    --error "Property '{//name}' must be initialized" \
    --expect none 2>&1 | grep "error:")
if echo "$OUTPUT" | grep -q "Property 'Tags'"; then
    echo "✓ Rule M7 named captures work"
    ((PASSED++))
else
    echo "✗ Rule M7 named captures failed"
    echo "  Output: $OUTPUT"
    ((FAILED++))
fi

# Test Rule B6: Builders must not accept Guid parameters
echo "Test: Rule B6 - Guid parameter detection"
OUTPUT=$(tractor test-design-rules.cs \
    -x "//method[public and .//parameter[type[.='Guid']]]" \
    --error "Method '{//name}' accepts Guid - should accept Model instead" \
    --expect none 2>&1 | grep "error:")
if echo "$OUTPUT" | grep -q "Method 'ForCustomer'"; then
    echo "✓ Rule B6 detection works with named captures"
    ((PASSED++))
else
    echo "✗ Rule B6 detection failed"
    echo "  Output: $OUTPUT"
    ((FAILED++))
fi

# Test Rule B7: No builder-to-builder dependencies
echo "Test: Rule B7 - Builder field detection"
OUTPUT=$(tractor test-design-rules.cs \
    -x "//field[.//type[contains(.,'Builder')]]" \
    --error "Field '{//name}' stores Builder reference" \
    --expect none 2>&1 | grep "error:")
if echo "$OUTPUT" | grep -q "Field '_orderBuilder'"; then
    echo "✓ Rule B7 detection works with named captures"
    ((PASSED++))
else
    echo "✗ Rule B7 detection failed"
    echo "  Output: $OUTPUT"
    ((FAILED++))
fi

# Test Rule B11: Fluent methods must return builder type
echo "Test: Rule B11 - Void return type detection"
OUTPUT=$(tractor test-design-rules.cs \
    -x "//method[public and returns[.='void']]" \
    --error "Method '{//name}' returns void - should return builder for fluent API" \
    --expect none 2>&1 | grep "error:")
if echo "$OUTPUT" | grep -q "Method 'WithName'"; then
    echo "✓ Rule B11 detection works with named captures"
    ((PASSED++))
else
    echo "✗ Rule B11 detection failed"
    echo "  Output: $OUTPUT"
    ((FAILED++))
fi

# Test combining file/line info with XPath captures
echo "Test: Full error context with file:line and XPath"
OUTPUT=$(tractor test-design-rules.cs \
    -x "//property[type='Guid']" \
    --error "{file}:{line}: Property '{//name}' has non-nullable Guid" \
    --expect none 2>&1 | grep "test-design-rules.cs")
if echo "$OUTPUT" | grep -q "test-design-rules.cs:[0-9]*: Property 'CustomerId'"; then
    echo "✓ Full error context works"
    ((PASSED++))
else
    echo "✗ Full error context failed"
    echo "  Output: $OUTPUT"
    ((FAILED++))
fi

# Test with warning mode (should not fail)
echo "Test: Design rules with --warning flag"
if tractor test-design-rules.cs \
    -x "//property[type='Guid']" \
    --error "Property '{//name}' violation" \
    --expect none --warning >/dev/null 2>&1; then
    echo "✓ Warning mode works with named captures"
    ((PASSED++))
else
    echo "✗ Warning mode failed"
    ((FAILED++))
fi

# Cleanup
rm -f test-design-rules.cs

report
