---
title: TypeScript / JavaScript Transformations
---

Per-node decisions for the TypeScript transform
(`tractor/src/languages/typescript.rs`), which also handles
JavaScript, JSX, and TSX.

## Summary

TypeScript's tree-sitter grammar covers the most variation of any
language we handle: type annotations, type parameters, generic
references, arrow functions, class-field definitions, optional
parameters, call/member-expression field roles, and JSX.

The transform:

1. Uses FIELD_WRAPPINGS to canonicalise `return_type` → `<returns>`,
   and to wrap call-target / member-access fields as
   `<callee>` / `<object>` / `<property>`.
2. Hoists `async` and `*` (generator) into empty markers on
   function/method elements.
3. Marks required vs optional parameters exhaustively (Principle #9).
4. Rewrites generic type references into the shared C# pattern.
5. Harmonises JS (naked identifier params) with TS (wrapped `<param>`)
   so the tree shape is the same across both grammars.

## Element names

| tree-sitter kind | semantic name | rationale |
|---|---|---|
| `program` | `<program>` | Language convention. |
| `class_declaration` | `<class>` | Language keyword. |
| `function_declaration` | `<function>` | Language keyword. |
| `method_definition` | `<method>` | Developer mental model (Goal #5). |
| `arrow_function` | `<lambda>` | Matches Python/Java lambda naming; the shape *is* a lambda. |
| `interface_declaration` | `<interface>` | Language keyword. |
| `type_alias_declaration` | `<alias>` | Short, developer-readable; "type alias" is the spec term. |
| `enum_declaration` | `<enum>` | Language keyword. |
| `lexical_declaration`, `variable_declaration` | `<variable>` | Matches C#/Go. |
| `required_parameter`, `optional_parameter` | `<param>` | Short; matches other languages. |
| `statement_block` | `<block>` | Developer mental model. |
| `return_statement` | `<return>` | Language keyword. |
| `if_statement` | `<if>` | Language keyword. |
| `else_clause` | `<else>` | Language keyword; chain collapsed — see below. |
| `for_statement`, `while_statement` | `<for>`, `<while>` | Language keywords. |
| `try_statement` | `<try>` | Language keyword. |
| `catch_clause` | `<catch>` | Language keyword. |
| `throw_statement` | `<throw>` | Language keyword. |
| `call_expression` | `<call>` | Matches other languages. |
| `new_expression` | `<new>` | Language keyword. |
| `member_expression` | `<member>` | Matches C#/Java. |
| `assignment_expression` | `<assign>` | Consistent. |
| `binary_expression` | `<binary>` | Consistent; operator extracted. |
| `unary_expression` | `<unary>` | Consistent. |
| `ternary_expression` | `<ternary>` | Language concept. |
| `await_expression` | `<await>` | Language keyword. |
| `import_statement` | `<import>` | Language keyword. |
| `export_statement` | `<export>` | Language keyword. |
| `string` | `<string>` | Language concept. |
| `number` | `<number>` | Language concept. |
| `true`, `false` | `<bool>` | Both lift to the same element for symmetry. |
| `null` | `<null>` | Language keyword. |
| `predefined_type` | `<type>` | Namespace vocabulary (Principle #14). |
| `type_parameters` | `<generics>` | Plural-wrapper name; flat-listed. |
| `type_parameter` | `<generic>` | Singular name matches C# generic marker. |
| `type_identifier` | `<type>` | Namespace vocabulary. |
| `identifier`, `property_identifier` | `<name>` | Namespace vocabulary. |
| `generator_function`, `generator_function_declaration` | `<function>` | With `<generator/>` marker (see structural transforms). |

## Structural transforms

### Flatten

- `expression_statement` → Skip (children absorbed into parent).
- `variable_declarator` — the `name = value` pair inside a
  `<variable>`; flattened so the `<name>`, `<value>` etc. become
  direct children of `<variable>`.
- `class_body`, `interface_body`, `enum_body` — purely structural.
- `type_annotation` — the `:` prefix is a syntactic form of "followed
  by a type"; the `<typeof>` wrapper we used to add was confusing
  (the TS `typeof` operator is a different thing). Flatten; the `:`
  stays as a text sibling for renderers; the actual `<type>` appears
  directly.

### Flat lists (Principle #12)

- `formal_parameters` → children get `field="parameters"`.
  Before distributing, `wrap_bare_identifier_params` wraps any
  naked JS identifier child in a `<param>` so JS and TS produce
  the same shape (JS grammar emits bare identifiers for untyped
  params; TS wraps them in `required_parameter`).
- `arguments` (tree-sitter kind, not our field) → `field="arguments"`.
- `type_arguments` → `field="arguments"` (inside a generic `<type>`).

### Generic type references (shared helper)

```xml
<type>
  <generic/>
  Map
  <type field="arguments">string</type>
  <type field="arguments">number</type>
</type>
```

Implemented by `rewrite_generic_type` with name-kinds
`type_identifier`, `identifier`. If the inner name is a qualified /
nested identifier the helper leaves a `<name>` sub-element instead
of inlining to text.

### Variable keyword markers (exhaustive — Principle #9)

`lexical_declaration` and `variable_declaration` extract `let`,
`const`, `var`, `async`, `export`, `default` keywords into empty
marker children:

```xml
<variable>
  <const/>
  <name>x</name>
  <value>...</value>
</variable>
```

One of `<let/>`/`<const/>`/`<var/>` is always present (exhaustive).

### Parameter optional/required markers (exhaustive — Principle #9)

```xml
<param><required/>...</param>      <!-- foo(x: number) -->
<param><optional/>...</param>      <!-- foo(x?: number) -->
```

### Function markers

`extract_function_markers` handles five function-shaped kinds
(`method_definition`, `function_declaration`, `function_expression`,
`arrow_function`, and the two generator variants) and extracts:

- `async` keyword → `<async/>` marker.
- `*` prefix (generator) → `<generator/>` marker.

Both live on the complex node and cost effectively nothing in JSON
(each adds one boolean property; Principle #13).

### Binary/unary operator extraction

`extract_operator` pulls the operator text out of expressions like
`a + b` and puts it in an `<op>` child:

```xml
<binary>
  <op>+</op>
  <left><name>a</name></left>
  <right><name>b</name></right>
</binary>
```

## Language-specific decisions

### `<callee>` (not `<function>`) for call targets

Tree-sitter tags the function-being-called with `field="function"`.
If we wrapped that as `<function>`, it would collide with
`<function>` used for function *declarations*. FIELD_WRAPPINGS maps
`function` → `<callee>` instead, so precise queries can distinguish
"the function declared here" from "the function being called here"
(Goal #5 / precision over ambiguity).

### `<arrow_function>` → `<lambda>`

JavaScript docs call arrow functions "arrow functions", but at the
level of semantic category they're anonymous function expressions —
what most languages (Python, Java, Scala, Haskell) call lambdas. The
arrow-vs-function distinction is an implementation detail in a JS
dev's head. Using `<lambda>` keeps queries symmetric with other
languages.

### Harmonising JS naked params into `<param>`

JS grammar emits `function f(x) {}` with a bare `identifier` where
typed TS emits a `required_parameter`. `wrap_bare_identifier_params`
wraps naked identifiers in a `<param>` element before the
formal_parameters flat-list runs, so the resulting tree shape
`<function><param><name>x</name></param></function>` is the same
whether the file is JS or TS.

### No `<ref/>` empty marker

Previously the transform used an `<ref/>` empty marker inside role
wrappers like `<object><ref/>console</object>` to signal "bare
identifier". Issue #73 flagged this as confusing. The marker was
removed; the content is now a plain `<name>` element:

```xml
<member>
  <object><name>console</name></object>
  <property><name>log</name></property>
</member>
```

### Conditional shape (`<if>` / `<else_if>` / `<else>`)

TypeScript / JavaScript share the C-like nested-`else_clause` shape
— an `if_statement`'s `alternative` field is an `else_clause` whose
only child is another `if_statement` for `else if`. The transform:

- `TS_FIELD_WRAPPINGS` maps `consequence` → `<then>` and
  `alternative` → `<else>`; the `else_clause` kind renames to
  `<else>` as well.
- The post-walk `collapse_else_if_chain` (registered in
  `languages/mod.rs`) unwraps the redundant `<else><else>` wrapper
  that the field-wrap + rename produce, then lifts each nested
  `<if>`'s condition/then into an `<else_if>` sibling of the outer
  `<if>` and recurses until the chain ends in a plain block `<else>`
  or nothing.
- A ternary expression (`cond ? a : b`) reuses the same field
  names: `consequence` → `<then>`, `alternative` → `<else>` inside
  `<ternary>`.

See the cross-cutting "Conditional shape" convention in the index
[`transformations.md`](../transformations.md).

## Comments

TypeScript and JavaScript follow the cross-language rules — see
[`transformations.md`](../transformations.md) — *Comments*. Line
prefix is `//`; JSDoc blocks (`/** */`) stay as single comments,
structuring their internal tags is deferred to a separate
doc-comment shape.
