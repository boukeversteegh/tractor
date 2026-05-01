# Imports & use-grouping: cross-language shape

This doc captures the unified shape for import / use statements across
Go, PHP, Rust, and TypeScript. It supersedes the per-language ad-hoc
shapes that lost alias/blank/wildcard semantics.

## Goal

`//import[name='foo']` should match wherever a developer wrote a
statement that imports `foo`, regardless of language or syntax form
(plain, aliased, blank, wildcard, dot-import, group). Variant kinds
attach as markers on `<import>`.

## Shape

Every imported entity is an `<import>` element with these slots:

- `<path>`     — namespace path. Multi-segment paths use nested
                 `<name>` children (`<path><name>std</name><name>fmt</name></path>`).
                 Single-token paths can be bare text
                 (`<path>fmt</path>`).
- `<name>`     — the leaf identifier when separable from the path
                 (`HashMap` in `use std::collections::HashMap`).
                 Omitted when the path is the leaf (Go's quoted import).
- `<alias>`    — wraps the local binding `<name>` for aliased imports.
- markers      — variant kind on the `<import>` host:
                 `[alias]`, `[blank]`, `[dot]`, `[wildcard]`,
                 `[self]`, `[group]`, `[sideeffect]`, `[function]`,
                 `[const]`, `[reexport]`, `[namespace]`.

The element name is `<import>` for **all** languages, including Rust
and PHP whose source keyword is `use`. The cross-language uniformity
(Principle #5 Unified Concepts) wins over Principle #1 (Use Language
Keywords) for this concept.

## Examples

**Go**

| Source                        | Shape |
|-------------------------------|-------|
| `import "fmt"`                | `<import><path>fmt</path></import>` |
| `import myio "io"`            | `<import[alias]><path>io</path><alias><name>myio</name></alias></import>` |
| `import . "strings"`          | `<import[dot]><path>strings</path></import>` |
| `import _ "net/http/pprof"`   | `<import[blank]><path>net/http/pprof</path></import>` |
| `import (a; b; c)` block      | flat `<import>` siblings — no group wrapper |

**PHP**

| Source                       | Shape |
|------------------------------|-------|
| `use App\Base`               | `<import><path><name>App</name></path><name>Base</name></import>` |
| `use App\Logger as Log`      | `<import[alias]><path><name>App</name></path><name>Logger</name><alias><name>Log</name></alias></import>` |
| `use App\{First, Second}`    | `<import[group]><path><name>App</name></path><import><name>First</name></import><import><name>Second</name></import></import>` |
| `use function App\foo`       | `<import[function]><path><name>App</name></path><name>foo</name></import>` |
| `use const App\BAR`          | `<import[const]><path><name>App</name></path><name>BAR</name></import>` |

**Rust**

| Source                                    | Shape |
|-------------------------------------------|-------|
| `use std::collections::HashMap`           | `<import><path><name>std</name><name>collections</name></path><name>HashMap</name></import>` |
| `use std::collections::{HashMap, HashSet}`| `<import[group]><path><name>std</name><name>collections</name></path><import><name>HashMap</name></import><import><name>HashSet</name></import></import>` |
| `use std::collections::HashSet as Set`    | `<import[alias]><path><name>std</name><name>collections</name></path><name>HashSet</name><alias><name>Set</name></alias></import>` |
| `use std::fmt::self`                      | `<import[self]><path><name>std</name><name>fmt</name></path></import>` |
| `use std::fmt::*`                         | `<import[wildcard]><path><name>std</name><name>fmt</name></path></import>` |
| `pub use foo::bar`                        | `<import[reexport][pub]>...</import>` (visibility composes) |

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
- **Cost**: Rust source uses `use` but the element is `<import>`.
  Source text is preserved in the element's textual content; the
  semantic tag wins for queries.
- **Won**: cross-language uniformity. `//import[alias]/alias/name`,
  `//import[wildcard]`, `//import[blank]` work identically in
  every language.

## Implementation status

Designed iter 69 (subagent proposal). Implementation rolls out
language by language; each iter migrates one language to the new
shape and updates per-language transformation tests. Tracked in the
self-improvement loop.
