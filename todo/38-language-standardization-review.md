# Language standardization — names to confirm

This todo collects defensible naming decisions made autonomously
during the simplify-node-names branch (iters 25–64). Each is a
**reversible micro-decision** — picked by principle, ship now,
queueable for review.

The pattern: when a transform introduced a new node name, marker,
or vocabulary choice, the loop picked the most defensible option,
documented the rationale, and listed the choice here. Renaming any
of these is mechanical (sed across rules + snapshots).

## How to use this list

For each item: read the rationale. If the chosen name still feels
wrong, propose an alternative; renaming is a single rule-table edit
plus snapshot regeneration. If the name is fine, strike it through
or delete the entry.

## Picks queued for review

### Parameter-shape markers (Python + Ruby — iters 50 / 51)

Splat parameters now wrap in `<parameter>` with a marker variant
distinguishing `*args` from `**kwargs`:

- `*args` → `<parameter[splat]>` (uses existing `Splat` marker)
- `**kwargs` → `<parameter[kwsplat]>` (new `Kwsplat` marker)

**Defensible per Principle #5** (cross-language unification —
matches Java's `<parameter[variadic]>` for varargs). Alternatives
considered:
- `<parameter[doublesplat]>` — descriptive but verbose
- `<parameter[dict]>` — re-uses the existing Dict marker (parallels
  `<spread[dict]>`); reads ambiguously ("a parameter that's a
  dict" vs. "a kwargs parameter")
- `<parameter[splat][dict]>` — multi-marker; not currently
  supported by `RenameWithMarker`
- `<parameter[keywords]>` — reads naturally for query authors

**To rename**: edit `Kwsplat` → `<NewName>` in
`tractor/src/languages/python/output.rs` and
`tractor/src/languages/ruby/output.rs`; the rule tables already
reference the variant, so a single rename + snapshot regen finishes
the change.

### Ruby `<constant>` collapsed to `<name>` (iter 39)

Ruby's `RubyKind::Constant` (capitalized identifier per the lexer)
now renames to `<name>` instead of `<constant>`. **Defensible per
Principle #5** — every other language uses `<name>` for value-
namespace identifiers regardless of casing.

The capitalization is preserved in the text content, so:
- `<name>Foo</name>` represents Ruby's lexical "constant"
- `<name>foo</name>` represents a regular identifier
- A query that needs the distinction can predicate on the text:
  `//name[matches(., '^[A-Z]')]`

**Alternative considered**: keep `<constant>` as-is, OR collapse to
`<name>` with an additional `<constant/>` marker for the lexical
distinction. The marker option was rejected as redundant — the text
already carries the distinction.

**To restore `<constant>`**: change
`RubyKind::Constant => Rename(Name)` back to `Passthrough` in
`tractor/src/languages/ruby/rules.rs`.

### TypeScript chained type assertions whitelisted as recursive (iter 45)

`<number>(<unknown>"42")` produces nested `<as>` elements. The
`tree_invariants::no_repeated_parent_child_name` invariant
whitelists `as` along with `path`, `pattern`, `member`, `type`,
`call`, `compare`, `binary`, `ternary`, `list`, `dict`, `tuple`,
`string`, `variable`.

**Defensible** — TS chained type assertions are intentionally
recursive (each `<x>` is a nesting of two type-assertion
expressions). Alternative: collapse into a single `<as>` with the
chained types as siblings (loses operator order).

### C# `?.` marker named `Optional`, not `Conditional` (iter 26)

`Root.MaybeProperty?.Property` produces `<member[optional]>` —
matches TypeScript's `OptionalChain` rule. **Defensible per
Principle #5** — same concept, same vocabulary across languages.

Alternative considered: `<conditional>` (the original C# tree-sitter
kind name). Rejected because TS already canonicalized `optional`
and reserving `conditional` for the ternary expression keeps the
two concepts distinct.

### C# `?.` shape isomorphic with regular member access (iter 57)

`Root.MaybeProperty?.Property` →
```
member[instance and optional]/
  ├─ member[instance]/...
  ├─ "?."
  └─ name = "Property"
```

Matches the regular `member[instance]` shape exactly except for the
`<optional/>` marker (and the source `?.` token vs `.`).

**User flagged** the original non-isomorphic shape; this is the
shape they explicitly asked for. No alternative under review.

### Python PEP 695 generics — `<generic>` wrap for declarations,
flatten for subscripts (iter 53)

- `def f[T]:` → `function/{name, generic/type/name=T, ...}` — matches
  Java `class[…]/generic/...` and TS declaration-level shape.
- `Optional[str]` → `type[generic]/{name=Optional, type/name=str}` —
  matches TS `type[generic]/{name=Map, type, type}`.

**Defensible per Principle #5**. Tree-sitter Python uses one
`type_parameter` kind for both contexts; the Custom handler
dispatches by parent.

### Ruby body-level expression hosts (iter 54)

Method bodies now wrap value-producing children in `<expression>`:
- `body/expression/string/interpolation/...` (was `body/string/...`)
- `body/expression/call/...` (was `body/call/...`)

Statement-only kinds (`<assign>`, `<if>`, `<while>`, `<class>`,
declarations, jump statements, comments) stay bare.

**Per Principle #15** — every other language gets `<expression>`
hosts via `expression_statement` rename; Ruby has no such kind, so
this post-walk simulates the same shape. The opt-IN list of
value-producing kinds (`RUBY_VALUE_KINDS` in
`tractor/src/languages/mod.rs`) may need expansion as new
constructs surface.

### Dual-use spec declarations for keyword-statement names (iter 47)

These names are now declared `marker: true, container: true` (dual-
use) in their per-language `output.rs`, since they appear both as
empty markers (bare keywords) and as containers (with content):

- **Rust**: `Pub`, `Unsafe`, `Break`, `Continue`, `Return`, `Yield`
- **Ruby**: `Break`, `Next`, `Redo`, `Retry`, `Return`, `Yield`
- **Go**: `Return`, `Break`, `Continue`, `Goto`, `Fallthrough`
- **C#**: `Throw`, `Return`, `Break`, `Continue`, `Yield`, `Class`,
  `Type`
- **Python**: `Pass`, `Break`, `Continue`, `Return`, `Yield`,
  `Class`, `Generic`, `Tuple`
- **TypeScript**: `Break`, `Continue`, `Return`, `Throw`, `Yield`
- **PHP**: `Break`, `Continue`, `Return`, `Throw`
- **Java**: `Break`, `Continue`, `Return`, `Throw`, `Yield`,
  `Annotation`, `Generic`
- **TSQL**: `Int`, `Varchar`, `Nvarchar`, `Datetime`, `Direction`,
  `Delete`, `Literal`

These declarations match the runtime shape — required by the
`tree_invariants::containers_have_content_or_are_absent`
invariant. Review: any name in the lists that should NOT be
dual-use (i.e., should always be a container with content)
indicates a runtime shape that needs fixing.

### Imports / use grouping vocabulary (iters 70/72/73/74)

Restructured imports across Go / PHP / Rust / TypeScript with the
shared structural shape (`<path>` / `<alias>` / variant markers).
Element name preserves each language's source keyword
(`<import>` for Go/Java/C#/TS/Python; `<use>` for PHP/Rust) per
the iter-71 Principle #5 scope clarification.

Marker vocabulary picks (queued for review):

- `[alias]` — used dual-form: marker on host AND `<alias>` child
  wrapping the local-binding `<name>`. Two queries, two clean
  paths. Alternative: marker only, name attribute on alias.
  Rejected — Principle #14 prefers `<name>` element for
  identifiers.
- `[blank]` (Go `_`) vs `[dot]` (Go `.`) — distinct names because
  the semantics differ (side-effect-only vs name-into-scope).
- `[group]` for braced multi-imports vs flat siblings for Go's
  parens. Asymmetry justified: parens carry no shared prefix,
  braces always do.
- `[wildcard]` (Rust `*`) vs `[namespace]` (TS `* as ns`) — two
  different concepts, different markers. TS `*` without `as` is
  a syntax error so there's no overlap.
- `[self]` (Rust `use std::fmt::self`) — the module itself.
- `[reexport]` (Rust `pub use`) — composes with `[pub]` visibility.
- `[function]` / `[const]` (PHP `use function` / `use const`) —
  PHP-specific namespace flavors.
- `[sideeffect]` (TS `import './x'`) — module evaluated for side
  effects only.
- `<path>` content: bare text for Go (single string `"net/http/pprof"`
  preserves source), nested `<name>` segments for languages that
  expose path segmentation (Rust, PHP, Java, etc.).

Cross-language test query: `(//import | //use)[alias]/alias/name`
extracts local bindings uniformly from any language's import.

### Go const / var bindings split into siblings (iter 76)

Each `const_spec` / `var_spec` becomes its own `<const>` /
`<var>` sibling. The block `const (A = 1; B = 2)` no longer
merges everything into one element — per binding is per element,
matching the iter-70 import handling.

### Snapshot cold-read iters 78-92

Iter 77 spawned a snapshot cold-read pass; iters 78-92 closed
findings:

- iter 78: Java path segments use `<name>`, not `<type>`.
- iter 80: PHP `op[instanceof]` (added to OPERATOR_MARKERS).
- iter 81: Java `dimensions` `[]` detached; `wildcard` `?` becomes
  `[wildcard]` marker.
- iter 82: Ruby pair keys extracted from text leaks (three forms).
- iter 83: C-family body braces stripped; keyword-statement bodies
  no longer leak text matching the marker.
- iter 84: Go qualified types wrap in `<type>`.
- iter 85: Python class-pattern keyword args distinguished by
  `[keyword]` marker.
- iter 86: PHP `variable[static]` / `variable[global]` de-duplicated.
- iter 87: TS object literal shorthand wraps in `<pair>`.
- iter 88: Ruby for-loop drops the noise `<in>` wrapper.
- iter 89: keyword text inside body matching marker siblings stripped.
- iter 90: empty body elements detached after brace strip.
- iter 91: Python class bases wrap in `<extends>` via field config.
- iter 92: Rust closure params uniformly wrapped in `<parameter>`.
- iter 94: PHP `op[concat] = "."` and `op[assign[concat]] = ".="`.
- iter 95: PHP cast type wraps in `<name>` (Principle #14).
- iter 96: Ruby `simple_symbol` becomes `<symbol>` (was bare text).
- iter 97: C# constructor chain `: this(0)` / `: base(...)` uses
  `[this]` / `[base]` markers (introduces `Base` enum variant).
- iter 99: body-brace-strip extended to `block`/`then`/`else`/
  `section`/`chain` per language; reverse iteration so child
  bodies process before parents (so empty parent bodies detach
  cleanly).
- iter 100: Java `default ->` switch label uses `[default]` marker
  (introduces `Default` dual-use enum variant).
- iter 102: Java `super()` / `this()` constructor invocations
  drop the keyword text and parens noise — `<call[super]/>` /
  `<call[this]>argument...</call>`.
- iter 103: Go `type Container[T any]` wraps generic params in
  `<generics>` (via `type_parameters` field-wrap config).
- iter 106: C# `<chain>` → `<call>` (matches Java's
  `<call[super]>` / `<call[this]>`; nobody calls `:` "chain").
- iter 107: heuristic-driven name fixes — Ruby `op[spaceship]` (was
  `compare-three-way`), PHP/Java multiple `<implements>` siblings,
  C# multiple `<base>` siblings (was `<extends>` list container).
- iter 108: Python class bases use `<base>` siblings (no list
  container); `parameter[args]` / `parameter[kwargs]` for `*args`
  / `**kwargs` (Python community names, Goal #5 — was `[splat]`/
  `[kwsplat]` which is Ruby-native vocabulary).

### Heuristics for naming (settled iter 107-108 per user)

These are the principled rules to apply when picking marker /
element names. Applies retroactively to evaluate existing picks.

1. **Operator without keyword** → use the language's idiomatic
   community name. Examples:
   - Ruby `<=>` → `op[spaceship]` (community standard).
   - PHP `.` → `op[concat]` (PHP idiom).
   - Python `:=` → `op[walrus]` (PEP 572 / community).

2. **Keyword or literal text in source** → use that text.
   Examples:
   - `**kwargs` → `[kwargs]` (Python convention text).
   - `*args` → `[args]` (Python convention text).
   - Ruby `*args` → `[splat]` (Ruby community name).

3. **Multi-target construct** (Principle #12 — no list containers):
   - **If source has a keyword** (Java/PHP/TS `extends`/`implements`,
     Python's effective `__bases__` accessed via `(...)` syntax):
     use the keyword as element name; multiple targets → multiple
     sibling elements (NEVER one wrapper around a list).
   - **If source has only punctuation** (C# `:`, Ruby `<`): name
     the *targets* using the language's idiomatic dev term.
     Multiple targets → multiple siblings.

   Examples:
   - Java `class Foo extends A implements B, C`:
     `<extends>A</extends><implements>B</implements><implements>C</implements>`.
   - PHP same as Java.
   - C# `class Foo : A, B`: `<base>A</base><base>B</base>` (no
     keyword in source; "base list" is the MS dev term).
   - Python `class Foo(A, B)`: `<base>A</base><base>B</base>`.
   - Ruby `class Foo < Base`: `<superclass>Base</superclass>`
     (single-target so no list issue; `superclass` is the Ruby
     idiomatic term).

Decisions queued for review (renames are mechanical):
- `[keyword]` marker name on pattern (vs `kwarg`, `kw`) — kept
  for cross-language uniformity with `parameter[keyword]`.
- `[wildcard]` for Java generic `?` (vs `[any]`) — kept; matches
  Java terminology.
- `[relative]` marker for Python `from . import` — kept; matches
  Python terminology.
- `[group]` marker on `<use>`/`<import>` for braced multi-imports —
  kept.
- `<extends>` for Python superclasses — matches Java/PHP; reverses
  on Ruby's `<superclass>` which is preserved (within-language).
