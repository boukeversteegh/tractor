# Integration Tests

This directory contains integration tests for tractor that verify:

1. **Snapshot Testing** - Detect unintended changes in XML output
2. **Structure Assertions** - Query XML with XPath to verify structure
3. **XML Pass-through** - Load and query pre-generated XML snapshots

## Directory Structure

```
tests/integration/
├── fixtures/          # Source files in various languages
├── snapshots/         # Expected XML output (committed to git)
├── generate_snapshots.sh  # Regenerate snapshots from fixtures
├── run_tests.sh       # Full test suite (comprehensive)
├── test_simple.sh     # Simple test (quick sanity check)
└── README.md          # This file
```

## Running Tests

### Quick Test
```bash
./test_simple.sh
```

### Full Test Suite
```bash
./run_tests.sh
```

### Regenerate Snapshots
When you make intentional changes to the parser or transformations:
```bash
./generate_snapshots.sh
```

Then review the changes with git diff and commit if expected.

## How It Works

### 1. Snapshot Testing

The test suite converts source files to XML and compares against committed snapshots:

```bash
tractor fixtures/sample.rs > current.xml
diff current.xml snapshots/sample.rs.xml
```

This detects unintended changes in XML output. If the diff shows changes:
- If changes are **expected**: regenerate snapshots and commit
- If changes are **unexpected**: investigate and fix the regression

### 2. Structure Assertions

The test suite uses XPath queries to verify XML structure:

```bash
# Check that Rust file has 2 functions
tractor fixtures/sample.rs -x "//function" --output count
# Expected: 2

# Check for specific function names
tractor fixtures/sample.rs -x "//function/name[type='add']" --output count
# Expected: 1
```

This ensures the semantic structure is correct regardless of formatting.

### 3. XML Pass-through

The test suite can also load XML snapshots directly and query them:

```rust
use tractor_core::{load_xml_file, XPathEngine, generate_xml_document};

// Load snapshot XML
let result = load_xml_file("snapshots/sample.rs.xml")?;

// Query it just like parsed source
let xml = generate_xml_document(&[result]);
let engine = XPathEngine::new();
let matches = engine.query(&xml, "//function", &[], "test");
assert_eq!(matches.len(), 2);
```

This is useful for:
- Testing XPath queries without re-parsing source files
- Verifying backward compatibility with old XML formats
- Testing query engine independently from parser

## Test Fixtures

Sample files are provided in multiple languages:

- **sample.rs** - Rust (function definitions, binary operators)
- **sample.py** - Python (function definitions, docstrings)
- **sample.ts** - TypeScript (typed functions, template strings)
- **sample.js** - JavaScript (functions, template strings)
- **sample.go** - Go (package, typed functions, fmt import)
- **sample.java** - Java (class, static methods, System.out)
- **sample.cs** - C# (class, static methods, Console.WriteLine)
- **sample.rb** - Ruby (method definitions, string interpolation)

Each fixture is designed to test common language constructs while being simple enough to understand and maintain.

## Adding New Tests

### Add a new fixture
1. Create `fixtures/sample.ext` with representative code
2. Run `./generate_snapshots.sh` to create the snapshot
3. Add structure assertions in `run_tests.sh`

### Add a new assertion
Edit `run_tests.sh` and add an `assert_xpath` call:

```bash
assert_xpath "$FIXTURES_DIR/sample.rs" "//loop" "0" "  Should have no loops"
```

## Troubleshooting

### Tests fail after code changes
1. Review what changed: `git diff snapshots/`
2. If changes are expected: `./generate_snapshots.sh && git add snapshots/`
3. If changes are unexpected: investigate the regression

### Snapshot paths differ
The snapshots contain absolute paths from when they were generated. The test suite normalizes paths before comparing. If you see path-only diffs, this is expected and handled by the normalization logic.

### XPath queries fail
Make sure you're using the correct XPath syntax for the semantic structure. Use `tractor file.rs --debug -x "//query"` to see the full XML with syntax highlighting to help debug your query.
