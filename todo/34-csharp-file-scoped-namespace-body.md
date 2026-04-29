# Wrap file-scoped C# namespace declarations in a `<body>` for unified queries

## Context

C# has two namespace declaration forms:

- **Block-scoped** (classic): `namespace Foo { class A {} }`
- **File-scoped** (C# 10+): `namespace Foo;` followed by declarations
  at the top level.

Today the semantic transform exposes them with two different shapes
(see `tractor/tests/transform/imports.rs::csharp_namespace_block_vs_file_scoped`):

```xml
<!-- block-scoped -->
<namespace>
  <name>Foo</name>
  <body>
    <class>...</class>
  </body>
</namespace>

<!-- file-scoped -->
<namespace>
  <name>Foo</name>
</namespace>
<class>...</class>   <!-- flat sibling under <unit> -->
```

## Problem

The two forms describe the same thing — "these declarations belong
to namespace Foo" — but a query has to special-case both shapes:

- `//namespace[name='Foo']/body/class` finds block-scoped only.
- `//namespace[name='Foo']/following-sibling::class` finds file-scoped
  (and would also pick up declarations from later namespace blocks).
- A user writing "find all classes in namespace Foo" has to write a
  union or branch.

This split is an artefact of the tree-sitter grammar reflecting the
syntax. Semantically there is one concept; the transform should
unify the shape.

## Desired state

Both forms render with `<body>` wrapping the declarations. A marker
distinguishes the two:

```xml
<!-- block-scoped (default — the file-scoped marker is absent) -->
<namespace>
  <name>Foo</name>
  <body>
    <class>...</class>
  </body>
</namespace>

<!-- file-scoped -->
<namespace>
  <name>Foo</name>
  <file/>
  <body>
    <class>...</class>
  </body>
</namespace>
```

Queries that don't care about the syntactic distinction become
uniform:

- `//namespace[name='Foo']/body/class` — finds *all* classes in
  namespace Foo regardless of declaration form.
- `//namespace[file]` — finds file-scoped namespace declarations.
- `//namespace[not(file)]` — finds block-scoped declarations.

The `<file/>` marker is a Principle #13 cheap marker. An alternative
considered: omit the marker and let the presence/absence of `<body>`
itself distinguish them — but a marker is more explicit and matches
how other split forms (e.g. `<async/>`, `<generator/>`) are surfaced.

## What to do

1. In `tractor/src/languages/csharp/transform.rs`, when
   `file_scoped_namespace_declaration` is encountered:
   - Add a `<file/>` marker (Principle #13).
   - Wrap the trailing top-level declarations (siblings of the
     namespace at the `<unit>` level) into a `<body>` child of the
     namespace.
2. Update the catalogue (`csharp/semantic.rs`) — add `<file/>` and
   confirm `<body>` is permitted under `<namespace>`.
3. Flip the test in `tractor/tests/transform/imports.rs::csharp_namespace_block_vs_file_scoped`
   from "file-scoped has no body, declarations are flat siblings"
   to "file-scoped has `<body>` and a `<file/>` marker" — both forms
   should match the same `//namespace/body` shape.
4. Regenerate the C# blueprint snapshot
   (`tests/integration/languages/csharp/blueprint.cs.snapshot.txt`)
   if the blueprint includes a file-scoped namespace.
5. Snapshot regeneration; verify `task test`.

## Notes

- Surfaced while dropping the `namespaces-file-scoped.cs` rule
  fixture (Phase D, commit `74f1e65`); the test that replaces it
  pinned the *current* split shape, but the user wants this unified.
- Related: the design principle of "describe the same concept with
  the same shape" sits behind a lot of the existing transform
  decisions (e.g. always wrapping conditional bodies, always
  flattening parameter lists). Namespace-form unification is the
  same idea applied here.
