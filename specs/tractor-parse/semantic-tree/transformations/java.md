---
title: Java Transformations
---

Per-node decisions for the Java transform
(`tractor/src/languages/java.rs`).

## Summary

Java's tree-sitter grammar exposes a handful of patterns that need
special handling: a wrapping `<modifiers>` container around keyword
modifiers (public/static/final/etc.), an overloaded `field="type"`
that tags both parameter types *and* method return types, and
`interface_body` / `class_body` / `enum_body` wrappers that add no
semantic information.

The transform lifts modifiers into empty markers, wraps method
return types explicitly in `<returns>`, flattens the body/list
wrappers, and applies the default modifier that Java leaves implicit
(access on class members and interface methods).

## Element names

| tree-sitter kind | semantic name | rationale |
|---|---|---|
| `program` | `<program>` | Language keyword. |
| `class_declaration` | `<class>` | Language keyword (Principle #1). |
| `interface_declaration` | `<interface>` | Language keyword. |
| `enum_declaration` | `<enum>` | Language keyword. |
| `method_declaration` | `<method>` | Developer mental model (Goal #5). |
| `constructor_declaration` | `<constructor>` | Full word (Principle #2); matches C#. |
| `field_declaration` | `<field>` | Matches C# / Rust / JS field (Principle #5). |
| `enum_constant` | `<constant>` | Short, developer-readable (preferred over `enum_constant`). |
| `formal_parameter` | `<param>` | Short; matches TS/Rust/Go. |
| `import_declaration` | `<import>` | Language keyword. |
| `package_declaration` | `<package>` | Language keyword. |
| `generic_type` | rewritten — see below | Not a rename, a restructure. |
| `array_type` | `<array>` | Language concept. |
| `scoped_identifier`, `scoped_type_identifier` | `<path>` | `com.example.Foo` qualified name — short, consistent. |
| `super_interfaces` | `<implements>` | Developer term (Goal #5); matches the Java keyword. |
| `type_bound` | `<bound>` | The `extends Comparable` constraint on a type parameter. |
| `type_parameter` | `<generic>` | Matches the other languages' generic-parameter name. |
| `integral_type`, `floating_point_type`, `boolean_type`, `void_type` | `<type>` | Namespace vocabulary (Principle #14) — these are all type references; tree-sitter's split into multiple primitive kinds adds no semantic value. |
| `return_statement` | `<return>` | Language keyword. |
| `if_statement` | `<if>` | Language keyword. |
| `else_clause` | `<else>` | Language keyword; chain collapsed — see below. |
| `for_statement` | `<for>` | Language keyword. |
| `enhanced_for_statement` | `<foreach>` | Matches C#; `enhanced_for` is a spec term not used in speech. |
| `while_statement` | `<while>` | Language keyword. |
| `try_statement` | `<try>` | Language keyword. |
| `catch_clause` | `<catch>` | Language keyword. |
| `finally_clause` | `<finally>` | Language keyword. |
| `throw_statement` | `<throw>` | Language keyword. |
| `switch_expression` | `<switch>` | Language keyword. |
| `switch_block_statement_group` | `<case>` | Developer mental model — this IS a case. |
| `method_invocation` | `<call>` | Matches the other languages (Principle #5). |
| `object_creation_expression` | `<new>` | Language keyword. |
| `field_access` | `<member>` | Matches C# / TS shape. |
| `array_access` | `<index>` | Developer mental model — this is indexing. |
| `assignment_expression` | `<assign>` | Consistent with other languages. |
| `binary_expression` | `<binary>` | Consistent; operator extracted as `<op>` child. |
| `unary_expression` | `<unary>` | Consistent. |
| `ternary_expression` | `<ternary>` | Language concept. |
| `lambda_expression` | `<lambda>` | Language keyword-ish; matches Python's `lambda`. |
| `string_literal` | `<string>` | Language concept. |
| `decimal_integer_literal` | `<int>` | Language keyword. |
| `decimal_floating_point_literal` | `<float>` | Language keyword. |
| `true`, `false` | `<true>`, `<false>` | Language keywords. |
| `null_literal` | `<null>` | Language keyword. |

## Structural transforms

### Flatten (Principle #12)

- `class_body`, `interface_body`, `enum_body`, `block` — purely
  structural wrappers with no semantic meaning.
- `field_declaration_list` — the `{ field; field; }` wrapper inside
  a struct-like. Drop; fields become direct children.
- `type_list` — used inside `super_interfaces` to hold the list of
  interface types. After flattening, the interfaces are direct
  children of `<implements>`.

### Flat lists with field distribution (Principle #12)

- `formal_parameters` → children get `field="parameters"`.
- `argument_list` → children get `field="arguments"`.
- `type_arguments` → children get `field="arguments"` (inside a
  generic `<type>`).
- `type_parameters` → children get `field="generics"` (and the
  wrapper renames to `<generics>` before flattening? Currently the
  rename+flatten leaves no wrapper; see code).

### Generic type references (C# pattern)

```xml
<type>
  <generic/>
  List
  <type field="arguments">String</type>
</type>
```

Implemented by the shared `rewrite_generic_type` helper, using
`type_identifier` and `scoped_type_identifier` as the recognised
name-kinds for Java.

### Return type wrap

Java's grammar uses `field="type"` for both method return types
*and* parameter types. The builder's field canonicalisation can't
tell them apart. `wrap_method_return_type` walks the
`method_declaration`'s children, finds the one with
`field="type"`, and wraps it in a `<returns>` element — symmetric
with C#/Rust/TS/Go.

### Modifier lifting

Tree-sitter wraps modifiers in a `<modifiers>` element whose text
content is the whitespace-separated keywords. The transform
expands each known keyword into an empty marker (`<public/>`,
`<static/>`, `<final/>`, etc.) as a sibling of the declaration
it qualifies, then detaches the `<modifiers>` wrapper.

### Access-modifier defaults

If a class-body declaration has no access modifier, the transform
inserts one based on context (Java spec §9.4 and §8.x):

- Inside an `interface_declaration`: default is `<public/>`
  (interface members are implicitly public).
- Everywhere else (top-level, class/enum/record body): default is
  `<package/>` (Java's package-private access).

The `is_inside_interface` helper walks up from the declaration,
stopping at the first class/enum/record boundary so nested types
inherit the correct default.

## Language-specific decisions

### Why `<package/>` (not `<package-private/>`)

The tree needs a name for the implicit-access default. `package`
is shorter, lowercase-consistent, and matches how Java developers
describe it ("package access" or "package visibility"). The dash
in `package-private` would break XPath name tokens and we don't
use `-` in element names per the naming-conventions spec.

### Why `<constant>` (not `<enum_constant>`)

Developers reading Java code call enum values "constants" (or just
"values"). The compound `enum_constant` is tree-sitter's own spec
terminology, not developer speech. Short, correct, no context
loss because the parent is already `<enum>`.

### Why flatten `enum_body`

`<enum>` is a declaration element; its direct children are already
`<name>`, access markers, and the constants. An `<enum_body>` wrapper
adds a level of nesting that doesn't correspond to anything a
developer thinks about.

### Conditional shape (`<if>` / `<else_if>` / `<else>`)

Java's `if_statement` uses the common C-like nested-`else_clause`
shape: `if` → `consequence` (block) → `alternative` (an
`else_clause`, which wraps another `if_statement` when the source
says `else if`). The transform reuses the shared machinery:

- `CSHARP_FIELD_WRAPPINGS`-style wrappings apply via
  `COMMON_FIELD_WRAPPINGS`: `consequence` → `<then>`, `alternative`
  → `<else>`.
- The post-walk `collapse_else_if_chain` (registered in
  `languages/mod.rs`) collapses the nested `<else><else><if>…</if>`
  triple into an `<else_if>` sibling of the outer `<if>`, then
  recurses on the inner `<if>`'s own else chain. A terminal
  `<else>` with a block body is kept as-is.

See the cross-cutting "Conditional shape" convention in the index
[`transformations.md`](../transformations.md).

## Comments

Java follows the cross-language rules — see
[`transformations.md`](../transformations.md) — *Comments*. Line
prefix is `//`.
