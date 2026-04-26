---
title: Ruby Transformations
---

Per-node decisions for the Ruby transform
(`tractor/src/languages/ruby.rs`).

## Summary

Ruby's transform is the lightest of the programming-language
transforms. Ruby's tree-sitter grammar is already close to what
developers think in — most nodes are keywords (`class`, `module`,
`method`, `if`, `unless`, `case`, etc.) and need no renaming.

The transform:

1. Flattens `body_statement` (a purely-grouping wrapper around a
   method or control-flow body).
2. Flattens `method_parameters` with `field="parameters"`
   distribution (Principle #12).
3. Flattens `argument_list` with `field="arguments"`.
4. Inlines the `<name>` field wrapper when the declaration is a
   method/class/module (so `def foo` renders as
   `<method><name>foo</name>…</method>` rather than
   `<method><name><identifier>foo</identifier></name>…</method>`).

## Element names

| tree-sitter kind | semantic name | rationale |
|---|---|---|
| `program` | `<program>` | Convention. |
| `method` | `<method>` | Language keyword (Ruby's `def`). |
| `class` | `<class>` | Language keyword. |
| `module` | `<module>` | Language keyword. |
| `if`, `unless` | `<if>`, `<unless>` | Language keywords. |
| `elsif` | `<else_if>` | Cross-cutting conditional shape — see below. |
| `else` | `<else>` | Language keyword. |
| `case` | `<case>` | Language keyword. |
| `while`, `until`, `for` | `<while>`, `<until>`, `<for>` | Language keywords. |
| `begin` | `<begin>` | Language keyword (exception handling). |
| `rescue` | `<rescue>` | Language keyword. |
| `ensure` | `<ensure>` | Language keyword. |
| `call`, `method_call` | `<call>` | Unified — both are calls. |
| `assignment` | `<assign>` | Consistent. |
| `binary` | `<binary>` | Consistent. |
| `string` | `<string>` | Language concept. |
| `integer` | `<int>` | Short, matches Python. |
| `float` | `<float>` | Language concept. |
| `symbol` | `<symbol>` | Ruby-specific. |
| `array` | `<array>` | Language concept. |
| `hash` | `<hash>` | Language keyword. |

## Structural transforms

### Flatten (Principle #12)

- `body_statement` — the wrapping body of methods and control
  structures; drop so statements are direct children of the
  method/class/module.

### Flat lists with field distribution

- `method_parameters` → children get `field="parameters"`.
- `argument_list` → children get `field="arguments"`.

### Name inlining

When a `<name>` wrapper sits inside a `method`/`class`/`module`
declaration and contains a single `<identifier>` child, the
transform inlines the identifier's text directly:

```xml
<method><name>foo</name>…</method>   <!-- not <method><name><identifier>foo</identifier></name>… -->
```

Consistent with how TS/JS/Java/Rust handle the same case.

## Language-specific decisions

### Identifier handling stays passthrough

Unlike the statically-typed languages, Ruby doesn't have a
`type_identifier` vs `identifier` split at the parser level. All
identifiers are `identifier`, currently passing through without
a rename. Whether to add `identifier` → `<name>` for consistency
with Principle #14 is a minor follow-up.

### Conditional shape (`<if>` / `<else_if>` / `<else>`)

Ruby's tree-sitter grammar is an outlier in two ways that together
require a Ruby-specific pair of wrappings and the post-walk
collapse:

1. The grammar already emits a literal `<then>` kind (with
   `field="consequence"`) for the true-branch. Wrapping the
   `consequence` field in another `<then>` would double-nest, so
   `RUBY_FIELD_WRAPPINGS` omits the `consequence` mapping entirely.
2. The `elsif`/`else` chain is *nested* — each `elsif` lives
   inside the previous one rather than being a flat sibling of
   `if`. The `alternative` field is also omitted from
   `RUBY_FIELD_WRAPPINGS` so each clause stays a direct named
   child; the post-walk `collapse_else_if_chain` then walks the
   chain and lifts each `<elsif>` / `<else>` out as a flat child
   of the outer `<if>`. `elsif` is renamed to `<else_if>` in
   `map_element_name` (so the post-walk and the rename produce the
   same final name).

See the cross-cutting "Conditional shape" convention in the index
[`transformations.md`](../transformations.md).

## Comments

Ruby uses the shared `CommentClassifier`
(`tractor/src/languages/comments.rs`) with `["#"]` as the line
prefix. `comment` renames to `<comment>` and gets a `<trailing/>`
or `<leading/>` marker per the cross-cutting rules (see
[`transformations.md`](../transformations.md) — *Comments*).
Adjacent `#` comments merge into a single `<comment>`.
Multi-line `=begin…=end` block comments are rare and stay as
single comments — no special handling.
