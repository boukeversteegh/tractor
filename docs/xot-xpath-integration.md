# XOT to XPath Integration: Avoiding the Serialize/Parse Roundtrip

## Problem Statement

When using TreeSitter to parse source code into abstract syntax trees (AST), we convert the AST into XOT (XML Object Tree) nodes for XPath querying. However, the `xee-xpath` library requires an XML document to run XPath queries.

**Current inefficient workflow:**
```
TreeSitter AST → XOT tree → serialize to XML string → parse string → xee-xpath Document → XPath query
```

This serialize-then-parse roundtrip is wasteful since we already have the tree structure in memory.

## Research Goals

1. **Primary:** Find a way to run XPath queries on XOT trees without serialization
2. **Secondary:** Enable chained XPath queries (query results as input to new queries)

## Key Discovery

**xee-xpath::Documents provides mutable access to its internal XOT!**

The `Documents` struct has two crucial methods that weren't immediately obvious:

```rust
// Get mutable access to the internal Xot arena
pub fn xot_mut(&mut self) -> &mut Xot

// Convert a DocumentHandle to an actual xot::Node
pub fn document_node(&self, handle: DocumentHandle) -> Option<xot::Node>
```

## Solution: The Shell Document Approach

### Workflow

```rust
use xee_xpath::{Documents, Queries, Query, DocumentHandle};
use xot::Xot;

// 1. Create Documents and add a minimal shell document
let mut documents = Documents::new();
let doc_handle: DocumentHandle = documents.add_string(
    "file:///source.ts".try_into().unwrap(),
    "<Files/>",  // Shell with correct root element name
)?;

// 2. Get the document node (converts DocumentHandle to xot::Node)
let doc_node = documents.document_node(doc_handle)
    .ok_or("Failed to get document node")?;

// 3. Get mutable access to the Xot arena
let xot = documents.xot_mut();

// 4. Get the root element to build into
let root = xot.document_element(doc_node)?;

// 5. Build your tree directly (example)
let file_name = xot.add_name("File");
let file_el = xot.new_element(file_name);
xot.append(root, file_el)?;

// 6. Run XPath queries - no serialization needed!
let queries = Queries::default();
let query = queries.sequence("//File")?;
let results = query.execute(&mut documents, doc_handle)?;
```

### Why This Works

1. `Documents::new()` creates an empty document collection with its own `Xot` arena
2. `add_string()` parses minimal XML into that arena, returning a `DocumentHandle`
3. `document_node()` gives us the actual `xot::Node` for tree manipulation
4. `xot_mut()` provides `&mut Xot` - full mutable access to the arena
5. We build our tree directly into the document's root element
6. XPath queries work on the modified document without any serialization

## Secondary Use Case: Chained Queries

XPath result nodes can be used directly as context for new queries:

```rust
// First query
let class_query = queries.sequence("//class")?;
let classes = class_query.execute(&mut documents, doc_handle)?;

// Use each result as context for a sub-query
let name_query = queries.sequence("name")?;  // relative path
for item in classes.iter() {
    if let xee_xpath::Item::Node(class_node) = item {
        // Use class_node as context - no document wrapping needed!
        let names = name_query.execute(&mut documents, class_node)?;
    }
}
```

## API Reference

### Documents (xee_xpath)

| Method | Signature | Description |
|--------|-----------|-------------|
| `new()` | `fn new() -> Self` | Create empty document collection |
| `add_string()` | `fn add_string(&mut self, uri: &IriStr, xml: &str) -> Result<DocumentHandle, Error>` | Parse XML string into collection |
| `document_node()` | `fn document_node(&self, handle: DocumentHandle) -> Option<xot::Node>` | Get xot::Node from handle |
| `xot()` | `fn xot(&self) -> &Xot` | Immutable access to Xot arena |
| `xot_mut()` | `fn xot_mut(&mut self) -> &mut Xot` | **Mutable access to Xot arena** |

### Xot Manipulation

| Method | Description |
|--------|-------------|
| `add_name(name)` | Register element/attribute name, get NameId |
| `new_element(name_id)` | Create new element node |
| `new_text(content)` | Create new text node |
| `append(parent, child)` | Append child to parent |
| `attributes_mut(node)` | Get mutable attribute map |
| `element_mut(node)` | Get mutable element (for renaming) |
| `document_element(doc)` | Get root element of document |

## Implementation Recommendations

### Option 1: Modify XotBuilder

Add a method to build directly into a `Documents` instance:

```rust
impl XotBuilder {
    pub fn build_into_documents(
        &mut self,
        documents: &mut Documents,
        ts_node: TsNode,
        source: &str,
        file_path: &str,
    ) -> Result<DocumentHandle, Error> {
        // Add shell document
        let handle = documents.add_string(uri, "<Files/>")?;
        let doc_node = documents.document_node(handle).unwrap();
        let xot = documents.xot_mut();
        let root = xot.document_element(doc_node)?;

        // Build tree into root...
        self.build_into_node(xot, root, ts_node, source, file_path)?;

        Ok(handle)
    }
}
```

### Option 2: Create XeeBuilder

A new builder that wraps `Documents`:

```rust
pub struct XeeBuilder {
    documents: Documents,
    name_cache: HashMap<String, NameId>,
}

impl XeeBuilder {
    pub fn new() -> Self {
        Self {
            documents: Documents::new(),
            name_cache: HashMap::new(),
        }
    }

    pub fn build(&mut self, ts_node: TsNode, source: &str, path: &str)
        -> Result<DocumentHandle, Error>
    {
        // Build directly into documents.xot_mut()
    }

    pub fn into_documents(self) -> Documents {
        self.documents
    }
}
```

## Performance Implications

| Approach | Operations | Estimated Cost |
|----------|------------|----------------|
| **Old (serialize/parse)** | Build XOT → Serialize → Parse → Query | O(n) + O(n) + O(n) = O(3n) |
| **New (direct build)** | Build into Documents → Query | O(n) + O(1) = O(n) |

Where n = number of AST nodes.

The new approach eliminates:
- String allocation for XML serialization
- XML parsing overhead
- Duplicate tree construction

## Test Results

All experiments pass successfully. See `tractor-core/src/xot_document_experiments.rs` for:

- `test_shell_document_approach()` - Demonstrates XOT tree manipulation
- `test_build_into_documents_approach()` - Shows Documents API exploration
- `test_documents_api_exploration()` - Proves chained queries work
- `test_documents_mutable_exploration()` - Discovers and tests `xot_mut()`
- `test_full_workflow_no_roundtrip()` - Complete TreeSitter → Documents → XPath demo

## Conclusion

The `xee-xpath` library already supports our use case through `xot_mut()` and `document_node()`. No library modifications are needed. The solution is to:

1. Create a shell document with `add_string()`
2. Build the AST directly into the document's XOT arena using `xot_mut()`
3. Query using the original `DocumentHandle`

This eliminates the serialize/parse roundtrip entirely.
