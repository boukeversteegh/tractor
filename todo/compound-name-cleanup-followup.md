# Compound-name cleanup: deferred items

Follow-ups from the tree-sitter compound-name cleanup pass across
Python/Go/Rust/C#/Java/TS. The items below were explicitly flagged
during that pass because the semantic naming is non-obvious and needs
a design decision rather than a mechanical rename.

## TypeScript / JavaScript

### `function_type`
Tree-sitter kind for TS function-type references: `(x: number) => string`
as an *annotation*, distinct from a function *declaration*. Naming is
ambiguous because `<function>` is already used for declarations.

Candidates considered:
- `<function>` — collides with declarations; a query like
  `//function` would match both definitions and bare function-type
  annotations.
- `<function_type>` — keeps the tree-sitter underscore; violates the
  "no compound tree-sitter names" rule we're enforcing elsewhere.
- `<signature>` — matches the common mental model ("the function's
  signature"), reads well in a type position
  (`<type><signature>…</signature></type>`).
- `<callable>` — more abstract; captures the fact that call
  expressions target *callables*, which includes function types.

Decision needed: probably `<signature>` inside a `<type>` wrapper.
Left as-is for now.

## C#

### `type_parameter_constraint`, `type_parameter_constraints_clause`
The `where T : class, new()` syntax on generic declarations. No direct
natural-language word for it.

Candidates considered:
- `<where>` — matches the keyword; short and queryable, but `where`
  is also an SQL/LINQ keyword and a Ruby/Swift concept — could clash
  if we ever unify cross-language queries.
- `<constraint>` / `<constraints>` — literal translation, wordy but
  precise.
- `<bound>` / `<bounds>` — matches Java's `type_bound` naming
  (cross-language consistency), but feels slightly off for the
  clause-level wrapper (`where T : class` is not just a "bound", it's
  multiple bounds).

Currently the two names leak through as
`<type_parameter_constraint>` / `<type_parameter_constraints_clause>`.

Decision needed.

## Rust

### `struct_expression`
The `Point { x: 1, y: 2 }` expression form of constructing a struct.
Conceptually this is a literal/constructor/creation, but none of those
words map cleanly.

Candidates considered:
- `<new>` — matches the mental model ("I'm newing up a Point"), and
  aligns with TS/Java/C# `new Foo()`. But Rust doesn't have `new` as a
  keyword — idiomatic Rust uses `StructName::new()` which is a
  `call`, not this syntactic form. Risk of confusion.
- `<struct>` — collides with `struct_item` (the declaration).
- `<literal>` — overloaded; we already use `<literal/>` as a marker on
  collections (Python).
- `<init>` — short, reads well, distinct from both `new` and
  `literal`.

Currently leaks through as `<struct_expression>`. Decision needed.

### `reference_type`
Rust's `&T` / `&mut T` type reference. Currently renamed to `<ref>`,
which collides with the deprecated value-reference element we
previously removed (issue #73, commit 25b5906). The collision is
latent — `<ref>` no longer appears anywhere else — but the name is
confusing.

Candidates considered:
- Keep `<ref>` — shortest, matches Rust's `&` sigil intent. But name
  is semantically overloaded with the now-deleted `<ref>` for value
  references; readers may assume the old meaning.
- `<reference>` — spelt out; unambiguous.
- Drop the rename entirely and leave as `reference_type` — violates
  the cleanup rule elsewhere.

Currently `<ref>`. Decision needed: rename to `<reference>` for
clarity, or keep `<ref>` since the collision is resolved.

## Items encountered but not in the brief

### Go `expression_list`
Not in the original task list, but `a, b = 1, 2` in Python produces a
`<pattern_list>` on the LHS (now flattened) and `<expression_list>` on
the RHS. Python's `expression_list` is the sibling of `pattern_list`
for tuple expressions; similarly, Go has `expression_list` in
multi-assignment. Flattening LHS patterns but not RHS expressions is
inconsistent. Probably both should flatten.

Currently: Go `expression_list` flattens (already landed). Python
`expression_list` does not flatten. Raise separately if we want
symmetry.

### Rust `match_block`
Tree-sitter `match_block` wraps the `{ arm1, arm2 }` block inside a
`match` expression. Not in the original task list, but a natural
candidate for flatten (purely-grouping wrapper around `match_arm`
siblings). Defer until user requests.
