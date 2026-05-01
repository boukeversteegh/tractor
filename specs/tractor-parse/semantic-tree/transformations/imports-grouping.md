# Imports & use-grouping: per-language structural shape

This doc captures the structural shape for import / use statements
across Go, PHP, Rust, and TypeScript. It supersedes the per-language
ad-hoc shapes that lost alias/blank/wildcard semantics.

## Goal

For each language, the import shape should be queryable enough to
answer "which imports of `foo`," "what's the alias for `bar`," "which
imports are side-effect only," etc. The *structural* shape (path /
alias / variant markers) is the same across languages — that's where
unification clearly pays off. The *element name* preserves each
language's source keyword (Principle #1).

## Element naming

Per Principle #5's within-language scope clarification:

- Languages whose source keyword is `import` use `<import>`:
  Go, Java, C#, TypeScript, Python.
- Languages whose source keyword is `use` use `<use>`: Rust, PHP.

A cross-language query for "any imported thing" is two paths
(`//import | //use`). The small disjunction is the correct cost for
keeping each community's mental model intact.

## Shape

Every imported entity (regardless of element name) carries the same
structural slots:

- `<path>`     — namespace path. Multi-segment paths use nested
                 `<name>` children (`<path><name>std</name><name>fmt</name></path>`).
                 Single-token paths can be bare text
                 (`<path>fmt</path>`).
- `<name>`     — the leaf identifier when separable from the path
                 (`HashMap` in `use std::collections::HashMap`).
                 Omitted when the path is the leaf (Go's quoted import).
- `<alias>`    — wraps the local binding `<name>` for aliased imports.
- markers      — variant kind on the host:
                 `[alias]`, `[blank]`, `[dot]`, `[wildcard]`,
                 `[self]`, `[group]`, `[sideeffect]`, `[function]`,
                 `[const]`, `[reexport]`, `[namespace]`.

## Examples

**Go**

| Source                        | Shape |
|-------------------------------|-------|
| `import "fmt"`                | `<import><path>fmt</path></import>` |
| `import myio "io"`            | `<import[alias]><path>io</path><alias><name>myio</name></alias></import>` |
| `import . "strings"`          | `<import[dot]><path>strings</path></import>` |
| `import _ "net/http/pprof"`   | `<import[blank]><path>net/http/pprof</path></import>` |
| `import (a; b; c)` block      | flat `<import>` siblings — no group wrapper |

**PHP** (element name `<use>`, structure same as `<import>`)

| Source                       | Shape |
|------------------------------|-------|
| `use App\Base`               | `<use><path><name>App</name></path><name>Base</name></use>` |
| `use App\Logger as Log`      | `<use[alias]><path><name>App</name></path><name>Logger</name><alias><name>Log</name></alias></use>` |
| `use App\{First, Second}`    | `<use[group]><path><name>App</name></path><use><name>First</name></use><use><name>Second</name></use></use>` |
| `use function App\foo`       | `<use[function]><path><name>App</name></path><name>foo</name></use>` |
| `use const App\BAR`          | `<use[const]><path><name>App</name></path><name>BAR</name></use>` |

**Rust** (element name `<use>`, structure same as `<import>`)

| Source                                    | Shape |
|-------------------------------------------|-------|
| `use std::collections::HashMap`           | `<use><path><name>std</name><name>collections</name></path><name>HashMap</name></use>` |
| `use std::collections::{HashMap, HashSet}`| `<use[group]><path><name>std</name><name>collections</name></path><use><name>HashMap</name></use><use><name>HashSet</name></use></use>` |
| `use std::collections::HashSet as Set`    | `<use[alias]><path><name>std</name><name>collections</name></path><name>HashSet</name><alias><name>Set</name></alias></use>` |
| `use std::fmt::self`                      | `<use[self]><path><name>std</name><name>fmt</name></path></use>` |
| `use std::fmt::*`                         | `<use[wildcard]><path><name>std</name><name>fmt</name></path></use>` |
| `pub use foo::bar`                        | `<use[reexport][pub]>...</use>` (visibility composes) |

**TypeScript**

| Source                                  | Shape |
|-----------------------------------------|-------|
| `import { a, b } from 'mod'`            | `<import[group]><path>mod</path><import><name>a</name></import><import><name>b</name></import></import>` |
| `import { a as x } from 'mod'`          | inner `<import[alias]><name>a</name><alias><name>x</name></alias></import>` |
| `import * as mod from 'mod'`            | `<import[namespace]><path>mod</path><alias><name>mod</name></alias></import>` |
| `import default from 'mod'`             | `<import><path>mod</path><name>default</name></import>` |
| `import './x'`                          | `<import[sideeffect]><path>./x</path></import>` |

## Grouping rules

- **Go `import (…)`**: NO wrapper. The parens are syntax sugar with
  no shared prefix. Imports become flat siblings of the parent
  (program/source-file).
- **Braced groups (Rust `{…}`, PHP `{…}`, TS `{…}`)**: outer
  `<import[group]>` with shared `<path>` and inner `<import>`
  children for each member. The braces always carry a shared
  prefix; duplicating it across siblings would be a lie.

The asymmetry is intentional: Go's parens are a bag; braces are a
prefix-share construct.

## Alias representation

Aliases use an `<alias>` element wrapping the local-binding `<name>`.
Both an `<alias>` child AND the `[alias]` marker appear:
- `//import[alias]` → finds aliased imports.
- `//import/alias/name` → extracts local bindings.
- `//import/name` → extracts the source-side leaf (the original).

The dual form is intentional: each query intent has a clean path.

## Variant marker vocabulary

| Marker         | Meaning                                  | Languages |
|----------------|------------------------------------------|-----------|
| `[alias]`      | Local binding renames the import         | All |
| `[blank]`      | Side-effect import, discarded binding    | Go (`_`) |
| `[dot]`        | Names imported into current scope        | Go (`.`) |
| `[wildcard]`   | All names imported (`*`)                 | Rust, Java |
| `[self]`       | Module itself imported (Rust `self`)     | Rust |
| `[group]`      | Braced multi-import with shared prefix   | Rust, PHP, TS |
| `[sideeffect]` | Module evaluated for side effects only   | TS (`import './x'`) |
| `[function]`   | PHP `use function …`                     | PHP |
| `[const]`      | PHP `use const …`                        | PHP |
| `[reexport]`   | Rust `pub use` re-export                 | Rust (composes with `[pub]`) |
| `[namespace]`  | TS `import * as ns from …`               | TS |

## Tradeoffs

- **Lost**: ability to query "all imports written in the same Go
  `import()` block." Considered a textual/locational concern, not
  semantic.
- **Lost**: distinguishing TS default-import from named-import via a
  marker. Defaults are rare and the path-vs-leaf shape suffices.
- **Cost**: cross-language queries need a small disjunction
  (`//import | //use`) to cover both keyword families. Acceptable —
  the structural shape under each is identical, so
  `(//import | //use)[alias]/alias/name` extracts local bindings
  uniformly.
- **Won**: within-language structural uniformity. The same paths/
  alias/marker shape applies to every language, so cross-language
  queries beyond just-find-imports stay clean.

## Implementation status

- iter 69: design proposal (subagent draft).
- iter 71: per-user clarification — Principle #5 scope is within-
  language; PHP/Rust keep `<use>`, only structure unifies.
- iter 70: Go shipped — `<import>` with full path/alias/blank/dot
  shape; block parens dissolved.
- PHP, Rust, TS: pending. Each rolls out as its own iter.
