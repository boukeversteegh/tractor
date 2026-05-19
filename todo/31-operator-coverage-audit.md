# Operator coverage audit

## Goal

Audit cross-language operator coverage — ensure every language-specific
operator either has a row in `OPERATOR_MARKERS` (with a chosen marker
name) or is explicitly documented as an intentional gap.

## Done

The simplify-node-names branch closed the bulk of this audit:

- **Cross-language coverage** — `typeof`, `void`, `defined?`,
  `floor` (`//`, `//=`), `matmul` (`@`, `@=`) added to
  `OPERATOR_MARKERS` in iter 9.
- **Go channel `<-`** — `op[receive] = "<-"` emitted via the table.
- **Go increment / decrement** — `++` / `--` carry the canonical
  marker on `<unary>`.
- **Token-boundary refinement** — `extract_operator` now finds the
  longest known operator prefix bounded by whitespace or
  end-of-string, fixing the `op = "== this"` concatenation that
  surfaced when adjacent anonymous keywords leaked into the same
  text leaf (iter 25).
- **C# `?.`** — structurally redesigned to be isomorphic with
  regular member access plus an `<optional/>` marker; vocabulary
  aligned with TS `<member[optional]>` (iters 26 / 57).
- **Ruby `&.` safe-navigation** — `<call[optional]>` matches the
  C# / TS shape (iter 64).
- **Ruby unary `defined?`** — already in `OPERATOR_MARKERS` with
  `<defined/>` primary marker.
- **Source-location threading** — `<op>` and its marker children
  carry `line` / `column` per Principle #10 (iter 37).

## Open

These remain genuinely undecided. Each is a small one-iter call:

- **Rust `?` (try)** — emitted as `<try>` element today (the
  `<expression>` host carries the marker per iter 27 work). Not in
  the `<op>` table. Decide: keep as a marker on the host (current
  state), or also add to `OPERATOR_MARKERS` so `//op[try]` works
  uniformly. Current state defensible per Principle #15 (markers
  on stable hosts), so a query-language audience chooses
  `//expression[try]` rather than `//op[try]`.

- ~~**Ruby `<=>` (spaceship)**~~ — *DONE iter 75.*
  `op[compare-three-way] = "<=>"` chosen over `spaceship` (more
  explicit, parallels existing `compare` family).
- ~~**Ruby `=~` / `!~` (regex match)**~~ — *DONE iter 75.*
  `op[match] = "=~"` and nested `op/{match[not]}/!~`.
- ~~**Python `:=` (walrus)**~~ — *DONE iter 75.* Rule changed from
  `Rename(Assign)` to `ExtractOpThenRename(Assign)` so `:=` extracts
  via `op/{assign[walrus]}/`.

- **Swift / Scala / OCaml** (when those languages get richer
  semantic transforms) — no audit yet; revisit per language.

## Process

1. For each Open item: pick a defensible marker name from
   principles + cross-language consistency.
2. Add to `OPERATOR_MARKERS` table in
   `tractor/src/transform/operators.rs`.
3. Extend per-language fixtures (e.g. `blueprint.rb`) so the
   shape is exercised in snapshots.
4. `task test`; review snapshot diffs.

## Origin

Surfaced during PR #148 (proposal E3). The simplify-node-names
branch closed most of it iter-by-iter; this version captures
what remains.
