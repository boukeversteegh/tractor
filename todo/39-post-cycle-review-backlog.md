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
- **Reviewer "likely dead" claims are hypotheses, not facts.**
  Iter 157 deleted JSON/YAML `data_pair` `field=` writes on a
  reviewer's "likely dead" tip and broke 10+ mutation tests.
  Mutation/upsert logic depended on the attribute. Lesson: when a
  reviewer says "likely dead" / "probably unused", treat as a
  hypothesis to test (delete + run tests) before treating as fact.
  Reverted the deletion and added explanatory comments.
- **When a structural fix lands for one language, scan all
  language analogues in the same iter.** Iter 161 fixed Rust
  closure body single-name over-tagging; iter 162 had to follow
  up for TS arrow + Python lambda when a post-cycle review caught
  the parallels. Lesson: when fixing a node-shape archetype
  (closure / arrow / lambda / member-access / path / etc.),
  immediately grep blueprints for the same shape across all PLs
  and apply the fix in the same commit (or split deliberately
  per-language for blast radius reasons).
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
- **Read BOTH snapshot surfaces.** Every iter that touches
  transforms produces two diffs per affected language: the
  `.snapshot.txt` (tree shape, markers, `[@list="X"]` attrs) and
  the `.snapshot.json` (object-vs-array, key collisions,
  `children` fall-throughs — landed iter 170). Some shape
  regressions only manifest in the JSON: a 1-element array where
  a singleton was expected; a `"children": [...]` overflow key
  where a structural role-slot was intended; a cross-language
  key-name divergence (e.g. `"argument": ...` in one language vs
  bare keys in another). Cold-read pass and per-iter snapshot
  review must check both. Reviewer subagents should be told to
  scan JSON specifically.
- **Whack-a-mole / flip-flop is a real risk over many iters.** A
  later iter can silently reverse a deliberate decision from an
  earlier one — same element name, same marker, same flatten —
  without realizing the prior iter chose the opposite for a
  reason. Before implementing any rename / wrapper-vs-marker /
  flatten-vs-preserve change, grep this file (Open + Addressed +
  Lessons) and `git log --oneline --grep=<topic>` for prior
  decisions on the same element or kind. If a prior iter shipped
  the opposite, either (a) articulate WHY the new decision
  supersedes (new evidence; changed surroundings; better
  evaluation criterion) and tag the commit "Reverses iter N
  because…", or (b) abort and pick a different target. The Lessons
  section + Addressed list exist partly to make this check
  cheap — keep them populated with enough specificity that later
  iters can see what was decided.
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

- [x] **Drop `field=` writes in three internal helpers** — closed
  iter 164. `promote_field_to_wrapper` had zero callers and was
  deleted entirely (32 lines). `replace_identifier_with_name_child`
  and `wrap_text_in_name` had their `with_attr(name_el, "field",
  "name")` writes removed. All 829 tests green, snapshot diff empty
  — confirming the JSON serializer (post-iter-139) and XML output
  don't depend on these `field=` writes.

### `field=` claim about `--meta` is misleading

- [x] **iter 139 `--meta field=` claim verified iter 163** — the
  claim is *partially* accurate, not blatantly false. Verified
  with `echo 'class Foo { void m(int x) {} }' | tractor --lang
  java --format xml --meta`:
  - field= IS preserved on elements whose subtree wasn't
    text-inlined (parameters, returned types, etc. — `<type
    field="type">`, `<name field="name">int</name>`).
  - field= is LOST on elements where text-inlining replaced the
    inner identifier with raw text (class/method/parameter
    `<name>` wrappers, where `<name><identifier>X</identifier></name>`
    collapsed to `<name>X</name>` and the identifier with field=
    is gone).
  - The reviewer's "false" verdict was overstated; the iter 139
    framing of "available for --meta debug" is accurate for the
    cases where field= survives. Low priority — fixing the
    text-inlined case to copy field= would help debug round-trip
    but isn't structurally necessary.
  - Closing without code change. Lesson: "verify before acting"
    catches both false-claims AND overstated-claims.

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

### Closure/arrow/lambda body unification — round 2 (DONE)

- [x] C# lambda body unification — closed iter 167.
- [x] PHP arrow function body unification — closed iter 168.
- [x] Ruby Block/DoBlock body unification — closed iter 169 (partial,
  fully fixed in iter 173).
- [x] Ruby Lambda outer-body collapse — closed iter 174.

### TS arrow block-body fixture missing

- [x] **Block-bodied TS arrow added to blueprint** — closed iter 166.
  `const arrowBlock = (x: number): number => { return x * 2; };`
  added at line 123. Snapshot confirms `body/block[@list="block"]/...`
  shape (block-body branch of iter 162's `is_block` discriminator)
  while the single-expr arrow on the same line above renders as
  `value/expression/...`. Discriminator verified working.

### Stale doc-comment sweep round 3

- [x] **Three doc-comment sites updated** — closed iter 165.
  `apply_field_wrappings` doc, `replace_identifier_with_name_child`
  doc, and `Rule::Flatten` doc all corrected to match post-iter-164
  / post-iter-145 reality. Bonus: removed unused `Lambda` import in
  `python/rules.rs` (orphaned by iter 162 closure work).

### Ruby member-access role-wrap (fold into call-shape backlog)

- [ ] **Ruby `obj.method(arg)` adds a 4th call-shape variant** —
  flat `call[optional]/<name><name>...` at
  `tests/integration/languages/ruby/blueprint.rb.snapshot.txt:554-557`.
  Already-tracked Java method-call divergence backlog item lists
  3 distinct shapes; Ruby is a 4th. Fold into that item when the
  cross-language alignment design call is made. (severity: med;
  effort: small once decided)

### Standing items (re-flag every cycle)

- [ ] Snapshot cold-read pass every ~5 cycles — fresh eyes on every
  blueprint, surface anything suspicious that familiarity has hidden.
- [ ] When a transform test needs `(A or B)` disjunction or
  descendant-axis fallback, log as a candidate for re-shaping the
  underlying tree.

## Addressed

(Most-recent first. Older addressed items may be pruned periodically.)

- [x] iter 174: Ruby Lambda outer-body collapse. Stabby lambdas
  `->(x) { ... }` produced a doubled-body shape
  (`lambda/body/block/value/expression/...`) due to two field-wraps
  (lambda.body wraps the block element; block.body wraps statements).
  New post-pass `ruby_collapse_lambda_body` lifts the inner
  `<value>` (single-stmt) or `<body>` (multi-stmt) up to replace
  the outer body+block chain. Result:
  - single-stmt: `lambda/value/expression/binary/...` (matches Rust
    closure / TS arrow / C# lambda / PHP arrow / Python lambda).
  - multi-stmt: `lambda/body/expression: [...]` (mirrors Block
    multi-stmt shape from iter 173).
  Added `multi_lambda` fixture to expose both shapes. `lambda do ... end`
  is unaffected (parsed as a `call` to `lambda` with attached
  `<do_block>`, handled by the iter-173 path). All 829 tests green.
- [x] iter 173: fix iter-169 Ruby Block retag bug. Iter 169's
  body→value retag ran at walk-time before `block_body` flattened,
  always seeing "1 element child" (the unflattened block_body
  wrapper) and retagging multi-statement bodies wrongly. Moved to
  a post-pass `ruby_retag_singleton_block_body` that runs after
  `wrap_body_value_children` (so the count is post-flatten,
  post-expression-wrap). Reverted Block to Passthrough and
  DoBlock to RenameWithMarker(Block, Do). Added `multi_stmt`
  fixture (`proc { |n| puts n; n + 1 }`) to expose both shapes:
  - single-stmt: `block/value/expression/...` ✓
  - multi-stmt: `block/body/expression/.../expression/...` ✓
  In JSON: `block.value.expression.binary` (object) vs
  `block.body.expression: [...]` (array). The post-pass also
  wraps non-value-producing single statements (`<if>`, `<break>`,
  …) in `<expression>` to maintain a uniform `value/expression/X`
  shape (since `wrap_body_value_children` only wraps
  RUBY_VALUE_KINDS).
- [x] iter 172: hide `list=` from tree-text + XML rendering. Treated
  as a renderer-internal cardinality signal (parallel to `field=`
  iter 139 decision). `list=` still surfaces with `--meta` for
  debugging; xml_to_json continues to read it as the JSON
  cardinality driver. Two filters updated: `is_hidden_meta_attr`
  in `query_tree_renderer.rs:740` and the open-tag attribute filter
  in `xml_renderer.rs:447,789`. Snapshots: ~1.9K
  `[@list="X"]` predicates and `list="X"` attributes removed across
  all 9 blueprint .txt snapshots (XML report snapshots in
  tests/integration/cli/ also benefit). All 829 tests + 149
  fixtures green.
- [x] iter 171: strip redundant `$type` from JSON/YAML output.
  `$type` was being emitted on every non-text-leaf object; for
  ~90% of children it just repeated the parent's JSON key. Now
  stripped only when redundant (singleton lifted by element-name,
  or list-item where list-name == element-name). Kept where
  load-bearing: root, items in `children: [...]` overflow,
  list-items where list-name differs from element-name (e.g.
  `parameters: [{$type: "parameter", ...}]`). Render path was
  already key-tolerant. Per-language reduction: csharp 1042→114,
  ts 1081→84, rust 1022→97, java 525→28, go 788→87, python
  847→94, php 749→49, ruby 698→51, tsql 342→57. ~6.4K
  redundant `$type` lines gone across the JSON snapshots; YAML
  benefits identically (same code path).
- [x] iter 170: per-language JSON blueprint snapshots
  (`blueprint.<ext>.snapshot.json`) added alongside existing
  `.snapshot.txt`. Generated via `-p tree --single -f json`.
  149 fixtures total (was 140). Cardinality decisions (`list=` →
  array vs object) are now visible in fixtures, so
  shape-changes that only manifest in JSON consumers will surface
  in PR diffs. Already revealed: Ruby `MAX_RETRIES.zero?` call
  serializes with `name=MAX_RETRIES` and `children=["zero?"]` —
  receiver/method collision; folds into the existing
  Java/Python/Go/TS/Ruby call-shape divergence backlog item.
- [x] iter 169: Ruby Block + DoBlock body re-tag — call-attached
  closures (`each {}`, `proc {}`, `each do...end`, `loop {}`, etc.)
  now produce `block/value/expression/...` matching the
  iter-161-168 archetype. Custom handler with discriminator that
  skips early-return on `block_body`/`body_statement`
  intermediates (those flatten away later in the walk) and only
  bails on real nested closures (`block`/`do_block`). Lambda's
  inner block also benefits (partial fix; outer body shell still
  needs iter 170).
- [x] iter 168: PHP arrow function body unification — single-expr
  arrow `fn(...) => expr` retags `<body>` as `<value>` (matches
  C#/Rust/TS/Python archetype). PHP arrow is syntactically always
  single-expression, no discriminator needed.
- [x] iter 167: C# lambda body unification — single-expr lambdas
  retag `<body>` as `<value>` (matches Rust closure / TS arrow /
  Python lambda); block-bodied lambdas keep `<body>` via
  `is_block` discriminator. Added block-bodied lambda fixture.
  AnonymousMethodExpression (`delegate {...}`) routes through same
  handler; always-block keeps `<body>` naturally.
- [x] iter 166: TS arrow block-body fixture added; iter-162
  `is_block` discriminator now exercised.
- [x] iter 165: stale doc-comment sweep round 3 — `apply_field_wrappings`
  + `replace_identifier_with_name_child` + `Rule::Flatten` docs aligned
  to post-iter-164 / post-iter-145 reality; bonus dropped orphaned
  `Lambda` import in `python/rules.rs`.
- [x] iter 164: drop dead `field=` writes from three transform/mod.rs
  helpers (`promote_field_to_wrapper` deleted entirely;
  `replace_identifier_with_name_child` + `wrap_text_in_name`
  cleaned). Snapshot diff empty.
- [x] iter 162: TS arrow + Python lambda body re-tag (cross-language
  follow-up to iter 161 — closure-shape archetype fix applied to
  all single-expression-body languages).
- [x] iter 161: Rust closure body re-tagged as `<value>`.
- [x] iter 160: design.md decision — paths always emit JSON array.
- [x] iter 159: stale doc-comment sweep round 2.
- [x] iter 158: rename Rule::Flatten distribute_field → distribute_list.
- [x] iter 157: documented JSON/YAML data_pair field= as load-bearing.
- [x] iter 156: backlog format upgrade — checkboxes + lessons +
  post-compaction precision.
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
