# Rule-table follow-ups (per language)

After the typed-Kind + Name-enum migration, each language's
`rules.rs` carried "TODO" comments describing kinds left as raw
passthrough. This file tracked the bucketed cleanup. Most buckets
landed during the simplify-node-names branch (iters 12–34); what
remains is captured below.

## Done — collapsed into the simplify-node-names branch

- ~~Bucket A — pure rename, sibling consistency.~~ Cleared during
  the per-language Phase 3 migrations and the underscore sweeps:
  C# (iter 20), Java (iter 19), TSQL (iter 21–22, 33), Python
  (iter 13), Ruby (iter 15), Rust (iter 16), Go (iter 12), PHP
  (iter 17). No `Rename(<existing variant>)` candidates remain.

- ~~Bucket B — new Name variant + rename.~~ Cleared:
  - C# `cast_expression` / `default_expression` / `throw_expression`
    / `with_expression` / array-creation forms / pattern combinators
    / lock/fixed/unsafe/checked/goto/yield/empty/labeled statements
    (iter 20).
  - Go `array_type` / `fallthrough_statement` / `slice_expression` /
    `type_conversion_expression` / `type_instantiation_expression` /
    `variadic_argument` (iter 12).
  - Java `module-info` / `annotation_type` / `pattern_combinator`
    families (iter 19).
  - TSQL `add_constraint` / `change_column` / `modify_column` /
    `rename_column` / `covering_columns` / `ordered_columns` etc.
    (iter 21).

- ~~TSQL list-vs-item double-wrap (`<col>/<col>`, `<arg>/<arg>`,
  `<constraint>/<constraint>`, `<create>/<create>`).~~ Cleared
  iters 33 / 34 / 43 / 45.

- ~~Python `aliased_import`, `type_parameter`,
  `list_splat_pattern` / `dictionary_splat_pattern`.~~ Cleared
  iters 45 / 50 / 53.

- ~~Ruby `*args` / `**kwargs` parameter unification.~~ Cleared
  iter 51.

- ~~Ruby `<constant>` collapse.~~ Cleared iter 39.

## Open — Bucket C (design call needed)

Each item below requires a per-language semantic decision before
implementation. They sit on the active backlog as Tier 4/5 items
(see plan file). Listing them here for cross-reference only — fix
them via the loop's normal "design + ship" cycle, not as a
mechanical sweep.

### Ruby

- **`alias` / `undef` declarations** — currently passthrough as
  `<alias>` / `<undef>`. Open: own semantic vs. shared with
  import-like? No fixture exercises these; pick when a user query
  needs them.

- **Numeric / literal kinds without underscored names** —
  `<character>`, `<complex>`, `<rational>` currently passthrough.
  Could pick proper Rename targets; not a query-priority concern
  yet.

- **Keyword constants** — `__ENCODING__` / `__FILE__` / `__LINE__`
  / `setter` / `subshell` / `super` / `uninterpreted` currently
  passthrough. Single-word, no Principle #17 violation; revisit
  if a query needs them.

### TSQL

- **Data-type kinds** — `<bigint>`, `<binary>`, `<bit>`, `<char>`,
  `<datetimeoffset>`, `<decimal>`, `<double>`, `<float>`,
  `<interval>`, `<mediumint>`, `<nchar>`, `<numeric>`, `<smallint>`,
  `<time>`, `<timestamp>`, `<tinyint>`, `<varbinary>` currently
  passthrough. Could unify under `<type>` with markers per kind
  (matches PHP's `<type[primitive]>` shape). Pending decision on
  marker vocabulary: per-keyword markers (`[bigint]`) vs. unified
  `[primitive]` marker.

### YAML

- **Directive / tag kinds** — currently passthrough. Question:
  do YAML directives (`%YAML 1.2`, `%TAG ! tag:`) belong in the
  syntax tree at all, or should they be detached?

## What's NOT in this todo anymore

Anything that was already implemented during the simplify-node-names
branch. The TODO sentinel comments in `rules.rs` files were
rephrased in iter 63 to point at this todo without the TODO
keyword (so `cargo run -- run` stays clean).
