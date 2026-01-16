# XML and Named Captures Integration Tests

This directory contains integration tests for XML passthrough mode and the XPath-based named captures feature.

## Test Files

### `test.sh` - XML Passthrough Tests
Tests basic XML passthrough functionality - querying pre-existing XML files without code parsing.

**Run**: `bash test.sh`

### `test-named-captures.sh` - Named Captures Feature Tests
Tests the XPath-based named captures feature in error messages.

Tests include:
- Basic XPath placeholder extraction (`{//name}`)
- Multiple XPath placeholders in one message
- Combining standard placeholders (`{file}`, `{line}`) with XPath
- Non-matching XPath expressions (fallback behavior)
- Integration with `--warning` flag
- Various XPath expressions and element extraction

**Run**: `bash test-named-captures.sh`

**Example Usage**:
```bash
tractor test.cs -x "//property[type='Guid']" \
  --error "Property '{//name}' in {file}:{line} should be Guid?" \
  --expect none
```

Output:
```
test.cs:5:9: error: Property 'CustomerId' in test.cs:5 should be Guid?
```

### `test-design-rules.sh` - Real-World Design Rules Tests
Tests applying the named captures feature to real-world design rule enforcement (based on `docs/usecase-integration-test-framework-linting.md`).

Tests include:
- **Rule M1**: Model IDs must be nullable (`Guid?` not `Guid`)
- **Rule M7**: Collections must be initialized
- **Rule B6**: Builders must not accept Guid parameters
- **Rule B7**: No builder-to-builder dependencies
- **Rule B11**: Fluent methods must return builder type

**Run**: `bash test-design-rules.sh`

### `run-all-named-capture-tests.sh` - Master Test Runner
Runs all named capture tests in sequence and reports overall pass/fail status.

**Run**: `bash run-all-named-capture-tests.sh`

### `test-named-captures-edge-cases.sh.experimental`
Advanced edge case tests for the named captures feature. Currently marked as experimental as some edge cases are not yet fully supported.

**Not run by default** - add to `run-all-named-capture-tests.sh` if you want to include it.

## Named Captures Feature

The named captures feature allows using XPath expressions in error message templates via the `--error` flag.

### Supported Placeholders

- `{file}` - file path
- `{line}` - line number
- `{col}` - column number
- `{value}` - matched text value
- `{//xpath}` - any XPath expression (use absolute paths like `//name`, `//type`)

### XPath Requirements

- XPath expressions must use **absolute paths** (e.g., `//name`) not relative paths (e.g., `name` or `./name`)
- The XML fragment contains only the matched element, not its ancestors
- If the XPath doesn't match anything, the placeholder is left unchanged

### Example

```bash
tractor "src/**/*.cs" \
  -x "//property[type[not(nullable) and .='Guid']]" \
  --error "Property '{//name}' should be Guid? in {file}:{line}" \
  --expect none
```

Output:
```
Models/Customer.cs:10:5: error: Property 'CustomerId' should be Guid? in Models/Customer.cs:10
```

## Running All Tests

From the repository root:
```bash
cd tests/integration/xml
bash run-all-named-capture-tests.sh
```

Or run individual test files directly.
