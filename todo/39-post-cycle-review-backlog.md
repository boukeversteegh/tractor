# Post-cycle review backlog

Items flagged by post-cycle subagent reviews of self-improvement loop
iterations. Sources tagged with the iter range that surfaced them.
Open items use `[ ]`; an iter closes one by flipping it to `[x]`.
Sub-bullets are allowed when an item has multiple parts.

This file is the canonical place to land reviewer findings. When a
subagent flags issues that aren't fixed in the same loop cycle,
they get written here.

## Lessons (memory — avoid repeating these mistakes)

Patterns the loop has demonstrably gotten wrong. Re-read this list
before committing a non-trivial change.

- **Cross-language sweeps need per-language verification.** When wiring
  a shared post-pass into multiple languages, missing one is the
  default failure mode. Iter 152 + 153 were follow-ups to iter 151
  because Rust/PHP/Go were forgotten on first pass. Lesson: when
  introducing a new shared pass, immediately scan all language
  post-transforms and wire each in the same iter — don't promise
  "follow-up later".
- **Over-aggressive sweeps catch unintended cases.** Iter 140's
  `distribute_member_list_attrs` tagged self-closing markers because
  the predicate wasn't tight enough; iter 141 had to clean up.
  Lesson: when writing a "for every X, do Y" pass, explicitly
  enumerate what X *excludes*. Self-closing markers, text-only
  leaves, and singleton wrappers are common false-positives.
- **Sed-based bulk edits miss context-dependent cases.** Iter 154's
  doc-sweep used pattern replacement that worked on most sites but
  missed alternate phrasings. Lesson: always finish a doc sweep
  with a final grep for the OLD pattern across all targets — if
  any matches remain, revisit. The "review found N stale comments,
  iter fixed N-K" pattern is real and recurring.
- **Don't trust commit-message claims without verifying.** Iter 139
  claimed tree-sitter `field=` survives for `--meta` debug; the
  reviewer found this is false (renderer filters them). Lesson:
  when a commit message states a side-property, run the actual
  command first (e.g. `--meta` output) and confirm.
- **Skipping fixtures hides regressions.** Iter 130 fixed a C#
  primary-ctor bug but didn't add a fixture; iter 134 added one and
  immediately surfaced a regression that iter 130 had introduced.
  Lesson: every shape-fixing iter MUST land a fixture in the same
  commit, OR file an explicit follow-up to add one.
- **Helpers can be wrong silently.** `multi_xpath` stripped all
  whitespace — broke `[a and b]` predicates. Lesson: when a helper
  exists "just to make tests look nice", prove it's actually doing
  what's promised. Pass-through is sometimes the right answer.
- **Subagent reviews flag more than the next iter addresses.** Open
  items evaporate without a tracked backlog. This file exists
  because of that pattern. When closing items, mark `[x]`; when
  deferring, leave `[ ]` and reference the cycle that flagged it.
- **Over-tightening invariants traps real cases.** When adding a
  cross-cutting invariant test, sweep current fixtures FIRST and
  fix any pre-existing violations before committing the test —
  otherwise CI breaks unrelated work. Iter 146's no-dash invariant
  followed this; the dashed-operator renames landed in the same
  iter.

## Open

### Format note for new items

Open items are written so they can be acted on with no conversation
context. Each `[ ]` entry includes: **(1)** file path with line
numbers, **(2)** concrete current shape (verbatim), **(3)** desired
shape, **(4)** the Principle/Goal it serves, **(5)** rough effort
estimate, **(6)** the iter range that flagged it. Without the
current shape and target shape spelled out, post-compaction the
item can't be picked up.

### Cleanup of `field=` attribute setters that no longer drive JSON

After iter 139, the JSON serializer ignores `field=` entirely —
it reads `list="X"` for list-shape and uses element-name as the
key for singletons. Tree-sitter sets `field=X` during build, which
we preserve as input metadata for transform-time lookups (e.g.
`get_attr(child, "field") == Some("object")` to identify a
member-access receiver). But several transform sites still
*write* `field=` to wrappers — that output is dead.

- [x] **`field=` writes in JSON / YAML data-pair handlers** — iter 157
  investigated; attribute is **NOT dead**. The mutation/upsert logic
  in `tractor/src/mutation/xpath_upsert.rs` reads `field=` to identify
  data-pair elements during JSON/YAML write-path operations.
  Removing the writes broke 10+ mutation tests
  (`insert_simple_property`, `insert_nested_property`,
  `yaml_insert_*`, `declarative_set_json_nested`, etc.). Iter 157
  added a comment to both handlers documenting the attribute's
  load-bearing role for the data-tree write path. Validates Lesson:
  "Don't trust commit-message / review claims without verifying" —
  the reviewer's "likely dead" was wrong.

- [ ] **Drop `field=` writes in three internal helpers** in
  `tractor/src/transform/mod.rs` (likely dead post-iter-139 since
  `apply_field_wrappings` and `wrap_field_child` already had their
  `field=` setters removed in iter 139). Concrete callsites
  (re-verify line numbers — file has shifted):
  - `promote_field_to_wrapper` — sets `field=` on the synthesized
    wrapper.
  - `replace_identifier_with_name_child` — same pattern.
  - `wrap_text_in_name` — same pattern.
  - **Action**: grep for `with_attr.*"field"` in `tractor/src/transform/mod.rs`,
    drop each that's writing TO an output element (not reading
    from a tree-sitter input).
  - **Effort**: 15 minutes including verification.
  - **Source**: iters 138-140 review.

### `field=` claim about `--meta` is misleading

- [ ] **iter 139 commit message claims tree-sitter `field=` survives
  for `--meta` debug output**, but it doesn't — the field-wrapper
  pass + name_wrapper Customs inline the inner identifier text and
  the `field=` evaporates with it.
  - **Verify**: `echo 'class Foo {}' | ./target/release/tractor.exe --lang java --format xml --meta`.
    The `<name>` element will NOT show `@field=` even though the
    underlying tree-sitter `identifier` had `field="name"`.
  - **Two fixes possible**:
    1. Have the renderer copy `field=` from inlined-away inner
       identifiers onto the wrapper before inlining (preserves the
       claim).
    2. Drop the rationale from iter 139's commit message and accept
       that synthetic wrappers don't carry tree-sitter metadata.
  - **File for fix #1**: `tractor/src/output/xml_renderer.rs:443-455`
    is the meta-attribute filter; the actual `field=` loss happens
    earlier in the transform pipeline (per-language `name_wrapper`
    handlers — search `with_only_text` calls).
  - **Effort**: 30 minutes for option 2; 1-2 hours for option 1.
  - **Source**: iters 138-140 review.

### Rust closure body single-name over-tagged

- [x] **Rust closure body single-name over-tagged** — closed iter 161.
  Custom handler for `closure_expression` re-tags the inner
  `<body>` (from tree-sitter `field="body"` wrapping) as `<value>`
  before the post-pass `wrap_expression_positions` runs. The
  body's content gets wrapped in `<expression>` host (Principle
  #15), and `distribute_member_list_attrs` skips `<value>` since
  it's not in the container list. `body/name[@list="name"]="x"`
  → `value/expression/name="x"` (single-expr) or
  `value/expression/binary/...` (block-body, with block flattened
  by Pure Flatten rule).

### `Rule::Flatten` field rename

- [x] **`Rule::Flatten { distribute_field: ... }` enum field name** —
  closed iter 158. Mechanical rename across 17 files: enum variant
  field renamed `distribute_field` → `distribute_list`, all
  Rule::Flatten { ... } callsites updated. Helper signature
  parameters also renamed (Ruby, Custom handlers). All 829 tests
  green. The `distribute_field` identifier is now zero in the
  codebase.

### Java method-call shape divergence

- [ ] **Java method-invocation has `<call><object/><name="method"/>...</call>`**
  (flat shape, iter 148); **Python/Go method calls have**
  `<call><member><object/><property/></member>...args</call>`
  (nested via `<member>`); **TypeScript uses**
  `<call><callee><member>...</member></callee>...</call>`.
  - **Concrete examples**:
    - Java `mapper.apply("x")` → `<call><object/>mapper<name>apply</name>...</call>` (flat).
    - Python/Go `obj.method(x)` → `<call><member><object/><property/></member>...</call>`.
    - TS same `obj.method(x)` → `<call><callee><member>...</member></callee>...</call>`.
  - **Three different call shapes across four PLs** for the same
    syntactic construct. Within-language consistency is intact
    (`feedback_principle5_scope.md` permits this), but cross-language
    `//call/member/property/name='X'` queries are language-specific.
  - **Decision needed**: align to one of (a) Java's flat, (b)
    Python/Go's nested member, (c) TS's callee+member. Each has
    trade-offs the user should weigh.
  - **Effort**: 30 minutes (subagent design review) + 1-2 iters
    (per-language migration) once decided.
  - **Source**: iters 143-148 review.

### Stale doc-comment sweep — round 2

- [x] **Stale doc-comment sweep round 2** — closed iter 159. Bulk
  sed across 8+ files: `<extends field="extends">` →
  `<extends list="extends">`, `<type field="arguments">` →
  `<type list="arguments">`, removed dead `field="underlying"`
  references from PHP rules + transformations (the underlying
  marker is just `[underlying]` now, no field= companion). Tree-
  sitter input field= reads (e.g. `get_attr(child, "field") ==
  Some("object")` for member-access dispatch) preserved as-is.

### Single-segment paths render as 1-element arrays

- [x] **Single-segment paths render as 1-element arrays** — closed
  iter 160. Documented in `design.md` as a new Decision section
  "Path segments always emit as a JSON array". The 1-element-array
  shape is intentional per Principle #12 (`list="X"` always emits an
  array regardless of cardinality) — content-deterministic, no
  scalar-vs-array branching for consumers.

### Cross-language `<argument>` vocabulary mismatch

- [ ] **C# / PHP wrap call arguments in `<argument>`**:
  `<call><argument list="arguments">...</argument></call>`.
  **Java / Python / Rust / Go / TS use bare children**:
  `<call><string list="arguments">...</string><int list="arguments">...</int></call>`.
  - **Concrete fixture sites**:
    - C# wrap: `tests/integration/languages/csharp/blueprint.cs.snapshot.txt:200-220`.
    - PHP wrap: `tests/integration/languages/php/blueprint.php.snapshot.txt:23`.
    - Java bare: `tests/integration/languages/java/blueprint.java.snapshot.txt:101-103`.
  - **Cross-language query problem**: `//argument` finds C#/PHP
    args; misses Java/Python/Rust/Go/TS args (which would need
    `//*[@list="arguments"]`).
  - **Decision needed**: align all to wrap (Java/Python/Rust/Go/TS
    each gain `<argument>` wrappers) or align all to bare (C#/PHP
    drop `<argument>` wrappers).
  - **Effort**: 1 iter per language (~7 iters total) once decided.
  - **Source**: long-pending; flagged in multiple cycles.

### Standing items (re-flag every cycle)

- [ ] Snapshot cold-read pass every ~5 cycles — fresh eyes on every
  blueprint, surface anything suspicious that familiarity has hidden.
- [ ] When a transform test needs `(A or B)` disjunction or
  descendant-axis fallback, log as a candidate for re-shaping the
  underlying tree.

## Addressed

(Most-recent first. Older addressed items may be pruned periodically.)

- [x] iter 155: persistent backlog file (this file).
- [x] iter 154: C# import path-segment tagging + stale-doc sweep batch 1
  + drop unused `append_marker` import.
- [x] iter 153: PHP + Go wired into `flatten_nested_paths`.
- [x] iter 152: Rust wired into `flatten_nested_paths`.
- [x] iter 151: shared `flatten_nested_paths` post-pass; Java + Python wired.
- [x] iter 150: Go `qualified_type` package-wrap.
- [x] iter 149: stale-doc sweep for iter 146 operator renames; fix
  `distribute_list_to_children` doc.
- [x] iter 148: Java method_invocation receiver wraps in `<object>`.
- [x] iter 147: member-access role-wrap (Java/Python/Go to match TS).
- [x] iter 146: `no_dash_in_node_names` invariant + dashed operator
  marker renames (`not-equals` → `inequality`, `or-equal` → `equal`,
  `nullish-coalescing` → `nullish`, `floor-divide` → `floor`).
- [x] iter 145: rename `distribute_field_to_children` → `distribute_list_to_children`.
- [x] iter 144: design.md path-segments example matches actual emit.
- [x] iter 143: Go composite-literal `keyed_element` → `<pair>`.
- [x] iter 142: drop dead `is_anon_text_entry` stub.
- [x] iter 141: tighten `distribute_member_list_attrs` — skip self-closing
  markers.
