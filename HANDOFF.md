# Tractor Semantic Transform - Implementation Handoff

## Project Overview

**Tractor** is a code query tool that parses source code into XML AST and queries it with XPath 3.1. It supports 22+ languages via tree-sitter.

**Repository**: `D:\tmp\code-xpath`

**Key capability**: Parse code → XML → XPath query → results with source locations

Example:
```bash
tractor "//method[@name='DoSomething']" src/**/*.cs
```

## The Problem

The current semantic transformation pipeline is broken:

```
Current (WRONG):
TreeSitter AST → write_semantic_node() → XML String → re-parse → xot nodes → XPath
                 ↑ writes formatted text directly, wasteful round-trip
```

This was supposed to be:

```
Correct:
TreeSitter AST → build_semantic() → xot nodes directly → XPath
                 ↑ single pass, no string serialization
```

## What Exists

### Good - Keep these:

1. **`XotBuilder`** (`tractor-core/src/xot_builder.rs`)
   - Has `build_raw()` that builds xot nodes directly from tree-sitter
   - This is the pattern to follow for semantic mode

2. **`LangTransforms`** (`tractor-core/src/parser/transform.rs`)
   - Language-specific transformation configs
   - Element renaming, skip/flatten rules, modifier extraction
   - `classify_identifier` function pointer per language

3. **Language configs** (`tractor-core/src/parser/languages/*.rs`)
   - TypeScript, C#, Python, Go, Rust, Java
   - Each has element mappings, modifiers, identifier classification

### Bad - Needs rewrite:

1. **`semantic.rs`** (`tractor-core/src/parser/semantic.rs`)
   - Currently writes XML strings with `writeln!`
   - Should be deleted or gutted
   - Replace with `build_semantic()` in XotBuilder

2. **`parse_string()`** in `mod.rs` line 152
   - Calls `semantic::write_semantic_node()` which writes strings
   - Should call xot-based builder instead

## Implementation Plan

### Step 1: Add `build_semantic()` to XotBuilder

In `tractor-core/src/xot_builder.rs`, add:

```rust
pub fn build_semantic(
    &mut self,
    ts_node: TsNode,
    source: &str,
    file_path: &str,
    transforms: &LangTransforms,
) -> Result<XotNode, xot::Error> {
    // Similar to build_raw but applies transforms
}
```

The key transformations to apply while building xot nodes:

1. **Rename elements**: `binary_expression` → `binary` (use `transforms.rename_element()`)
2. **Skip nodes**: `expression_statement` (use `transforms.should_skip()`)
3. **Flatten nodes**: `declaration_list` (use `transforms.should_flatten()`)
4. **Extract operators**: `+` from binary_expression → `op="+"` attribute
5. **Extract modifiers**: `public`, `static` → `<public/>`, `<static/>` elements
6. **Classify identifiers**: Use `transforms.classify_identifier()` → `<name>` or `<type>`
7. **Wrap fields**: `left`, `right`, `name`, `value` → `<left>...</left>` wrapper elements

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

Remove or deprecate:
- `semantic::write_semantic_node()`
- `semantic::write_semantic_node_with_context()`
- All the `write_*` helper functions in semantic.rs

## Key Files to Read

| File | Purpose |
|------|---------|
| `tractor-core/src/xot_builder.rs` | XotBuilder with build_raw() - **follow this pattern** |
| `tractor-core/src/parser/transform.rs` | LangTransforms struct and helper functions |
| `tractor-core/src/parser/languages/mod.rs` | get_transforms() and language config exports |
| `tractor-core/src/parser/languages/csharp.rs` | Example language config with classify_identifier |
| `tractor-core/src/parser/semantic.rs` | Current (broken) implementation - understand what it does, then replace |
| `tractor-core/src/parser/mod.rs` | Entry point parse_string() that needs updating |

## Transform Rules Reference

From `LangTransforms`:

```rust
pub struct LangTransforms {
    pub element_mappings: &'static [(&'static str, &'static str)],  // rename
    pub flatten_kinds: &'static [&'static str],      // skip node, keep children
    pub skip_kinds: &'static [&'static str],         // skip entirely
    pub operator_kinds: &'static [&'static str],     // extract op attribute
    pub keyword_modifier_kinds: &'static [&'static str],  // let/const/var
    pub known_modifiers: &'static [&'static str],    // public/static/async
    pub modifier_wrapper_kinds: &'static [&'static str],  // C# "modifier" wrapper
    pub extract_name_attr_kinds: &'static [&'static str], // namespace full name
    pub classify_identifier: fn(...) -> IdentifierKind,   // name vs type
    pub compute_identifier_context: fn(...) -> bool,      // C# namespace context
}
```

## Expected Output Format

Input C#:
```csharp
public class Foo {
    public void DoSomething() { }
}
```

Expected semantic XML:
```xml
<unit>
  <class start="1:1" end="3:2">
    <public/>
    <name>Foo</name>
    <method start="2:5" end="2:30">
      <public/>
      <returns><type>void</type></returns>
      <name>DoSomething</name>
      <params>()</params>
      <block>{ }</block>
    </method>
  </class>
</unit>
```

## Verification

1. `cargo test` - all 45 tests should pass
2. `echo 'let x = 1 + 2;' | cargo run -- --lang typescript` - check output structure
3. `cargo run -- "//method" some_file.cs` - XPath queries work
4. No XML string → re-parse in the pipeline

## Notes

- The xot library uses `NameId` for element/attribute names (cached for performance)
- Tree-sitter nodes have `.kind()` for node type, `.is_named()` for named vs anonymous
- Anonymous nodes are operators/punctuation (`+`, `;`, `{`) - extract as text content or attributes
- Field names come from tree-sitter grammar (`.field_name()` on cursor)
