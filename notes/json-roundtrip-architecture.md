# JSON Rendering Pipeline & Multi-Format Roundtrip Architecture

Future improvements for a more unified rendering pipeline that enables
faithful roundtrips between JSON, YAML, XML, and other formats.

## Current State

The pipeline today: **parse → xot tree → data transform → XmlNode → render**.

- Tree-sitter parses source into an xot tree with `kind` attributes tracking
  original node types.
- Data transforms (json/data.rs, yaml/data.rs) project this into a
  query-friendly XML shape: keys become element names, scalars become text.
- The JSON renderer (render/json.rs) converts XmlNode back to JSON source,
  using `kind` for scalar type and `field` for property detection.

### Key attributes on data-tree elements

| Attribute | Purpose | Set by | Read by |
|-----------|---------|--------|---------|
| `kind` | Value type (string/number/true/false/null) or structural role (pair) | data transform | JSON renderer |
| `field` | Property name (marks element as key-value pair) | data transform | JSON renderer, xml_to_json |
| `key` | Original unsanitized key (when element name was sanitized) | data transform | JSON renderer |
| `start`/`end` | Source location spans | parser, data transform | text output |

### What works

- JSON → data tree → JSON roundtrips scalar types correctly using `kind`.
- Heuristic fallback in render_scalar handles untyped nodes (e.g. hand-written XML).
- Arrays flatten into repeated sibling elements, reconstructed by the renderer.
- Sanitized keys preserved via `key` attribute for faithful key restoration.

### What's awkward

- JSON and YAML data transforms are separate implementations with duplicated
  logic (pair handling, array flattening, scalar extraction).
- The `kind` attribute overloads tree-sitter node type with semantic value type.
  For scalars this works fine, but for object/array properties `kind` stays as
  `"pair"` because `find_ancestor_key_name` depends on it.
- `XmlNode` carries attributes as `Vec<(String, String)>` — no typed access,
  easy to miss attributes, linear scan on every lookup.
- No explicit schema for what attributes a data-tree element should carry.
  Renderers discover attributes ad-hoc.

---

## Proposed Improvements

### 1. Unified Data Model for Format-Neutral Trees

Define a canonical data tree representation that isn't tied to any source format:

```
Property { name, value_type, value }
Sequence { item_name, items }
Scalar { type: String|Number|Bool|Null, text }
```

This would replace the current implicit convention where `field` means property,
repeated elements mean array, and `kind` encodes scalar type. A typed IR makes
the contract between transforms and renderers explicit.

**Benefit:** Any format's data transform produces the same IR; any renderer
consumes it. Adding TOML, CSV, or other formats becomes mechanical.

### 2. Separate `value_type` from `kind`

Currently `kind` serves two roles:
- Tree-sitter node type identity (used by transforms to identify nodes)
- Semantic value type (used by renderers for scalar formatting)

These should be distinct. Options:
- **a)** Keep `kind` as tree-sitter identity. Add `value_type` for renderer use.
  Clean separation, but adds an attribute.
- **b)** Use `kind` for value type (current approach for scalars), but ensure
  all consumers that need the original tree-sitter kind read it before the
  data transform overwrites it. This is what we do now — it works but is fragile.
- **c)** Move to a typed IR (option 1) where this distinction is structural.

### 3. Consolidate JSON/YAML Data Transforms

Both transforms do the same conceptual work:
- Extract key from pair → rename element
- Flatten value wrappers
- Handle arrays (repeat parent key as wrapper)
- Flatten scalars, propagate type info

A shared `DataTransform` trait or generic transform could handle the common
pattern, with language-specific hooks for:
- Key extraction (JSON string keys vs YAML scalar keys)
- String decoding (JSON escape sequences vs YAML quoting styles)
- Block scalar handling (YAML-specific)

### 4. Typed Attribute Access on XmlNode

Replace `Vec<(String, String)>` with a small struct or map:

```rust
struct DataAttrs {
    field: Option<String>,
    key: Option<String>,
    kind: Option<String>,
    start: Option<Span>,
    end: Option<Span>,
}
```

Or at minimum, use a `BTreeMap<String, String>` for O(log n) lookup instead
of linear scan. The current `get_attr` helper hides this but every render
call does a linear search.

### 5. Bidirectional Format Conversion

Current path: `JSON source → data tree → JSON source` (roundtrip).

To support `JSON → YAML` or `YAML → JSON`:
- Parse source format into canonical data tree
- Render data tree to target format

This mostly works today if you pipe JSON data XML into the YAML renderer
(or vice versa), but:
- Scalar type info must survive the conversion (currently `kind`-based, OK)
- Array representation must be consistent across formats (currently it is)
- Key ordering should be preserved (currently is, via child order)
- Comments are lost in data transforms (acceptable for data, but a YAML→YAML
  roundtrip might want to preserve them)

### 6. Schema-Aware Rendering

For formats like JSON Schema, OpenAPI, or typed configs, the renderer could
use schema information to:
- Coerce "42" to number when schema says integer (no need for `kind` heuristic)
- Validate output structure
- Generate default values for missing fields

This is lower priority but would make tractor useful as a format migration tool.

### 7. Streaming / Incremental Rendering

The current pipeline materializes the full XmlNode tree before rendering.
For large files, a streaming approach that renders nodes as they're visited
would reduce memory usage. The xot tree walker already visits nodes in
document order — the renderer could emit output during the walk rather than
building an intermediate XmlNode.

### 8. Object vs Array Property Distinction

Currently, `kind` stays as `"pair"` for object/array-valued properties because
`find_ancestor_key_name` matches on `kind == "pair"`. This means the renderer
can't distinguish `"key": {}` from `"key": []` without inspecting children.

Fix: either `find_ancestor_key_name` should match on `field` attribute presence
(more robust than matching `kind`), or use a `role="property"` attribute
separate from `kind`. Then `kind` could be set to `"object"` or `"array"` on
all properties, giving the renderer full type information.

---

## Priority Order

1. **Separate value_type from kind** (or fix find_ancestor_key_name) — unblocks
   full type info on all properties, not just scalars.
2. **Consolidate JSON/YAML transforms** — reduces duplication, makes adding
   formats easier.
3. **Typed attribute access** — improves correctness and performance.
4. **Bidirectional conversion CLI** — expose `tractor convert --from json --to yaml`.
5. **Unified data model** — longer-term architectural goal.
