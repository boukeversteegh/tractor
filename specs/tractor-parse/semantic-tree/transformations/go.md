---
title: Go Transformations
---

Per-node decisions for the Go transform
(`tractor/src/languages/go.rs`).

## Summary

Go has several grammar patterns that need special handling:

1. **`type_declaration` wrapper** — Go's `type X …` top-level
   declaration is wrapped by tree-sitter. We move the literal
   `type` keyword into the inner `type_spec` before flattening,
   so the keyword stays attached rather than floating as orphan
   text (see Go #10 design history).
2. **Overloaded `parameter_list`** — Go uses the same tree-sitter
   node for both formal parameters and multi-value return specs.
   Context (parent element) disambiguates.
3. **Overloaded `field_identifier`** — Go uses this kind for
   struct fields, method receivers, and method names on interfaces.
   Currently renamed to `<name>` to match the value-namespace
   convention.
4. **Exported/unexported markers** — based on the first character
   of the declared name (Go spec's visibility rule), the transform
   adds `<exported/>` or `<unexported/>`.

## Element names

| tree-sitter kind | semantic name | rationale |
|---|---|---|
| `source_file` | `<file>` | Short; matches Go's spec term. |
| `package_clause` | `<package>` | Language keyword. |
| `function_declaration` | `<function>` | Language keyword. |
| `method_declaration` | `<method>` | Developer mental model. |
| `method_elem` | `<method>` | Interface method; unified with declaration-level method. |
| `type_declaration` | flattened (after keyword move) | See structural transforms. |
| `type_spec` | `<type>` | Go's own vocabulary — `type X Y` declares a type. |
| `struct_type` | `<struct>` | Language keyword. |
| `interface_type` | `<interface>` | Language keyword. |
| `const_declaration` | `<const>` | Language keyword. |
| `var_declaration` | `<var>` | Language keyword. |
| `short_var_declaration` | `<variable>` with `<short/>` marker | Distinguishes `x := 42` from `var x = 42`. |
| `parameter_declaration` | `<param>` | Short; matches other languages. |
| `pointer_type` | `<pointer>` | Language concept. |
| `slice_type` | `<slice>` | Language concept. |
| `map_type` | `<map>` | Language keyword. |
| `channel_type` | `<chan>` | Language keyword. |
| `return_statement` | `<return>` | Language keyword. |
| `if_statement` | `<if>` | Language keyword. |
| `else_clause` | `<else>` | Language keyword. |
| `for_statement` | `<for>` | Language keyword. |
| `range_clause` | `<range>` | Language keyword. |
| `switch_statement` | `<switch>` | Language keyword. |
| `case_clause` | `<case>` | Language keyword. |
| `default_case` | `<default>` | Language keyword. |
| `defer_statement` | `<defer>` | Language keyword. |
| `go_statement` | `<go>` | Language keyword. |
| `select_statement` | `<select>` | Language keyword. |
| `call_expression` | `<call>` | Matches other languages. |
| `selector_expression` | `<member>` | Matches C#/Java/TS. |
| `index_expression` | `<index>` | Language concept. |
| `composite_literal` | `<literal>` | Language-neutral term. |
| `binary_expression` | `<binary>` | Consistent. |
| `unary_expression` | `<unary>` | Consistent. |
| `interpreted_string_literal` | `<string>` | Language concept. |
| `raw_string_literal` | `<string>` with `<raw/>` marker | Exhaustive marker (Principle #9 partial — raw is non-default; plain strings stay bare). |
| `int_literal` | `<int>` | Language keyword. |
| `float_literal` | `<float>` | Language concept. |
| `true`, `false` | `<true>`, `<false>` | Language keywords. |
| `nil` | `<nil>` | Language keyword. |
| `field_declaration` | `<field>` | Matches other languages. |
| `field_identifier` | `<name>` | Namespace vocabulary (Principle #14). Previously `<field>` which created a collision with `<field>` declaration elements. |
| `package_identifier` | `<name>` | Namespace vocabulary. |
| `import_declaration` | `<import>` | Language keyword. |

## Structural transforms

### Flatten (Principle #12)

- `expression_statement` → Skip.
- `block` — structural.
- `field_declaration_list` — the `{ field; field; }` inside a
  struct; drop so fields become direct children.
- `expression_list` — comma-separated expression group (e.g.
  `return x, y`); drop so individual expressions are siblings.
- `import_spec` — wrapper inside an `import ( … )` block; drop so
  each import path is a direct child of `<import>`.
- `interpreted_string_literal_content` — the content-inside-quotes
  node; flatten to inline text into the enclosing `<string>`.

### `type_declaration` + keyword preservation

Tree-sitter's shape:

```
type_declaration
  "type"                   (literal keyword text)
  type_spec                (or multiple, in `type ( … )` block form)
    name: ...
    type: struct_type | interface_type | ...
```

The transform calls `move_type_keyword_into_spec(decl)` to
relocate the literal `"type"` text into the inner `type_spec`,
then flattens the outer wrapper. Result: the keyword stays
attached to the `<type>` element (useful for the renderer / text
view) rather than floating as an orphan sibling at file level.

### Flat lists with context awareness

- `parameter_list` — dual use:
  - If parent is `<returns>` (via the `result`→`returns` field
    canonicalisation): call `collapse_return_param_list`, which
    rewrites each `parameter_declaration` to just its inner
    type. Result: `<returns><type>int</type><type>error</type></returns>`
    reads as a sequence of types, not a sequence of params.
  - Otherwise (formal parameters): distribute `field="parameters"`.
  - In both cases, flatten the wrapper.
- `argument_list` → children get `field="arguments"`.

### Return type canonicalisation

Go tree-sitter uses `field="result"` for return types (single or
multi). The builder's `canonical_field_name` maps `result` →
`returns`, producing a `<returns>` wrapper consistent with every
other language.

### Export markers

`function_declaration`, `method_declaration`, and `type_spec`
get `<exported/>` or `<unexported/>` based on the first
character of the declared name (Go spec — capital-letter prefix
means exported from the package). Not a syntactic modifier in
the source, so these markers don't have source locations.

### Short variable declarations

```xml
<variable><short/>...</variable>         <!-- from `x := 42` -->
<variable>...</variable>                 <!-- from `var x = 42` -->
```

The `<short/>` marker is an exhaustive indicator of the short form
(Principle #9); the standard `var` form is unmarked because
there are multiple declaration forms (`var`, `const`) that share
the `<variable>` shape.

### Raw string marker

`r"..."` (raw strings) — the transform renames `raw_string_literal`
to `<string>` and prepends a `<raw/>` marker. Mirrors the pattern
used in Rust.

## Language-specific decisions

### `<method>` for both declaration and interface member

Tree-sitter distinguishes `method_declaration` (a concrete
implementation) from `method_elem` (an interface method spec).
Both are "a method" in the developer's mental model (Goal #5) —
the difference (has-body vs no-body) is visible from the
`<body>` child's presence. Unifying to `<method>` simplifies
queries.

### `<field_identifier>` → `<name>` (not `<field>`)

A field_identifier in Go can appear in several places:
- As the name of a struct field declaration.
- As the property being accessed in a selector expression (`obj.Field`).
- As the name of an interface method.

All of these are identifiers in the value namespace (Principle
#14), so they all become `<name>`. An earlier draft renamed
field_identifier to `<field>`, which collided with the `<field>`
declaration element — that's why the rename to `<name>` was made.

### `interpreted_string_literal_content` flatten

The content-inside-quotes node adds no useful nesting — a
`<string>` should just contain its text. Flattening inlines it.
Applied uniformly.

### Keeping `<type>` for Go's type definitions

Go's `type X int` creates a new *defined type* (per the Go spec;
distinct from the alias form `type X = int`). Because the thing
being declared *is literally a type* in Go's vocabulary, using
`<type>` as the declaration element name matches the developer's
speech (`type` is the actual keyword). Principle #14's rule —
"type declarations have their own element name" — is satisfied
because `<type>` *is* a specific element; it just happens to also
be Go's spec term. Disambiguated from type *references* by having
a `<name>` child.

## Open questions / flagged follow-ups

- **Struct/interface hoist (design-agreed, not implemented).**
  `type Hello struct { … }` should render as `<struct><name>Hello</name>…</struct>`
  — the `type` wrapper is Go-grammar bleed-through, not in the
  developer's mental model. Pending implementation commit.
- **Defined-type vs alias form.** `type MyInt int` vs
  `type Color = int`. User-approved plan: `<type>` for the defined
  form (matches Go's spec term), `<alias>` for the `=` form.
  Pending implementation.
- **`expression_list` asymmetry.** Go flattens; Python doesn't. One
  of the two should change for consistency.
