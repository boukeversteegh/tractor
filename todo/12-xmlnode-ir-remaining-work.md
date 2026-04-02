# XmlNode IR: remaining work

## Background

The pipeline now uses a native `XmlNode` IR instead of serialized XML strings for
matched XML fragments. The IR is built from the xot tree in `engine.rs` and consumed
directly by all output renderers. No XML string roundtrip remains in the hot path.

See `docs/xquery-xpath-exploration.md` for full architecture details.

## Completed

1. **XmlNode IR introduced** — `XmlNode` enum in `match_result.rs` with Element, Text,
   Comment, ProcessingInstruction variants. Built from xot in `xot_node_to_xml_node()`.

2. **All consumers migrated to native IR** — every renderer operates on `XmlNode` directly:
   - `render_xml_node()` for text/XML pretty-printing
   - `xml_node_to_json()` for JSON/YAML tree conversion
   - `extract_syntax_spans_from_xml_node()` for syntax highlighting
   - `SchemaCollector::collect_from_xml_node()` for schema collection

3. **Bridge code removed** — `Match.xml_fragment_cache`, `with_xml_fragment()`,
   `xml_fragment_string()`, `has_xml()` all deleted. `ReportMatch.tree` is
   `Option<XmlNode>` throughout.

4. **Stage-appropriate code placement** — `xml_node_to_string()` (compact serializer)
   moved to the output module. `xml_fragment_to_json()` (string-based converter)
   moved to test module.

5. **Map/array output** — XPath 3.1 map and array constructors produce JSON strings
   via xee's `serialize()` method, stored in `Match.value`.

## Remaining work

### 1. Remove old string-based functions that are only used by WASM

The following functions still exist because they're called from `wasm.rs`:

- `render_xml_string()` in `xml_renderer.rs` — parses an XML string and re-renders it
- `extract_syntax_spans()` / `extract_syntax_spans_with_lang()` in `syntax_highlight.rs`
- `SchemaCollector::collect()` in `schema.rs` — string-based schema collection

**What to do:** When the WASM module is updated to receive `XmlNode` instead of XML
strings, these functions can be deleted. This requires changing the WASM API boundary
to pass structured data instead of strings (possibly via JSON serialization of `XmlNode`
since wasm-bindgen can't directly pass Rust enums).

**Priority:** Low. These functions work fine, they just represent dead code for the
native (non-WASM) path.

### 2. Native IR types for maps, arrays, and atomics

Currently, XPath map/array results are serialized to JSON strings in `engine.rs`
(`function_to_json_string`) and stored as opaque strings in `Match.value`. Ideally
they would be native IR types so output formats can render them appropriately (e.g.,
JSON output could nest maps as real JSON objects instead of double-escaped strings).

**Blocker:** xee's `Map` and `Array` types have all iteration methods (`entries()`,
`keys()`, `iter()`, `index()`) marked as `pub(crate)`. There is no public API to
inspect map entries or array elements. The only public extraction method is
`serialize()` with serialization parameters.

**What to do:** Either:
- Wait for / request upstream xee changes to expose iteration on Map/Array
- Parse the JSON string back into structured data (defeats the purpose but would
  enable proper nesting in JSON/YAML output)
- Accept the current JSON-string-in-value approach as good enough

**XmlNode variants that would be added** (if iteration becomes available):
```rust
pub enum XmlNode {
    Element { ... },
    Text(String),
    Comment(String),
    ProcessingInstruction { ... },
    // New:
    Map(Vec<(String, XmlNode)>),     // key-value pairs
    Array(Vec<XmlNode>),             // ordered items
    Atomic(String),                   // typed atomic value
}
```

**Priority:** Blocked on upstream. The current approach works — maps render as JSON
strings in all output formats.

### 3. Fix duplicate results for aggregate expressions

Moved to dedicated todo: `todo/22-aggregate-expression-duplicate-eval.md`

### 4. Improve atomic string rendering

String atomics render as `"hello"` (with XPath quotes) via `xpath_representation()`.
Integers and booleans render cleanly as `3`, `true()`.

**What to do:** Strip outer quotes from string atomics, or use a different
serialization method for the `value` field.

**Priority:** Low.

### 5. Consider shorthand for map output

The `serialize(map{...}, map{"method":"json"})` pattern is verbose. Since maps now
auto-serialize to JSON via `function_to_json_string`, this is already improved — users
can just write `map { "k": "v" }` and get JSON output. But the auto-serialization
only applies to the top-level result; nested serialize calls are still verbose.

**What to do:** Evaluate whether additional shortcuts are needed, or if the current
auto-serialization covers the common cases.

**Priority:** Low. The auto-serialization handles the main use case.
