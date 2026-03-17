# Semantic transform: eliminate XML string roundtrip

## Background

The semantic transform pipeline currently writes XML strings via `writeln!` in
`semantic.rs`, then re-parses them into xot nodes. This should build xot nodes
directly, following the pattern already established by `XotBuilder::build_raw()`.

## Current state (broken roundtrip)

```
TreeSitter AST → write_semantic_node() → XML String → re-parse → xot nodes → XPath
                 ↑ writes formatted text directly, wasteful round-trip
```

## Target state

```
TreeSitter AST → build_semantic() → xot nodes directly → XPath
                 ↑ single pass, no string serialization
```

## Implementation plan

### Step 1: Add `build_semantic()` to XotBuilder

In `tractor-core/src/xot_builder.rs`, add a method similar to `build_raw()` that
applies `LangTransforms` while building xot nodes:

1. Rename elements: `binary_expression` → `binary` (via `transforms.rename_element()`)
2. Skip nodes: `expression_statement` (via `transforms.should_skip()`)
3. Flatten nodes: `declaration_list` (via `transforms.should_flatten()`)
4. Extract operators: `+` from binary_expression → `op="+"` attribute
5. Extract modifiers: `public`, `static` → `<public/>`, `<static/>` elements
6. Classify identifiers: via `transforms.classify_identifier()` → `<name>` or `<type>`
7. Wrap fields: `left`, `right`, `name`, `value` → wrapper elements

### Step 2: Update `parse_string()`

In `tractor-core/src/parser/mod.rs`:
```rust
if raw_mode {
    builder.build_raw(...)
} else {
    let transforms = languages::get_transforms(lang);
    builder.build_semantic(..., transforms)
}
```

### Step 3: Delete string-writing code

Remove:
- `semantic::write_semantic_node()`
- `semantic::write_semantic_node_with_context()`
- All `write_*` helper functions in `semantic.rs`

## Key files

| File | Purpose |
|------|---------|
| `tractor-core/src/xot_builder.rs` | XotBuilder with `build_raw()` — follow this pattern |
| `tractor-core/src/parser/transform.rs` | LangTransforms struct and helper functions |
| `tractor-core/src/parser/languages/mod.rs` | `get_transforms()` and language config exports |
| `tractor-core/src/parser/languages/csharp.rs` | Example language config with `classify_identifier` |
| `tractor-core/src/parser/semantic.rs` | Current implementation — understand then replace |
| `tractor-core/src/parser/mod.rs` | Entry point `parse_string()` that needs updating |

## Priority

Medium-high. Performance improvement and code simplification. Same pattern as the
XmlNode IR migration (eliminate string roundtrip), but for the input/parse side
instead of the output side.
