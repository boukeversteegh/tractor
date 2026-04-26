# Operator coverage audit

## Goal

Audit cross-language operator coverage — ensure every language-specific
operator either has a row in `OPERATOR_MARKERS` (with a chosen marker
name) or is explicitly documented as an intentional gap.

## Currently uncovered or undecided

- **Rust** `?` (try) — emitted as `<try>` element today. Out of the
  `<op>` table; verify whether that's intentional or whether it should
  have a `<try/>` marker on `<op>` instead.
- **C#** `?.` (null-conditional) — no marker today.
- **C#** `??` / `??=` — currently `nullish-coalescing` (matches TS).
  Verify the marker name is canonical.
- **Ruby** `<=>` (spaceship) — no marker.
- **Ruby** `=~` / `!~` (regex match) — no marker.
- **Python** `:=` (walrus) — verify whether emitted as
  `<assign[walrus]/>` or something else.
- **Python** `//=` (floor-divide-assign) — no marker today; rejected
  earlier as out-of-scope but flagged here for consistency.
- **Swift** (when language lands) — `?:` ternary, `??` nil-coalescing.

## Process

1. Sweep each language's transform + augmented-assign / binary fixtures
   to enumerate operators that hit `<op>`.
2. For each, decide: add to `OPERATOR_MARKERS` with a chosen marker
   name, or document as intentional gap (with rationale).
3. Update tests to lock the chosen state.
4. Update `specs/tractor-parse/semantic-tree/operator-element.md` if
   the spec doesn't already cover the additions.

## Origin

Surfaced during PR #148 (proposal E3).
