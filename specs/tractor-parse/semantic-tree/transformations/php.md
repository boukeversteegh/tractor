---
title: PHP Transformations
---

Per-node decisions for the PHP transform
(`tractor/src/languages/php.rs`).

## Summary

PHP's semantic vocabulary mostly follows the cross-language pattern
(class / method / field / parameter etc.). The quirks are: the
`variable_name` kind for `$foo` identifiers (lifted into the
surrounding `<name>` text so `$foo` reads as a single name leaf),
interpolated strings (`"hi $name"` wraps interpolated expressions
in `<interpolation>`), and the `use` statement family which is
flagged below as an open design question.

## Open questions / flagged items

### Use statements — structural meaning lost in name soup

A single PHP `use` statement carries several semantically distinct
roles packed into adjacent `<name>` / text siblings. Current shape
(from the blueprint):

```
<use>
  "use"
  <name>App</name>
  "\\"
  <name>Logger</name>
  "as"
  <name>Log</name>
  ";"
</use>
```

Every `<name>` is opaque — the query `//use/name` returns `App`,
`Logger`, and `Log` without distinguishing which is the namespace
path, which is the leaf import, and which is the alias. The
semantic roles are in the source (`\\` delimiters, `as` keyword)
but not in the structure.

PHP `use` has more variants than this minimal example:

```php
use App\Logger;                            // fully-qualified class
use App\Logger as Log;                     // + alias
use App\{Logger, Cache, Db as DB};         // group
use function App\myFunc;                   // function import
use function App\myFunc as f;              // function + alias
use const App\MAX;                         // const import
```

All of these currently flatten into the same name/backslash soup.

**Proposed**: a `<use>` wraps a path, optional alias, and optional
kind marker as distinct children.

```
<use>
  <path><name>App</name><name>Logger</name></path>
  <as><name>Log</name></as>
</use>
```

With the kind markers:

```
<use function>
  <path><name>App</name><name>myFunc</name></path>
  <as><name>f</name></as>
</use>
```

Group form uses `<spec>` children, reusing the grouping-wrapper
convention flagged in `go.md`:

```
<use>
  <path><name>App</name></path>
  <spec><name>Logger</name></spec>
  <spec><name>Cache</name></spec>
  <spec><name>Db</name><as><name>DB</name></as></spec>
</use>
```

Benefits:

- `//use/as/name` — find every imported alias.
- `//use[function]/path` — every function-import path.
- `//use/path/name[last()]` — the leaf import name, independent
  of namespace depth.
- `//use/spec` — the individual imports inside a group form.

Related: Go's import / const / var grouping flagged as the same
class of problem (see `go.md`). The `<path>` + `<as>` + `<spec>`
shape could be shared across any import-ish construct.

**TODO:**

1. Stop flattening `namespace_use_clause` / `namespace_use_group` /
   `namespace_name` wholesale — keep `<path>` grouping intact.
2. Rename `namespace_aliasing_clause` → `<as>`; wrap the alias
   name inside.
3. Add `<function/>` / `<const/>` markers when `use function` /
   `use const` keywords are present.
4. Decide whether `<path>` wraps its segments as text-with-delimiters
   (keep `\\` as sibling text inside `<path>`) or promotes each
   segment to a separate `<name>` child; probably the latter for
   uniformity with the name-element rule (identifiers are a single
   `<name>` text leaf).
5. Shared design call: generalise `<spec>` across PHP `use`, Go
   imports, Go const/var, and possibly TS `import { ... }` named
   imports.
