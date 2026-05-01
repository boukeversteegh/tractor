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
