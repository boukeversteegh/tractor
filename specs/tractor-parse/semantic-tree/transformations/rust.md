---
title: Rust Transformations
---

Per-node decisions for the Rust transform
(`tractor/src/languages/rust_lang.rs`).

## Summary

Rust's transform exposes the language's rich declaration vocabulary
(struct, enum, trait, impl, fn, mod, const, static, use) and
handles a few Rust-specific constructs:

1. **Visibility modifier** — `pub` and its variants (`pub(crate)`,
   `pub(super)`, `pub(in path)`) lifted to a `<pub/>` marker with a
   nested detail child.
2. **Let modifiers** — `mut`, `async`, `unsafe`, `const` keywords on
   `let` declarations extracted as empty markers.
3. **Generic type references** — shared C# pattern via
   `rewrite_generic_type`.
4. **Raw string literal** — `<string>` with `<raw/>` marker
   (mirrors Go).

## Element names

| tree-sitter kind | semantic name | rationale |
|---|---|---|
| `source_file` | `<file>` | Short. |
| `function_item` | `<function>` | Language keyword. |
| `impl_item` | `<impl>` | Language keyword. |
| `struct_item` | `<struct>` | Language keyword. |
| `enum_item` | `<enum>` | Language keyword. |
| `trait_item` | `<trait>` | Language keyword. |
| `mod_item` | `<mod>` | Language keyword. |
| `use_declaration` | `<use>` | Language keyword. |
| `const_item` | `<const>` | Language keyword. |
| `static_item` | `<static>` | Language keyword. |
| `type_item` | `<alias>` | Rust's `type X = Y` is an alias declaration; the word "type" alone is too generic. |
| `parameter` | `<param>` | Short. |
| `self_parameter` | `<self>` | Language keyword. |
| `reference_type` | `<ref>` | Rust's `&T` / `&mut T`. *See Open questions.* |
| `generic_type` | rewritten — see below | Structural. |
| `scoped_identifier`, `scoped_type_identifier` | `<path>` | Rust's `std::collections::HashMap` — short, consistent. |
| `return_expression` | `<return>` | Language keyword. |
| `if_expression` | `<if>` | Language keyword. |
| `else_clause` | `<else>` | Language keyword. |
| `for_expression`, `while_expression` | `<for>`, `<while>` | Language keywords. |
| `loop_expression` | `<loop>` | Language keyword. |
| `match_expression` | `<match>` | Language keyword. |
| `match_arm` | `<arm>` | Short, matches the Rust spec term "match arm". |
| `field_declaration` | `<field>` | Matches other languages. |
| `field_initializer` | `<field>` | Same element name for declaration and initialisation — context (parent `<struct>` vs `<struct_expression>`) disambiguates. |
| `trait_bounds` | `<bounds>` | Plural; a bound list. |
| `call_expression` | `<call>` | Consistent. |
| `method_call_expression` | `<call>` | Unified with regular call — the `<member>` receiver in the callee already distinguishes. |
| `field_expression` | `<field>` | Matches declaration. |
| `index_expression` | `<index>` | Language concept. |
| `binary_expression` | `<binary>` | Consistent; operator extracted. |
| `unary_expression` | `<unary>` | Consistent. |
| `closure_expression` | `<closure>` | Language term. |
| `await_expression` | `<await>` | Language keyword. |
| `try_expression` | `<try>` | `?` operator. |
| `macro_invocation` | `<macro>` | Language concept. |
| `string_literal` | `<string>` | Language concept. |
| `raw_string_literal` | `<string>` with `<raw/>` marker | See structural transforms. |
| `integer_literal` | `<int>` | Language keyword. |
| `float_literal` | `<float>` | Language concept. |
| `boolean_literal` | `<bool>` | Language keyword. |
| `identifier`, `field_identifier`, `shorthand_field_identifier` | `<name>` | Namespace vocabulary (Principle #14). |
| `type_identifier`, `primitive_type` | `<type>` | Namespace vocabulary. |

## Structural transforms

### Flatten (Principle #12)

- `expression_statement` → Skip.
- `block`, `declaration_list` — structural.
- `field_declaration_list`, `field_initializer_list` — purely
  grouping wrappers inside a struct or struct expression.

### Flat lists with field distribution (Principle #12)

- `parameters` (tree-sitter kind; gated by `has_kind` so we don't
  match semantic `<parameters>` wrappers) → `field="parameters"`.
- `arguments` → `field="arguments"`.
- `type_arguments` → `field="arguments"` (inside generic `<type>`).

### Generic type references

Shared helper; Rust's name-kinds are `type_identifier` and
`scoped_type_identifier`:

```xml
<type>
  <generic/>
  Vec
  <type field="arguments">i32</type>
</type>
```

### Visibility modifier

Rust's `pub`, `pub(crate)`, `pub(super)`, `pub(in path)` all go
through the `visibility_modifier` handler. The marker is always
`<pub/>`; the restriction form is a nested detail child:

```xml
<pub><crate/></pub>               <!-- pub(crate) -->
<pub><super/></pub>               <!-- pub(super) -->
<pub><in>some::path</in></pub>    <!-- pub(in some::path) -->
<pub/>                            <!-- bare pub -->
```

If a top-level item has no visibility modifier, the transform
prepends `<private/>` so the marker is exhaustive (Principle #9
applied to Rust visibility).

### `let` declaration markers

`let_declaration` extracts `mut`, `async`, `unsafe`, `const`
keywords into empty markers and renames to `<let>`:

```xml
<let><mut/><name>x</name><value>5</value></let>
```

### Raw string marker

```xml
<string><raw/>...</string>       <!-- from r"..." -->
```

## Language-specific decisions

### `<type_item>` → `<alias>`

`type X = Y` in Rust is specifically a type alias (unlike Go's
`type X Y` which creates a new defined type). "Alias" matches the
Rust spec and is short. Using `<type>` would be confusing because
`<type>` is the type-reference element (Principle #14).

### `<impl>` kept as-is

Rust's `impl Trait for Type { … }` is a specific construct with
no direct analog in other languages. Keeping `impl` preserves the
language keyword; renaming would lose the Rust-specific meaning.

### `<method_call_expression>` → `<call>` (unified)

Method calls (`receiver.method(args)`) and regular calls
(`function(args)`) both produce `<call>` elements. The presence of
a `<member>` callee identifies a method call; a plain `<name>`
callee identifies a function call. Unifying simplifies
cross-language queries (Principle #5; matches C#/Java/Go/Python).

### `<field_initializer>` → `<field>` (not new element)

In `Point { x: 1, y: 2 }`, each `x: 1` is a `field_initializer`.
Semantically it's "the x field set to 1" — same concept as a
struct-declaration's field with a value. Unifying under `<field>`
works because the parent (`<struct>` for declarations vs
`<struct_expression>` — pending rename) disambiguates.

### `<trait_bounds>` → `<bounds>`

The constraint `T: Clone + Send` is a bound list. Plural `bounds`
matches Java's `<bound>` (single) and reads naturally.

