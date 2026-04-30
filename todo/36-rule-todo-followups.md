# Rule-table TODO follow-ups (per language)

After the typed-Kind + Name-enum migration, each language's
`rules.rs` carries a block of "TODO" comments at the bottom describing
kinds the dispatcher currently leaves as raw passthrough. The fixes
fall into a few categories — see `tractor run` for the full list
(86 TODOs, all in `tractor/src/languages/*/rules.rs`).

The simple no-snapshot-impact ones (Go's `dot`, `parenthesized_*`,
`empty_statement`, `imaginary_literal`) have been resolved inline.
The remainder fall into three buckets, listed in increasing cost.

## Bucket A — pure rename, sibling consistency (no new variants)

Each of these is `Rename(<existing name>)` where the rename target
already exists in the language's `Name` enum. Snapshot impact is
either none (no fixture exercises) or local (one file's golden
output shifts in a way the maintainer can eyeball).

- **csharp** `as_expression`, `is_expression` → `Rename(Is)`
- **csharp** `event_declaration` → `Rename(Event)`
- **csharp** `conversion_operator_declaration` → `Rename(Operator)`
- **csharp** `element_access_expression` → `Rename(Index)`
  (already used by `element_binding_expression`)
- **csharp** `anonymous_method_expression` → `Rename(Lambda)`
- **csharp** pattern combinators (`and_pattern`, `or_pattern`,
  `not_pattern`, etc.) → `RenameWithMarker(Pattern, And/Or/Not)`
  if a marker variant already exists in CsName, otherwise bucket B.
- **go** `expression_case` → `Rename(Case)` (siblings
  `communication_case` / `default_case` already do this; affects
  one transform test asserting `expression_case/value/int=…`)

## Bucket B — needs a new Name variant

Adds one variant to the language's `Name` enum, possibly a new
syntax-category match arm, and an updated `Rename(...)` arm.

- **csharp** `cast_expression`, `default_expression`,
  `throw_expression` — likely each a new variant or shared
  `Rename(Call)` with markers (`<cast/>`, `<default/>`, `<throw/>`).
- **csharp** `with_expression`, `with_initializer` — new `With`
  variant, or `Rename(New)` with `<with/>` marker.
- **csharp** array-creation forms (`array_creation_expression`,
  `implicit_array_creation_expression`,
  `stack_alloc_array_creation_expression`) → `Rename(New)` with
  an `<array/>` marker (Array variant likely already exists for
  `ArrayType`).
- **csharp** special-statement forms — `lock_statement`,
  `fixed_statement`, `unsafe_statement`, `checked_statement`,
  `goto_statement`, `yield_statement`, `empty_statement`,
  `labeled_statement`. Each gets a new keyword variant per
  Principle #1 (use language keywords). `empty_statement` should
  likely just `Flatten`.
- **go** `array_type` → new `Array` variant; fold
  `implicit_length_array_type` into the same target. Snapshot
  impact: any file using `[N]T` arrays.
- **go** `fallthrough_statement` → new `Fallthrough` variant
  (alongside Break/Continue/Goto).
- **go** `slice_expression` → `RenameWithMarker(Index, Slice)` —
  Slice variant already exists for the type kind (it's dual-use
  marker/container per Principle #15).
- **go** `type_conversion_expression` → `RenameWithMarker(Call,
  Type)` so `//call[type]` matches every conversion.
- **go** `type_instantiation_expression` (`Foo[T]`) — share the
  `<type><generic/>...` shape used for `generic_type`.
- **go** `variadic_argument` → `RenameWithMarker(Argument,
  Variadic)` — Variadic is a new marker variant.

## Bucket C — needs investigation / design call

These imply a semantic question the migration's
"preserve byte-identical snapshots" rule deferred.

- **java**, **php**, **python**, **rust**, **ruby**, **typescript** —
  each has 6–17 unhandled kinds documented with proposed treatment
  in their respective `rules.rs`. Read the TODO blocks for the
  per-kind breakdown.
- **tsql** — 8 TODOs covering DDL constructs (alter_*, create_*,
  drop_*) and data-type kinds. Several need new constants for
  semantic SQL keywords.
- **yaml** — directive / tag kinds. Currently passthrough; the
  question is whether YAML directives belong in the syntax tree
  at all or should be detached.

## Approach

Per language, do the bucket A items first (they're free), then
bucket B (small additive change to `Name` enum). Run snapshot
checks; rebaseline only the affected files and verify diffs
visually. Defer bucket C until the user signals which language to
prioritise.

Each language should be one focused PR, not a sweep — the
snapshot diffs need eyeballing and the choice of new variant
names is a vocabulary decision the maintainer should make.
