---
title: Semantic Tree Transformations
priority: 1
---

Index of per-language transformation decisions. Each language's
transform pass turns tree-sitter's raw syntax tree into tractor's
semantic tree by renaming, flattening, wrapping, marking, and
restructuring nodes. This document is a map; the per-language files
under `transformations/` carry the detail.

## How to read a per-language file

Each file follows the same shape:

1. **Summary** — one paragraph on the language's overall shape
   (any language-specific quirks worth knowing up front).
2. **Element names** — table of tree-sitter kinds → semantic element
   names, with rationale referencing the relevant principle/goal
   from [design.md](design.md).
3. **Structural transforms** — flattens, marker insertions,
   contextual rewrites that aren't pure renames.
4. **Language-specific decisions** — choices that depart from the
   common pattern, with explicit rationale.
5. **Open questions / flagged items** — places where the naming is
   still unsettled and awaiting user input.

## Languages

- [Java](transformations/java.md)
- [TypeScript / JavaScript](transformations/typescript.md)
- [Python](transformations/python.md)
- [C#](transformations/csharp.md)
- [Go](transformations/go.md)
- [Rust](transformations/rust.md)
- [Ruby](transformations/ruby.md)

## Cross-cutting conventions

A handful of conventions apply across every programming-language
transform. They're documented here to avoid repeating them in every
per-language file.

### Field wrapping → semantic element

The builder records tree-sitter's field name as a `field="X"`
attribute on the child element. The per-language `FIELD_WRAPPINGS`
table (in `languages/mod.rs`) maps a tree-sitter field name to a
semantic wrapper element name. The common set:

| Field | Wrapper |
|---|---|
| `name` | `<name>` |
| `value` | `<value>` |
| `left` | `<left>` |
| `right` | `<right>` |
| `body` | `<body>` |
| `condition` | `<condition>` |
| `consequence` | `<consequence>` |
| `alternative` | `<alternative>` |

Per-language additions:

| Language | Field | Wrapper | Rationale |
|---|---|---|---|
| TS/JS | `return_type` | `<returns>` | Canonicalise to match C# (Principle #5). |
| Rust | `return_type` | `<returns>` | Same. |
| Go | `result` | `<returns>` | Same concept, Go-specific field name. |
| C# | `returns` | `<returns>` | Already canonical. |
| TS/JS | `function` | `<callee>` | Distinguish call target from function declaration (avoid `<function>` collision). |
| TS/JS | `object` / `property` | `<object>` / `<property>` | Member expression roles. |

### Flat lists (Principle #12)

Purely-grouping wrappers get dropped, and their children become
siblings of the enclosing element, carrying `field="<plural>"`.
Covered uniformly across languages:

- `parameter_list` / `formal_parameters` / `parameters` → children become siblings with `field="parameters"`.
- `argument_list` / `arguments` → children become siblings with `field="arguments"`.
- `type_arguments` / `type_argument_list` → children become siblings with `field="arguments"` (inside a generic `<type>`).
- `type_parameter_list` / `type_parameters` → children become siblings with `field="generics"`.
- `attribute_list` (C#) → `field="attributes"`.
- `accessor_list` (C#) → `field="accessors"`.

### Identifier handling (Principle #14)

All languages now trust tree-sitter's token distinction:

- `identifier`, `property_identifier`, `shorthand_field_identifier`,
  `field_identifier` → `<name>` (value namespace).
- `type_identifier`, `primitive_type`, `predefined_type`,
  `integral_type`, `floating_point_type`, `boolean_type`,
  `void_type` → `<type>` (type namespace).

C# is the historical outlier; its classifier has been simplified
to match.

### Interface defaults (spec-level)

Members declared inside an interface default to `<public/>` rather
than the enclosing class/struct default. C# and Java both enforce
this (C# spec §18.4, Java spec §9.4). See each language's decision
file for mechanics.

### Generic type references

Pattern established by C#, now applied across TS/JS, Java, Rust,
Python:

```xml
<type>
  <generic/>           <!-- marker: this type is generic -->
  Name                 <!-- the generic's name, as text -->
  <type field="arguments">Arg1</type>
  <type field="arguments">Arg2</type>
</type>
```

### Return types

`<returns>` wrapper containing one `<type>` (single-return
languages) or a sequence of `<type>` siblings (Go multi-return).

## Relationship to `transform-rules/`

The older `transform-rules/` folder documents generic, pattern-level
transformations (lift-modifiers, flatten-declaration-lists, etc.)
that apply across languages. `transformations/` documents the
per-language *decisions* — which specific tree-sitter kinds are
affected, which names are chosen, and why.
