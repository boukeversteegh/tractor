# XQuery / XPath 3.1 Exploration

## Summary

**xee does NOT support XQuery** - it only supports XPath 3.1 (and XSLT 3.0).
"Expand XPath to XQuery" is listed as a future "Challenging" idea in their `ideas.md`.

However, **XPath 3.1 already provides most XQuery-like constructs** needed for
map transforms. The `-x` flag can already handle complex transformations.

## What Works (XPath 3.1)

### Core Constructs

| Construct | XPath 3.1 Syntax | Status |
|-----------|-------------------|--------|
| for/return | `for $f in //function return string($f/name)` | Works |
| let/return | `let $fns := //function return count($fns)` | Works |
| if/then/else | `if (expr) then a else b` | Works |
| some/every | `some $f in //function satisfies ...` | Works |
| string concat | `"fn:" \|\| string($f/name)` | Works |
| arrow operator | `//function/name => count()` | Works |
| nested for | `for $c in //class, $m in $c//function return ...` | Works |

### Map Constructs

| Pattern | Syntax | Status |
|---------|--------|--------|
| Map literal | `map { "key": "value" }` | Works (JSON output) |
| for + map | `for $f in //X return map { "k": string($f/name) }` | Works |
| map:merge | `map:merge(for $f in //X return map { ... })` | Works |
| map:keys | `map:keys($m)` | Works |
| Array literal | `array { ... }` | Works (JSON output) |

### What Does NOT Work (XQuery-only)

| Construct | Syntax | Workaround |
|-----------|--------|------------|
| where | `for $f in X where cond return ...` | Use predicate: `for $f in X[cond] return ...` |
| order by | `for $f in X order by ... return ...` | Use `sort()`: `sort(for $f in X return ...)` |
| element constructors | `<result>{$f/name}</result>` | Use `map{}` |
| user-defined functions | `declare function f() { ... }` | Use inline functions: `function($x) { ... }` |
| modules | `import module namespace ...` | Not available |

## Practical Examples

### Map transform (the user's `/ map` use case)
```bash
# Extract function names as JSON objects
tractor file.py -x 'for $f in //function return map {
  "name": string($f/name),
  "params": string($f/params)
}' -v value

# Output:
# {"name":"hello","params":"(name)"}
# {"name":"add","params":"(a, b)"}
```

### Filtering (where equivalent)
```bash
# XPath predicate instead of XQuery 'where'
tractor file.py -x 'for $f in //function[count(.//params/type) > 1]
  return string($f/name)' -v value
```

### Nested class.method projection
```bash
tractor file.py -x 'for $c in //class, $m in $c//function
  return map { "class": string($c/name), "method": string($m/name) }' -v value
```

## Known Issues

### 1. Duplicate results for aggregate/constant expressions
Expressions that produce a single aggregate result (like `serialize(array{...})`)
get duplicated ~96x per file. The `for/return` pattern does NOT have this issue.

**Affected**: `sort()`, `for-each()`, `serialize(array{...})`, `true()`, `map{...}`
**Not affected**: `for/return`, `count()`, `let/return`, `(true())`

Wrapping in parens sometimes helps: `(true())` returns 1 result vs `true()` returns 96.

### 2. Atomic string values show XPath quotes
String atomics show as `"hello"` (with quotes) in `-v value` mode because we use
`xpath_representation()`. Integer/boolean atomics render correctly as `3`, `true()`.

## Chosen Approach: Native IR with XPath 3.1

### Architecture

XPath 3.1 is sufficient — no XQuery needed. The pipeline uses a native intermediate
representation (IR) instead of XML string serialization:

```
Source files → TreeSitter → xot tree → XPath 3.1 query
                                            ↓
                                     xee result items
                                            ↓
                              ┌─────────────┼──────────────┐
                              │             │              │
                          Node items   Atomic items   Function items
                              │             │         (map/array)
                              ↓             ↓              ↓
                          XmlNode IR    value string    JSON string
                         (native tree)                 (via xee serialize)
                              │             │              │
                              └─────────────┼──────────────┘
                                            ↓
                                      ReportMatch
                                   (tree, value, etc.)
                                            ↓
                                   Output format layer
                              (text, json, yaml, xml, gcc)
```

### XmlNode IR

XML node results are represented as a native `XmlNode` enum instead of serialized
XML strings. This eliminates the serialize → parse roundtrip that existed before:

```rust
// tractor-core/src/xpath/match_result.rs
pub enum XmlNode {
    Element { name: String, attributes: Vec<(String, String)>, children: Vec<XmlNode> },
    Text(String),
    Comment(String),
    ProcessingInstruction { target: String, data: Option<String> },
}
```

The IR is built once from the xot tree in `engine.rs` (`xot_node_to_xml_node`),
then consumed directly by all downstream renderers:
- **Text/XML output**: `render_xml_node()` — pretty-prints with colors/depth-limiting
- **JSON/YAML output**: `xml_node_to_json()` — walks tree to build JSON with field lifting
- **Syntax highlighting**: `extract_syntax_spans_from_xml_node()` — extracts color spans
- **Schema collection**: `SchemaCollector::collect_from_xml_node()` — collects paths
- **Compact serialization**: `xml_node_to_string()` — for snapshot/report serialization

### Map/Array handling

XPath 3.1 map and array constructors produce `Function` items in xee. These are
serialized to JSON strings using xee's built-in serializer (`serialize(params, xot)`
with `method: "json"`), and stored in `Match.value` as opaque strings.

This is a pragmatic choice: xee's `Map` and `Array` types have their iteration
methods marked `pub(crate)`, making them inaccessible from external code. The only
public way to extract data is via `serialize()`. If xee exposes iteration in the
future, these could be converted to native IR types instead.

### Key files

| File | Role |
|------|------|
| `tractor-core/src/xpath/match_result.rs` | `XmlNode` enum and `Match` struct |
| `tractor-core/src/xpath/engine.rs` | `xot_node_to_xml_node()`, `function_to_json_string()` |
| `tractor-core/src/output/xml_renderer.rs` | `render_xml_node()`, `xml_node_to_string()` |
| `tractor-core/src/output/xml_to_json.rs` | `xml_node_to_json()` — tree-to-JSON with field lifting |
| `tractor-core/src/output/syntax_highlight.rs` | `extract_syntax_spans_from_xml_node()` |
| `tractor-core/src/output/schema.rs` | `SchemaCollector::collect_from_xml_node()` |
| `tractor-core/src/report.rs` | `ReportMatch` with `tree: Option<XmlNode>` |

## Recommendation

XPath 3.1 is **sufficient for map transforms**. The `for/return + map` pattern
covers the `/ map` use case without needing XQuery.

Key remaining work:
- Fix duplicate results for aggregate expressions (investigate xee SequenceQuery behavior)
- Consider a shorthand syntax for the verbose `serialize(..., map{"method":"json"})` pattern
- If xee exposes map/array iteration in the future, add native IR types for them
