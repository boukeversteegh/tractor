---
title: C# Transformations
---

Per-node decisions for the C# transform
(`tractor/src/languages/csharp.rs`).

## Summary

C# has the most mature transform — it was the original template for
the generic-type pattern now applied across other languages. The
transform exposes C#'s rich declaration vocabulary (class, struct,
interface, record, enum, namespace, delegate) and lifts its modifier
and attribute system into queryable markers.

Distinctive features:

1. **Generic type shape** — `<type><generic/>Name<type field="arguments">…</type></type>`
   — the pattern now shared with Java, TS, Rust, Python.
2. **Nullable types** — `Foo?` becomes `<type>Foo<nullable/></type>`.
3. **Attributes and accessors** — C# attribute lists and
   `{ get; set; }` accessor lists both flat-list (Principle #12).
4. **Context-aware defaults** — `<public/>` inside interfaces,
   `<private/>` inside classes/structs/records, `<internal/>` at
   top level.

## Element names (canonical constants)

Defined in `csharp::semantic` as Rust constants so the transform and
renderer share a single source of truth:

| Category | Elements |
|---|---|
| Structural | `<unit>`, `<namespace>`, `<import>`, `<body>` |
| Type decls | `<class>`, `<struct>`, `<interface>`, `<enum>`, `<record>` |
| Members | `<method>`, `<constructor>`, `<property>`, `<field>`, `<comment>`, `<constant>` (for enum members) |
| Shared children | `<name>`, `<type>`, `<get>`, `<set>`, `<init>`, `<add>`, `<remove>`, `<attributes>`, `<attribute>`, `<arguments>`, `<argument>`, `<parameters>`, `<parameter>`, `<variable>`, `<declarator>` |
| Type markers | `<nullable/>`, `<generic/>` |
| Comment markers | `<trailing/>`, `<leading/>` |

## Element names (tree-sitter → semantic)

| tree-sitter kind | semantic name | rationale |
|---|---|---|
| `compilation_unit` | `<unit>` | Short, matches C#'s compiler term. |
| `class_declaration` | `<class>` | Language keyword. |
| `struct_declaration` | `<struct>` | Language keyword. |
| `interface_declaration` | `<interface>` | Language keyword. |
| `enum_declaration` | `<enum>` | Language keyword. |
| `record_declaration` | `<record>` | Language keyword. |
| `method_declaration` | `<method>` | Developer mental model. |
| `constructor_declaration` | `<constructor>` | Full word. |
| `property_declaration` | `<property>` | Language term. |
| `field_declaration` | `<field>` | Language term. |
| `namespace_declaration` | `<namespace>` | Language keyword. |
| `enum_member_declaration` | `<constant>` | "Enum constant" shortened to `constant` per user preference. |
| `parameter_list` | flattened | Flat list. |
| `parameter` | `<parameter>` | Full word (C# uses verbose naming). |
| `argument_list` | flattened | Flat list. |
| `argument` | `<argument>` | Full word. |
| `type_argument_list` | flattened with `field="arguments"` | Inside generic `<type>`. |
| `type_parameter_list` | flattened with `field="generics"` | Renamed during flat-list distribution. |
| `type_parameter` | `<generic>` | Matches generic marker naming. |
| `attribute_list` | flattened with `field="attributes"` | Flat list. |
| `attribute` | `<attribute>` | Full word. |
| `accessor_list` | flattened with `field="accessors"` | Flat list. |
| `accessor_declaration` | `<get>`, `<set>`, `<init>`, `<add>`, or `<remove>` | Specific node name; avoids encoding accessor kind as a wrapper-plus-marker hierarchy. |
| `using_directive` | `<import>` | Developer mental model — "import" is more universal than "using". |
| `generic_name` | rewritten — see below | Structural, not a rename. |
| `nullable_type` | rewritten — see below | Structural. |
| `array_type` | `<array>` | Language concept. |
| `block` | `<block>` | Developer mental model. |
| `return_statement` | `<return>` | Language keyword. |
| `if_statement` | `<if>` | Language keyword. |
| `else_clause` | `<else>` | Language keyword; chain collapsed — see below. |
| `for_statement`, `while_statement` | `<for>`, `<while>` | Language keywords. |
| `foreach_statement` | `<foreach>` | Language keyword (C# spec). |
| `try_statement` | `<try>` | Language keyword. |
| `catch_clause` | `<catch>` | Language keyword. |
| `throw_statement` | `<throw>` | Language keyword. |
| `using_statement` | `<using>` | Language keyword. |
| `invocation_expression` | `<call>` | Matches other languages. |
| `member_access_expression` | `<member>` | Matches other languages. |
| `object_creation_expression` | `<new>` | Language keyword. |
| `assignment_expression` | `<assign>` | Consistent. |
| `binary_expression` | `<binary>` | Consistent; operator extracted. |
| `unary_expression` | `<unary>` | Consistent. |
| `conditional_expression` | `<ternary>` | Language concept. |
| `lambda_expression` | `<lambda>` | Matches other languages. |
| `await_expression` | `<await>` | Language keyword. |
| `variable_declaration` | `<variable>` | Consistent. |
| `variable_declarator` | `<declarator>` | C#-specific multi-declarator support. |
| `local_declaration_statement` | flattened | Purely structural. |
| `arrow_expression_clause` | flattened | The `=>` expression-body form. |
| `base_list` | `<extends>` | Developer mental model (`class Foo : Bar` reads as "Foo extends Bar"). |
| `string_literal` | `<string>` | Language concept. |
| `integer_literal` | `<int>` | Language keyword (int type). |
| `real_literal` | `<float>` | Language concept. |
| `boolean_literal` | `<bool>` | Language keyword. |
| `null_literal` | `<null>` | Language keyword. |
| `identifier` | `<name>` or `<type>` | Context-aware (see below). |
| `type_identifier`, `predefined_type` | `<type>` | Namespace vocabulary. |

## Structural transforms

### Flatten (Principle #12)

- `declaration_list`, `parameters` — structural wrappers.
- `enum_member_declaration_list` — the `{ Red, Green, Blue }` list
  inside `enum Color`.
- `local_declaration_statement` — wraps `type name = value;` inside
  a method body; the inner `variable_declaration` already becomes
  `<variable>`.
- `arrow_expression_clause` — the `=>` body form; children become
  body content directly.

### Flat lists (Principle #12)

| tree-sitter kind | field value |
|---|---|
| `parameter_list` | `parameters` |
| `argument_list` | `arguments` |
| `attribute_argument_list` | `arguments` |
| `type_argument_list` | `arguments` |
| `attribute_list` | `attributes` |
| `accessor_list` | `accessors` |
| `type_parameter_list` | `generics` |

### Nullable type rewrite

`nullable_type` in tree-sitter becomes a `<type>` with a
`<nullable/>` marker:

```xml
<type>Guid<nullable/></type>    <!-- from Guid? -->
```

### Generic type rewrite

`generic_name` (rewritten specifically for C#) applies the same
shape the shared helper produces for other languages:

```xml
<type>
  <generic/>
  List
  <type field="arguments">int</type>
</type>
```

### Identifier classification

`classify_identifier` trusts the tree-sitter kind but also needs
to handle C#'s namespace-qualified identifiers. The rule
(post-cleanup):

- If `field="type"` → `<type>` (type annotation position).
- If parent is a declaration (class/struct/interface/etc.) *and*
  this identifier is its name → `<name>`.
- If in a namespace qualification path → `<name>`.
- If a method/constructor name immediately followed by a
  parameter list → `<name>`.
- Default → `<name>` (value-namespace reference — Principle #14).

C# used to return `<ref>` for the default case. That was removed
(commit 25b5906) so C# matches every other language's convention.

### Access-modifier defaults

If a declaration has no explicit access modifier, the transform
inserts one based on context:

- Inside `interface_declaration` → `<public/>` (C# spec §18.4 —
  interface members are implicitly public).
- Inside `class_declaration`, `struct_declaration`, or
  `record_declaration` → `<private/>`.
- At namespace/top-level → `<internal/>`.

### Return type wrap

C# tree-sitter uses `field="returns"` natively, so the builder's
field-wrapping pass handles the `<returns>` wrapper automatically.

### Comment attachment

C# follows the cross-language rules — see
[`transformations.md`](../transformations.md) — *Comments*. Line
prefix is `//`; adjacent `//` comments group; declarations get
`<leading/>` and inline comments get `<trailing/>`.

## Language-specific decisions

### `<using_directive>` → `<import>`

A C# `using System;` is a namespace import. Every other language
in our set uses `<import>` for this concept. Unifying helps
cross-language queries and matches developer speech ("I'm
importing the System namespace"). The `<using_statement>` (for
`using (var x = ...)`) stays `<using>` — different construct.

### `<base_list>` → `<extends>`

`class Foo : Bar, IBaz` uses `:` for both base class and
implemented interfaces; C#'s tree-sitter groups them in
`base_list`. Renaming to `<extends>` matches Java/TS developer
speech. (The `<implements>` vs `<extends>` distinction from
Java doesn't apply here because C# uses the same syntax for
both — the distinction is only visible from which type reference
is a class vs interface, which isn't always recoverable
syntactically.)

### `<enum_member_declaration>` → `<constant>`

Matches Java's `enum_constant` → `<constant>`. Developer speech
is "enum constant" or "enum value"; `<constant>` is short and
correct (the parent `<enum>` disambiguates context).

### `<parameter>` vs `<param>`

C# uses the full word `parameter`. Other languages use `param`.
Historical inconsistency not yet unified — the rationale is that
C#'s original design preferred full words across the board. May
be revisited but not critical.

### Conditional shape (`<if>` / `<else_if>` / `<else>`)

C#'s tree-sitter grammar emits the common C-like nested
`else_clause` chain for `else if`. The transform collapses it into
the flat shape shared across all programming languages:

- `CSHARP_FIELD_WRAPPINGS` maps `consequence` → `<then>` and
  `alternative` → `<else>`; the `else_clause` kind is renamed to
  `<else>` via `map_element_name`.
- The post-walk `collapse_else_if_chain` (registered for C# in
  `languages/mod.rs`) unwraps the redundant `<else><else>` and
  lifts each inner `<if>`'s condition/then into an `<else_if>`
  sibling of the outer `<if>`, recursively.

See the cross-cutting "Conditional shape" convention in the index
[`transformations.md`](../transformations.md).
