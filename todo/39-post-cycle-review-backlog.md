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
- **"All N languages done" claims need a per-language verification
  step.** Iter 147's "all PLs sweep" missed C# member-access; iter
  174's "all 8 PLs" closure-archetype claim missed Go. Both
  surfaced ~25-30 iters later when the next cold-read pass ran.
  Lesson: when an iter ends with a claim that spans multiple
  languages, in the same iter spawn an explicit verification step:
  for each language, run a probe (or grep blueprint snapshots) for
  the target shape and check it's present. Don't lean on the
  spec-tag sweep to catch missed languages — it has the same
  blindspot as the original sweep.
- **Field-wrap is global per-language; can't scope per-kind.**
  Tempting to add `("consequence", "then")` / `("alternative", "else")`
  to FIELD_WRAPPINGS for ternary support — but those field names also
  appear on if/elsif/while/until and breaking-but-mechanical chain
  passes (collapse_else_if_chain). Always: when a field-wrap entry
  would wrap a per-kind concept, use a Custom handler scoped to that
  kind, calling **`crate::transform::helpers::wrap_field_child(xot,
  node, "field_name", "wrapper_name")`** — same one-liner shape, but
  scoped to the kind whose Custom handler is invoking it. Iter 179
  was caught by test; would have been a quiet regression otherwise.
  Iter 347 retried this for Rust if-let `pattern` field via global
  wrap, broke parameters, reverted (parameters also use field=pattern).
  (Sister to the "all N languages done" lesson: changes that look
  universal often aren't.) Iter 348 added discoverability comments
  on `apply_field_wrappings` and the FIELD_WRAPPINGS const declaration
  pointing at the helper.
- **Bulk `distribute_member_list_attrs` is a coverage-vs-correctness
  trade-off and SILENTLY REVERSES singleton-role-slot designs.**
  Iters 205-212 closed 121 audit overflow sites but created ~2,389
  new 1-element arrays — net JSON-shape regression. The audit metric
  (`"children": [` count) only measures ONE failure mode
  (anonymous-key overflow) and IGNORES the dual failure mode (1-elem
  arrays on singleton role slots). Concretely reversed prior
  iter decisions: 178 (`<member>` object/property), 179 (`<ternary>`
  condition/then/else), 180 (`<range>` from/to), 195 (Ruby `<call>`
  object), 212 (`<binary>`/`<unary>`/`<logical>` left/op/right).
  Before adding ANY element name to a distribute config, classify
  it: role-uniform (siblings are interchangeable list members —
  body/program/file/tuple/list/dict/array/hash/object/switch/
  literal/template) vs role-MIXED (siblings are distinct named
  slots — call/binary/member/ternary/range/try/with/decorator/as/
  logical/unary/condition/then/from/index/case). Only role-uniform
  parents are safe to bulk-distribute.
- **When extending a config-driven sweep (distribute, field-wrap,
  tag-multi), grep prior commits with `--grep=<element_name>` for
  EACH added name** — config sweeps are particularly prone to
  undoing prior single-element handler decisions because the audit
  metric doesn't surface the regression. The whack-a-mole check
  must apply per-name, not per-iter.
- **Field-wrap can be silently undone by a per-language Skip
  dispatcher.** TSQL `tractor/src/languages/tsql/transform.rs:29`
  routes builder-inserted `<left>`/`<right>`/`<value>` wrappers to
  `TransformAction::Skip` — intentional design choice from earlier
  work, but it makes the audit see "config says yes, output says
  no." When investigating "why doesn't field X wrap in language Y,"
  FIRST check the language's transform dispatcher for an explicit
  Skip route on the wrapper element name BEFORE adding more config
  or chasing parser-level bugs. Iter 197 review surfaced this; iter
  185 chased a non-issue at the field-wrap layer for over an hour.
- **Premature cross-language commitment in shape-divergent areas.**
  When committing to one of multiple under-debate cross-language
  shapes (e.g. Ruby `<call>` adopting Java's flat receiver-shape in
  iter 195 while the 4-way design call between Java flat /
  Python+Go nested member / TS callee+member is still pending),
  call it out in the commit message: "matches Java pending design
  call". Reduces archaeology cost when a future "revisit" iter
  needs to find all the affected sites. The blueprint-reduction
  signal can be strong enough to justify shipping early; the
  reversibility is what matters.
- **Marker name = wrapper name → JSON key collision.** When a
  concept has both a boolean marker (e.g. `<alias/>` empty) AND a
  structural wrapper (e.g. `<alias>X</alias>`) on the SAME parent,
  the JSON serializer collides on the same key — the marker lifts
  as `alias: true` and the wrapper falls through to anonymous
  `children:` overflow. Iter 184 resolved by renaming wrapper to
  `<aliased>` (kept `[alias]` marker for query stability). When
  introducing any new structural wrapper, grep for an existing
  marker by the same name; if found, pick a distinct wrapper name
  (`<aliased>`, `<bound>`, `<wrapped>`, …) BEFORE landing. Iter 180
  (range bounds) avoided this trap by using distinct from/to;
  iter 184 retroactively fixed an old collision.
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
- **Run `tractor run` (self-lint) AFTER every test-file edit, not
  just after source edits.** Iter 267 added a `let mut tree`
  followed by a single use, ran `tractor run` BEFORE adding the
  new test (got Exit: 0), then added the test and committed. CI
  caught the lint violation; iter 268 had to re-fix it. The
  pre-commit ordering should always be: (1) edit code + tests,
  (2) cargo build/test, (3) cargo run update-snapshots, (4)
  `tractor run` self-lint, (5) commit. Skipping or re-ordering
  step 4 means the project's own lint rules don't see the test
  files.
- **Snapshot updates produce BOTH .txt AND .json diffs — stage
  both.** Iter 283 changed TS meta-property rendering, ran
  update-snapshots, and committed only the .json snapshot — the
  parallel .txt diff at the same site got left unstaged. CI's
  `task test:snapshots --check` caught the mismatch on the next
  run; iter 285 had to ship a fixup. The pre-commit checklist
  must include `git diff --stat tests/integration/` to verify
  both surfaces are clean. Don't add specific files by glob; if
  unsure, `git status` shows untracked snapshot files that need
  staging.
- **Green tests ≠ no regression.** This is the meta-lesson that
  unifies iters 184/213/275/283. The audit metric (`"children":
  [` count) and `cargo test` together optimize for ONE failure
  mode while staying silent about others: 1-element JSON arrays
  on singleton role-slots, marker/wrapper name collisions,
  cross-language divergence newly introduced, and unintended
  snapshot churn in fixtures the iter didn't mean to touch. The
  iter-205-to-213 episode is the canonical example: 121 sites
  closed, ~2,389 new 1-element arrays created, 5 prior decisions
  silently reversed — all while tests stayed green. The fix is
  the strict-improvement gate codified in the loop plan
  (`mossy-riding-parrot.md` § step 7a): before pushing, classify
  every line of the snapshot diff as intended or
  incidental-but-neutral; no unfixed regression on any
  dimension. When in doubt, spawn a fresh diff-only reviewer
  subagent. Push is the completion signal, not the commit.

## Open

### Iter 325 wind-down — phase 2 + audit cluster complete

**State (post-iter-325)**: the loop has reached a natural pause after
~26 consecutive iters of validation tooling, distribute-config audit,
and invariant migration work. Three independent compounding
improvements all stable:

**Validation tooling** (iter 292+):
- 9 spec-contract rules active in
  `tractor/src/transform/shape_contracts.rs`, running both via
  cargo integration test (layer 1) AND debug-build assertion in
  every `post_transform` (layer 2).
- Strict-improvement gate (loop step 7a) mechanized for the major
  regression archetypes — Error-severity rules block at CI.
- Grandfather ratchet (iter 298) lets advisory rules block NEW
  regressions while existing population stays grandfathered.

**Distribute-config audit** (iters 302-311 + 323-324):
- ~342 JSON lines saved across **10 trap classes closed**:
  `switch`, `import`, `literal`, `namespace`, `string`, `template`,
  `macro`, Go-only `array`, Ruby-only `array`, TS-only `array`.
- Pattern: drop bulk on role-mixed parents; add targeted
  `(parent, child)` role tags for multi-cardinality children.
- Ratchet caught + helped fix several mid-iter regressions
  (iter 306 / 307 / 309).

**Invariant migration** (iters 312-322):
- **11 of 12 original hand-coded invariants** in `tree_invariants.rs`
  now spec-driven. Tree invariants 12 → 1 hand-coded.
- The remaining 1 (`no_underscore_in_node_names_except_whitelist`)
  has a static rule-table side that's fundamentally a build-time
  check; appropriate to keep hand-coded.
- Three deferred-migration cycles demonstrated end-to-end (revert →
  fix root cause → retry): iter 315→316→317, iter 319→320,
  iter 321→322.

**Cross-cutting transform names** (iter 321/325):
- `Subscript` declared in all 6 main programming-language enums
  (chain inverter emits it for `arr[0].field` chains).

**Remaining known work** (all need user direction or design judgment):
- 11 cold-read findings still open (iter 300 list above), several
  flagged HIGH but each needs design review:
  - PHP foreach `<value>` for iterable + `<pair>` shape collision
  - Ruby destructure left-spine bug
  - Java/C#/Rust switch-arm guard cross-language naming
  - C# anonymous-object name collision (deferred design class)
  - Chain-receiver-call collisions (deferred class — see iter 291)
- 9 children-overflow sites grandfathered (iter 291 wind-down list)
- TSQL MERGE shape (iter 292 finding — needs scope verification)

**Next iter loop pickup**: when user explicitly directs OR a fresh
cold-read pass surfaces new tractable items. The autonomous-pickup
options have been exhausted; remaining work is design-y or
cross-cutting and benefits from human input.

The strict-improvement gate (iter 292) and ratchet (iter 298) remain
the primary defenses against future regressions. Both proven to
work mid-iter across this cluster.

### Iter 315 — grammar-suffix migration revealed pre-existing issues

Attempted migration of `no_grammar_kind_suffixes` from
`tree_invariants.rs` to a spec-contract rule (continuing iters
312/313/314 invariant migration). The layer-2 coverage (every
transform invocation) revealed two pre-existing grammar-suffix
issues the blueprint-scoped invariant never saw:

- [x] ~~**Markdown `<code_block>`**~~ *(CLOSED iter 344)*. Renamed
  to `<codeblock>` (single word, no underscore) following the
  `<elseif>` precedent. CommonMark has no canonical unifying name
  for fenced + indented forms; alternatives (`<fenced>`/`<indented>`
  split, or `<block[code]>` conflict) were worse. The
  `GRAMMAR_SUFFIX_EXEMPT` list in shape_contracts.rs is now empty.

- [ ] **PHP empty `<static_modifier>` / `<visibility_modifier>`** —
  PHP's `modifier` Custom handler at
  `tractor/src/languages/php/transformations.rs:299` only renames
  when `text.trim()` is non-empty; empty modifier nodes survive with
  their grammar kind name. Tree-sitter-php emits empty modifier
  nodes for some default-public field declarations. Fix: either
  Detach empty modifier nodes (no semantic content) or rename
  unconditionally to a placeholder marker.

Iter 315 reverted the migration. Once both issues land, the
`no-grammar-kind-suffix` rule can be added to shape_contracts.rs and
the hand-coded `no_grammar_kind_suffixes` retired.

### Iter 300 cold-read findings

Cold-read pass spawned iter 300 (first since iters 233/186/197 cluster).
Reviewer read every blueprint snapshot (.txt + .json) without prior
context. 15 findings; severity per reviewer.

- [x] ~~**TypeScript: two competing shapes for member/subscript
  access**~~ *(HIGH; CLOSED iter 345)*. Unified subscript and
  member access into the same chain shape — both use
  `<object[access]>/<receiver/>/<step>/...` where step is
  `<member>` for dot or `<index>` for bracket. Bracket text leaks
  removed. All TS index access sites now consistent. See
  `chain::cross_language_index_access_chain_inverts` test for the
  cross-language unified shape.

- [x] ~~**Ruby: nested `left/expression/left/expression/left/...`
  destructure**~~ *(HIGH; CLOSED iter 346)*. Changed
  `RubyKind::LeftAssignmentList` from `Rename(Left)` to `Flatten`,
  promoting the comma-separated names directly into the outer
  field-wrapped `<left>`. Eliminates one redundant `<left>` level
  in both `_, a, b = [...]` and `(a, b) = [...]` forms. JSON shape
  is now a direct `assign.left.expressions: [...]` array.

- [ ] **Rust: `if`/`while` `condition/` has role-mixed flat
  children for `if let` and let-chains** *(HIGH, principle #19;
  iter 347 attempted, reverted)*. Each `expression/` sibling
  means something different (matched constructor, bound name,
  scrutinee, guard) but they're presented as a flat list of
  identical `expression` siblings. Querying "the scrutinee of an
  `if let`" requires positional logic.
  Reproduce: `tests/integration/languages/rust/blueprint.rs.snapshot.txt:536-541, 553-558, 897-912`.

  **Iter 347 attempt notes**: tried adding `("pattern", "pattern")`
  to `RUST_FIELD_WRAPPINGS` so the let_condition's field=pattern
  child wraps in `<pattern>` before Pure Flatten. This produced
  the desired `<condition>/<pattern>...</pattern>/<value>...</value>`
  shape for `if let Pat = Scr` — but BROKE Rust parameter shape
  because parameters also use field=pattern. `<parameter>/<name>`
  became `<parameter>/<pattern>/<name>`, breaking `//parameter[name='X']`
  queries.

  Knock-on issues surfacing during the attempt:
  - `wrap_expression_positions` wraps `<pattern>` and `<value>`
    siblings of `<condition>` in `<expression>` — needs a skip
    rule (pattern/value are slot wrappers, shouldn't get
    expression hosts).
  - Rust let-chains `if let A && let B && cond` produce multiple
    `<pattern>` and `<value>` siblings — need
    `("condition", "pattern")` and `("condition", "value")`
    role-tag entries to avoid `no-children-overflow` ratchet.

  **Cleaner fix path** for next attempt:
  - Custom handler for `LetCondition` that explicitly wraps the
    field=pattern child in `<pattern>` (narrow scope, doesn't
    affect parameter declarations).
  - Add the wrap_expression_positions skip rule for `<pattern>`
    and `<value>` (defensive — applies broadly, prevents the
    "slot wrapper inside slot wrapper" double-wrap).
  - Add the let-chain list-tag entries for role-uniform multi
    pattern/value siblings.
  - Verify Rust parameter shape unaffected (parameters are NOT
    let_condition kind; the Custom handler scopes to LetCondition
    only).

  Iter 347 reverted with no commit; needs subagent design review
  before retry covering these three concerns.

- [x] ~~**PHP `foreach`: `value/` is the iterable, not the element**~~
  *(HIGH; CLOSED iter 349)*. PHP foreach now uses `<right>` for the
  iterable and `<left>` for the binding, matching TS/C#/Python's
  `for x in items` shape. Cold-read finding closed.

  Java and Ruby `for ... in` STILL use `<value>` for iterable —
  same divergence pattern. Separate iters (each has its own Custom
  handler / rule path). Follow-up:
  - Java `enhanced_for_statement` Custom handler.
  - Ruby `for` Custom handler — also has the `"in"` text leaf inside
    `<value>` slot (`<value>"in"<expression>...</expression></value>`)
    which complicates the rename.

- [ ] **Java: `<call[this]>/null` shape for `this(null)` lacks
  `<argument>` wrapper** *(MEDIUM)*. The argument floats as a sibling
  without the wrapper that other call sites use (`call[base]/argument/int=...`).
  Reproduce: `tests/integration/languages/java/blueprint.java.snapshot.txt:67`.

- [ ] **Ruby: `member[static]` chain has no object slot** *(MEDIUM,
  principle #5 cross-language; iter 343 attempted, reverted —
  needs deeper design)*. `Configuration::Defaults` renders as
  `member[static]/member[static]/name=Configuration/name=Defaults` —
  no `<object>` or `<member>` slot. Other languages wrap in
  `path/`, `<object[access]>`, etc.
  Reproduce: `tests/integration/languages/ruby/blueprint.rb.snapshot.txt:549-551`.

  **Iter 343 attempt notes**: tried mirroring PHP's
  `class_constant_access` Custom handler (wrap LHS in `<object>`,
  RHS in `<property>`, then chain inversion runs). Worked for
  the 2-element-child case (`A::B::C` chain-inverts cleanly to
  `<object[access]>/<name>A</name>/<member[static]>/<name>B</name>
  </member>/<member[static]>/<name>C</name></member></object>`).

  BUT broke for the 1-element-child top-level case `::Configuration`
  (Ruby blueprint line 172 has `::Configuration::Defaults`):
  - Tree-sitter emits `ScopeResolution(ScopeResolution(Configuration), Defaults)`.
  - Inner ScopeResolution has only 1 element child (no LHS).
  - Whatever the handler emits for the 1-child case (bare
    `<member[static]><name>Foo</name></member>` OR
    `<member[static]><property><name>Foo</name></property></member>`),
    chain inversion mishandles it: when the OUTER member wraps the
    inner as its `<object>`, chain inversion treats the inner as a
    chain step (because it has `<member>` element name) AND the
    outer as a step → produces TWO `<member[static]>` siblings under
    `<object[access]>` instead of nested chain steps. Triggers
    `no-children-overflow` ratchet (count 13 → 14).

  **Design challenge**: top-level `::Foo` form has no receiver.
  Should it render as bare `<name>Foo</name>` (lose the static
  marker), as `<member[static]>` with no slots (current), or as
  `<object[access]>` with just one child (synthesizes the chain
  shape but with degenerate receiver)? Each has tradeoffs. Iter
  343 reverted; needs subagent design review before retry.

- [ ] **Rust: `pattern[or]/` flattens variants and bindings
  together** *(MEDIUM)*. `Shape::Dot(0, y) | Shape::Dot(y, 0)` arm
  renders as `pattern[or]/path/.../int=0/.../name=y/.../path/.../name=y/int=0` flat —
  both alternatives merged into one sibling list. Cannot tell where
  one pattern ends and the next begins.
  Reproduce: `tests/integration/languages/rust/blueprint.rs.snapshot.txt:455-466, 469-475`.

- [ ] **Rust: match-arm guard `condition/` sibling of pattern
  parts** *(MEDIUM, cross-language)*. `arm/pattern/path.../name=x/name=y/condition/.../`
  inlines guard with pattern bindings. Java uses
  `label/.../guard/...`, C# uses sibling `when/`. Three different
  homes for "switch arm guard."
  Reproduce: `tests/integration/languages/rust/blueprint.rs.snapshot.txt:469-478, 491-494`.

- [ ] **C#: switch `arm` vs `section[break]` duality** *(MEDIUM)*.
  Switch-expression uses `arm/`; switch-statement uses
  `section[break]/`. Different element names for what users treat as
  "switch case." Java uses `arm/` for both. Also: `section[break] =
  "default: break"` mixes "default case" marker with "fallthrough
  with break."
  Reproduce: `tests/integration/languages/csharp/blueprint.cs.snapshot.txt:438-460 vs 466-483`.

- [ ] **Java: switch arm guard inside `label/` not named slot**
  *(MEDIUM)*. `arm/label/pattern/.../guard/...` hides guard under
  `label/`. C# uses sibling `when/`, Rust nests `condition/`.
  Three different homes again.
  Reproduce: `tests/integration/languages/java/blueprint.java.snapshot.txt:191-199`.

- [x] ~~**PHP `member[constant]` divergent from plain `member`**~~
  *(LOW-MEDIUM; CLOSED iter 342)*. Renamed PHP's `<member[constant]>`
  to `<member[static]>` per Principle #5 alignment with Ruby's
  equivalent shape (and PHP's own `<call><static/>` for static
  method calls). The marker now describes the scope-resolution
  operator uniformly. Pinned by
  `cross_language_static_member_access_marker` in members.rs.

- [ ] **PHP `foreach pair/` shape misleading** *(MEDIUM)*. PHP's
  `key=>value` foreach binding uses `pair/`, which is the same name
  used for hashmap entries everywhere. `//pair` returns both.
  Reproduce: `tests/integration/languages/php/blueprint.php.snapshot.txt:188-190`.

- [ ] **Python `match` arm guard `compare/` floats unwrapped**
  *(MEDIUM)*. `arm/pattern/.../compare/.../then/...` — the guard sits
  as a sibling without a role wrapper. Querying "match arm guard"
  requires knowing it's the second-to-last child or specifically a
  `compare`.
  Reproduce: `tests/integration/languages/python/blueprint.py.snapshot.txt:401-405`.

- [ ] **Cross-language import shape divergence** *(MEDIUM, known)*.
  Six languages, 4+ shapes for "import this thing from this place."
  Java/C# put the imported symbol as the last `name` in `path/`;
  PHP/Rust put it as a sibling outside `path/`; Go renders as a
  string. Already partially-tracked in older backlog items; cold-read
  re-confirms the divergence is still present.

- [x] ~~**Python/Go/Ruby plain `=` assign lacks `<op>` wrapper**~~
  *(MEDIUM, Principle #5 cross-language; surfaced iter 339 mid-loop
  while writing cross-lang assign tests; CLOSED iter 340-341)*.
  Go/Ruby shipped iter 340 (`Rename(Assign)` → `ExtractOpThenRename(Assign)`).
  Python shipped iter 341 via Custom handler that explicitly extracts
  `=` (skipping `:` in annotated assigns). All 8 chain-inverting
  languages now uniform; pinned by
  `cross_language_plain_assign_extracts_op_equals` in operators.rs.
  Original divergence text below:

  **Source-preservation constraint** (user-confirmed iter 340): the
  fix must NOT duplicate the `=` text. The existing `<op>=</op>`
  shape is just the bare `=` text leaf WRAPPED in an `<op>` element —
  exactly one text node containing `=`. Verified iter 340: C#
  produces `<op>=</op>` (single text node); Python/Go/Ruby produce
  bare `=` text leaf (single text node) inside `<assign>`. The fix
  for Python/Go/Ruby should similarly wrap the existing bare `=`
  text leaf in `<op>` — no duplication. Source reconstruction
  (concatenating leaf text nodes) must continue to produce the
  original source.

  Affected: Python's `assign` Custom handler (or Rule mapping);
  Go's `assign` rule; Ruby's `assign` rule. Each needs operator
  extraction extended to plain `=` (currently only augmented
  `+=`/`*=`/etc. extract).

  Reproduce: `echo 'x = 5' | tractor --lang python -d 6 -x "//assign"`
  shows `"="` text leaf instead of `<op>="="</op>` wrapper. Same
  for Go (`x = 10`) and Ruby (`x = 5`).

  Sizing: 1 iter per language (3 iters total) OR 1 iter with
  shared operator-marker plumbing if a common helper can extend
  the extraction. Subagent design review before implementing —
  the assign Custom handlers may want to share with the
  augmented-assign extraction logic; landing 3 languages at once
  is a snapshot-diff churn concern.

  Cross-lang test pinning the contract once fixed:
  `cross_language_plain_assign_extracts_op_marker` in operators.rs
  (similar to iter 334's `cross_language_binary_plus_extracts_op_marker`).

**Sections noted clean** by the cold-read:
- Most singleton role-slots (`condition`, `then`, `else`, `left`,
  `right`, `value`) correctly collapse to JSON objects (no spurious
  singleton arrays on intrinsic singletons).
- Operator markers (`op[...]`) uniform across all 9 languages.
- `<comment[leading]>` / `<comment[trailing]>` placement consistent.
- Loop family (`for`/`while`/`do`) largely uniform.
- Go and TS member-access alignment strong outside the TS shape
  divergence above.

### Iter 292 — shape contracts surfaced 4 tsql sites

Phase 1 of the transform-validation architecture
(`docs/transform-validation-architecture.md`) shipped iter 292 with
the `no-children-overflow` rule running against ALL blueprints
(previous audit only counted main 8 languages via JSON grep). The
new tooling reports **13 advisory occurrences** total: 9 main-language
sites (already known/deferred) + **4 tsql sites not previously
tracked**:

- [ ] `tsql/blueprint.sql:82` — `<statement>` (MERGE) has 2 untagged
  `<when>` children. Role-uniform (each WHEN clause is one branch),
  list-tag-able. Narrow fix candidate, BUT parent name `<statement>`
  is the generic catch-all rename and may over-match other statement
  kinds with WHEN. Verify scope before adding `("statement", "when")`
  to a tsql `tag_multi_role_children` call. **Pre-fix question**: do
  CASE / IF / other tsql statements also produce `<statement>` with
  `<when>` children, or is MERGE the only one? Custom handler scoped
  to `merge_statement` may be safer than generic role tag.
- [ ] `tsql/blueprint.sql:82` — same `<statement>` has 2 untagged
  `<ref>` and 2 untagged `<alias>` children (Target+t / Source+s).
  Role-mixed (target vs source position); proper fix is slot
  wrappers (e.g. `<into>` / `<using>` analogous to MERGE keywords).
  Larger design call.
- [ ] `tsql/blueprint.sql:85` — `<when>` has 2 untagged `<list>`
  children (column-list `(ID, Name)` and values-list `(s.ID, s.Name)`
  in `WHEN NOT MATCHED THEN INSERT ...`). Role-mixed (columns vs
  values). Slot wrappers `<columns>` / `<values>` would disambiguate;
  larger design call.

Folds into the existing "TSQL `<from>` / `<case>` / `<call>`
overflow" backlog item (severity LOW, ~6 sites).

### Iter 290 wind-down — children-overflow audit natural pause

**Status (post-iter-290): 9 sites remaining across all 8 languages** —
csharp 1, java 1, python 2, typescript 1, rust 3, go 1, php 0, ruby 0.
86% reduction from the iter-181 baseline of 64 sites. PHP and Ruby at
zero. Six other languages at 1-3 sites each.

**Iters 288-290 contribution** (4 sites closed):
- iter 288: Go `slice_expression` bounds wrap → `<from>`/`<to>`/
  `<capacity>` slot wrappers, mirrors Rust ranges (iter 270) /
  Ruby ranges (iter 180). Go 2 → 1.
- iter 289: Python `pattern[dict]` values list-tag — added
  `("pattern", "value")` to Python's `tag_multi_role_children`.
  Python 3 → 2.
- iter 290: Rust assoc-type binding (`Drawable<Canvas = Vec<u8>>`)
  reuses `<type[associated]>` shape from declaration site. Rust
  5 → 3 (two sites).

**The remaining 9 sites are all in deferred design-call classes:**
- Chain-receiver-call collision (java 1: `getClass().getSimpleName()`,
  ts 1: `String(input).split("")`, go 1: `Sprintf` args). Needs
  cross-language design decision on call argument vs receiver
  shape.
- Attribute multi-args (rust 3: `#[allow(...)]`, `#[derive(...)]`).
  Needs design decision on whether attribute arguments form a
  uniform list with role-named slots or stay flat.
- Dict-spread marker-name shadow (python 2: `{**a, **b}` where the
  `<spread>` carries `[dict]` marker AND a `<dict>` content child).
  Needs marker name disambiguation.
- C# anonymous object initializer (csharp 1: `new { Name = "foo",
  Value = 42 }`). Each member needs slot wrapping (`<pair>`,
  `<assign>`, or `<member>`); design call.

**Loop status**: paused on overflow audit. Resume on user direction
for any of the deferred design classes. The audit metric `"children":
[` count is steady at 9; stable enough that future iters can pivot
to other backlog work without churning the audit.

### Iter 258 user-flagged

- [x] iter 259 — **Rust `expression/expression[try]` repeated
  parent-child name fixed.** Applied the same "parent-is-expression
  → lift marker + flatten" pattern that C#'s `await_expression`
  already used (transformations.rs:207). Both Rust `try_expression`
  and `await_expression` now lift their marker onto an existing
  `<expression>` parent rather than emit a nested second host.
  Pinned by `transform/errors.rs::rust_try_postfix_no_double_host`
  (asserts `//body/expression[try]/call` matches and
  `//expression/expression` does NOT match). Blueprint snapshots
  unchanged (the only Rust try in the blueprint sat under `<value>`,
  not `<expression>`).

### Cold-read findings (iter 233 review)

- [x] iter 260 — **Python `from`-import singleton/plural shape
  inconsistency fixed.** New helper
  `python_tag_from_imports_uniform` runs after
  `python_restructure_imports`; tags every `<import>` child of
  `<from>` with `list="imports"` regardless of cardinality. Both
  `from collections import OrderedDict` (single) and
  `from typing import Optional, Union` (multi) now render as
  `imports: [...]` in JSON. The `(from, import)` pair was removed
  from the cardinality-gated `tag_multi_role_children` call to
  avoid double-tagging. Pinned by
  `transform/imports.rs::python_from_import_list_attr_uniform`
  (asserts `//from/import[@list='imports']` matches both single
  and multi cases).

- [x] **`else_if` element name** — REJECTED. Iter 252 attempted
  to rename `<else_if>` → `<elseif>` across 8 languages on the
  cold-read reviewer's "Principle #2 violation" flag, then
  reverted (commit 85e58e29) after user flagged the design
  carve-out. `specs/.../design.md` § 17 (Avoid Compound Node
  Names) explicitly cites `else_if` as the canonical allowed
  exception: "the concept is genuinely the *combination* of two
  keywords and neither half alone names it. Rare; expect to
  justify each one individually." The cold-read reviewer (iter
  233) missed this carve-out when flagging.
  Lesson: before acting on a "Principle X violation" flag from
  the cold-read reviewer, search design.md for explicit
  allowed-exception citations naming the flagged element.
  **Do not re-flag this item.**

- [x] iter 262 — **Go `body[return]` marker semantics** —
  REJECTED (cold-read misread). Verified with
  `//body/return` against blueprint: 11 matches including the
  bare-return sites at Sum/Uniq/variadic. The `<return/>` element
  IS structural (an empty element child of `<body>`); the
  tree-text bracket notation (`body[return]/for/...`) just
  collapses empty children into marker-style display, but the
  structural element is present and queryable. The cold-read's
  claim "structurally invisible" was wrong. JSON does serialize
  `<return/>` as `"return": true` (boolean key, like a marker),
  but that's a general empty-element-vs-marker semantic that
  applies to every empty-element statement (`<break/>`,
  `<continue/>`, `<fallthrough/>`) — not specific to return.
  No fix needed.

- [x] iter 263 — **Java/C# single-declarator fields and locals
  flatten the `<declarator>` wrapper.** New shared post-pass
  `flatten_single_declarator_children(xot, root, &["field",
  "variable"])` runs in both `java_post_transform` and
  `csharp_post_transform`. Lifts the children of a single
  `<declarator>` up into the parent so `int x = 1;` produces
  `field/{type, name, value}` instead of
  `field/{type, declarator/{name, value}}`. Multi-declarator
  parents (`int a, b = 5, c`) keep the wrappers because each
  `<declarator>` is a role-mixed name+value group whose
  pairing depends on the wrapper. Subagent review (general-
  purpose, AMEND-then-ship): originally proposed cross-language
  flat-with-`list=` for multi-declarator too, but that loses
  pairing for mixed-init multi-declarator (`int a, b = 5, c`)
  per Principle #19 — kept conservative single-only flatten.
  TS's existing unconditional Flatten remains a separate gap
  (logged below). Tests: new `java_single_declarator_flattens`
  + `csharp_single_declarator_flattens` in transform/variables.rs;
  the existing `java` multi-declarator test still pins the
  wrapper-kept behavior for `int x = 1, y = 2`. Updated 7
  cross-cutting tests (comments / generics / literals / loops /
  modifiers / operators / types) that referenced
  `[declarator/name='X']` to use the flat `[name='X']` shape.
  Snapshot diff: ~600 lines across Java + C# blueprints (every
  single-declarator field/variable lost its wrapper).

- [x] iter 264 — **TypeScript multi-declarator binding loss
  fixed.** Switched TS `VariableDeclarator` from unconditional
  `Flatten` to `Rename(Declarator)`; added
  `flatten_single_declarator_children` to TS post_transform
  (same shared helper as iter 263). Multi-declarator
  (`let i = 0, j = 100`) now keeps both `<declarator>` wrappers
  with internal name↔value binding. Added `(variable, declarator)`
  and `(field, declarator)` to TS / Java / C#
  `tag_multi_role_children` calls so multi cases also get
  `list="declarators"` and JSON renders as `declarators: [...]`
  array. Pre-iter-264 JSON shape was broken: `children: ["j",
  value]` overflow + `name=i, value=0` singleton collision.
  Now: clean `declarators: [{name=i, value=0}, {name=j,
  value=100}]`. Pinned by
  `transform/variables.rs::typescript_multi_declarator_keeps_wrappers`.
  Snapshot diff: 12 lines in TS `.txt` + JSON shift around
  the for-loop multi-binding site.

- [x] iter 253 — **Python `yield from` keyword erasure** —
  Custom `yield_expression` handler detects "from" in the text
  leak and prepends a `<from/>` marker on `<yield>`. Result:
  `yield from range(n)` → `yield[from]/call/...` queryable as
  `//yield[from]` distinct from `//yield[not(from)]`.
  `From` enum variant promoted to dual-use (already a container
  for from-imports; now also a marker for yield-from).

- [x] **Python `match` wildcard `pattern = "_"` as bare text leaf** —
  closed iter 234. New `case_pattern` Custom handler (replaces
  `Rename(Pattern)`) detects element-children-empty + trimmed-text-
  is-`_` and emits `<pattern[wildcard]/>` (empty marker, joining
  `pattern[tuple]` / `pattern[list]` / `pattern[union]` /
  `pattern[splat]` / `pattern[dict]` / `pattern[complex]` family).
  JSON: `"pattern": "_"` → `{"pattern": {"wildcard": true}}`.
  2 fixture sites; 829 tests green.

- [x] iter 262 — **TypeScript `body[break]` switch-case marker** —
  REJECTED (cold-read misread, same as Go `body[return]`). Verified
  `//body/break` matches both case sites; `<break/>` IS a structural
  empty element child of `<body>`. The tree-text bracket notation
  `body[break]` just collapses empty children for display. JSON
  ambiguity (boolean key for empty element) applies to every empty
  statement keyword, not just break. No fix needed.

- [ ] **Cross-language bare-keyword statement shape inconsistency**
  *(Principle #5, severity LOW, surfaced iter 262)*
  - Python: `body/return = "return"` (`<return>` carries the
    keyword as text content)
  - Go: `body/<return/>` (empty element)
  - TypeScript: `body/<block><return/></block>` (empty element
    inside a block wrapper)
  - Three shapes for the same conceptual statement ("bare
    keyword statement at end of block"). Same applies to bare
    `break`, `continue`, etc.
  - Investigation needed: which shape is canonical? Probably the
    Python one (text content distinguishes statement from marker
    in JSON) — but check whether `<return>return</return>` adds
    value or just noise. Also affects: which empty-element
    statements in JSON look identical to markers and need
    disambiguation.
  - Effort: 1-iter design + 1-iter implementation.

- [x] iter 261 — **Go for-while gains `<condition>` wrapper.**
  New `for_statement` Custom handler detects "while-form" (no
  `for_clause`/`range_clause` child) and wraps the bare expression
  child in `<condition>`; the shared `wrap_expression_positions`
  pass adds the inner `<expression>` host. Now both `for n < 3 {}`
  and `for i := 0; i < 3; i++ {}` produce
  `for/condition/expression/binary/...` — within-Go Principle #5
  satisfied. Cross-language verified against Java/Python/TS/Rust
  `while` shapes (all use `<condition>/<expression>`). Reviewer
  confirmed ship via `general-purpose` agent. Pinned by
  `transform/loops.rs::go` (added while-form claim, bumped
  expected count from 3 to 4 fors).

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

- [x] iter 265 — **Java method-call shape divergence** —
  RESOLVED by chain inversion. The pre-iter-239 divergence
  (Java flat, Python/Go nested-member, TS callee+member) is
  obsolete: chain inversion (iters 239-248) unified all
  call-chain shapes to canonical `<object[access]>` with
  receiver-first, nested step spine. Verified `obj.method("x")`
  produces identical `<object[access]><name>obj</name><call>
  <name>method</name><string>"x"</string></call></object>`
  across Java, Python, Go, TypeScript. The cross-language
  query `//object[access]/name='obj'/call[name='method']`
  works uniformly.

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

- [x] iter 265 — **Ruby `obj.method(arg)` 4th call-shape variant** —
  RESOLVED by chain inversion (iter 246 Ruby pilot). Verified
  `obj.method(x)` produces canonical `<object[access]>/<name>obj
  </name>/<call>{name=method, name=x}</call>` matching
  Java/Python/Go/TS. The "4th variant" was the pre-iter-246 flat
  `call[optional]/<name><name>` shape which the chain inverter
  consumed.

### Findings from iter-175 post-cycle review (iters 170-174 cluster)

Surfaced once the cleaner post-iter-171 JSON snapshots became readable.

- [x] **C# member-access role-wrap** — closed iter 178. Mechanical
  port of iter 147 (Java/Python/Go) to C# `MemberAccessExpression`.
  Custom handler tags receiver (field=expression) as `<object>` and
  property as `<property>`. Tracker-test xpaths updated. Rust
  `<field_expression>` still pending (separate item — verify if it
  has the same shape gap).

- [x] **Go closure body archetype** — closed iter 176. Post-pass
  `go_retag_singleton_closure_body` retags `<body>` to `<value>` for
  single-statement closures and strips stray `{`/`}` text leaves;
  multi-stmt bodies keep `<body>`. Closes the iter-174 missed-language
  gap. Added `multiStmt` blueprint fixture for the multi-stmt case.

- [x] **Ruby ternary** — closed iter 179. Custom handler
  `conditional` renames to `<ternary>` (cross-language Principle #5;
  matches C#/Java/Python/PHP/TS) AND wraps the consequence/alternative
  arms in `<then>`/`<else>` slots. Custom handler (not field-wrap)
  because the broad `("alternative", "else")` field-wrap entry would
  also wrap if/elsif chain alternatives, breaking
  `collapse_else_if_chain`. JSON now: `ternary.condition`,
  `ternary.then`, `ternary.else` — uniform with other PLs; no more
  `symbol`/`children` collision.

- [x] **Ruby `range`** — closed iter 180. Custom handler `range`
  inspects text leaves to detect `..` (inclusive) vs `...` (exclusive),
  prepends the corresponding marker, and wraps `field="begin"` /
  `field="end"` children in `<from>` / `<to>` slots. Open-ended
  ranges (`1..`, `..9`) handled naturally (the absent field
  produces no wrapper). JSON: `{range: {inclusive: true, from:
  {int: "1"}, to: {int: "9"}}}` — Principle #8 (round-trip)
  satisfied; role-mix resolved.

- [x] **Ruby `case`/`when`/`pattern` list distribution** — closed
  iter 177. New post-pass `ruby_tag_case_when_lists` tags
  `<case>`'s `<when>` children with `list="when"` and `<when>`'s
  `<pattern>` children with `list="pattern"`. Targeted (not bulk
  via `distribute_member_list_attrs`) because case/when have
  role-mixed children — `<value>` discriminant and `<else>` are
  singleton, only `<when>`/`<pattern>` repeat. Result: JSON
  `case.when: [...]` array (was: 1 lifted, rest in `children:`),
  `when.pattern: [...]` array for multi-pattern `when X, Y`.

- [ ] **C# accessor heterogeneous JSON array** *(severity LOW)*.
  Site: `tests/integration/languages/csharp/blueprint.cs.snapshot.json:181-188`.
  Current: `accessors: ["get;", {$type: "set", protected: true,
  text: "protected set;"}]` — mixes string and object in same
  array; consumers must type-switch. Acceptable per #13 (text-leaf
  vs complex), worth noting. May require always-wrap-in-object
  if uniformity matters more than text-only-leaf compactness.

- [x] **Cross-language role-mix audit** — closed iter 181.
  Swept `"children": [` across all 9 `.snapshot.json` blueprints.
  283 occurrences total; per-language: rust 50, python 48, ruby 42,
  go 40, tsql 34, ts 28, csharp 18, php 18, java 5. Findings split
  into top patterns below; remaining items spawned as fresh
  backlog entries.

### Cross-language role-mix audit findings (iter 181)

**Top causes by language:**

- [x] **TSQL post-transform partial** — closed iter 182. Added
  `tsql_post_transform` with `distribute_member_list_attrs` for
  role-uniform containers (`file`, `transaction`, `union`,
  `columns`, `list`). Closed 8 of 34 TSQL overflow sites
  (34 → 26). Remaining 26 are role-MIXED parents (`select`,
  `insert`, `from`, `case`, `compare`, `between`, `assign`)
  needing targeted handlers — see new entry below.

- [x] **TSQL `<select>` / `<insert>` column lists** — closed iter
  185 partial. Targeted post-pass `tsql_tag_select_columns` adds
  `list="column"` to `<column>` children of `<select>`/`<insert>`.
  Closed 6 of remaining 26 TSQL sites (26 → 20). See follow-up
  for binary-operand wrapping.

- [ ] **TSQL `<compare>` / `<assign>` / `<between>` operand
  wrapping** *(severity MED, ~14 sites)* — TSQL binary expressions
  produce `<compare><column>...</column><op>...</op><column>...</column></compare>`.
  Tree-sitter DOES emit `field="left"` / `field="right"` on the
  operands (verified via `--meta -t raw`), and TSQL's
  `field_wrappings` is COMMON_FIELD_WRAPPINGS (which includes
  left/right). Yet the operand `<column>`s don't get wrapped in
  `<left>`/`<right>` — Java/etc. with the same config DO get
  wrapped. Root cause unknown (deferred for investigation).
  Workaround: targeted post-pass that re-wraps. Effort: medium
  (root-cause investigation) OR small (workaround). Sites:
  remaining 20 in tsql.

- [ ] **TSQL `<from>` / `<case>` / `<call>` overflow** *(severity
  LOW, ~6 sites)*. Smaller patterns; investigate per-parent.

- [x] **Cross-language `<alias>` marker/wrapper rename** — closed
  iter 184. Found across Rust/TS/Python/PHP (not just Rust): the
  dual-use `<alias>` (marker) + `<alias>` (structural binding wrapper)
  collided on the JSON `alias` key, lifting one and overflowing the
  other. Renamed the WRAPPER to `<aliased>` (kept marker `<alias/>`
  so existing `//import[alias]` queries still work). 7 wrapper-
  construction sites in mod.rs renamed. Spec doc updated. Closes
  3 Rust + 2 Python + 1 TS + 1 PHP audit sites (-7 total).

- [x] **Ruby `<match>` with multiple `<in>` clauses** — closed
  iter 183. Extended `ruby_tag_case_when_lists` to also tag
  `<in>` children of `<match>` with `list="in"`. JSON now:
  `match.in: [{...}, {...}]` array (was `match.children: [...]`
  overflow).

- [ ] **Rust/Python/PHP `<parameter>` overflow** — multiple
  parameters in some contexts (typeparams? closures?) overflow.
  Need to investigate which parent and why. Could be the same
  root as TSQL (missing distribute_member_list_attrs config) or
  a per-parent handler issue. Effort: medium (investigation +
  fix). Sample sites: rust 5, python 1, php 2.

- [ ] **C# `<arm>` / `<argument>` overflow** — small, likely
  isolated. C# arms (switch arms) and arguments may need list=
  distribution in specific positions. Effort: small.

- [ ] **TypeScript `<type>`/`<parameter>`/`<import>`/`<generic>`
  isolated overflow** — 4 sites, 1 each. Likely independent fixes
  per site. Effort: small (per-site).

- [ ] **Python `<type>` overflow (2 sites)** — possibly typeparams
  or generic-arg lists. Effort: small.

**Summary**:
- TSQL post-transform addition would close 34 sites in one iter.
- Each remaining language has 1-5 specific patterns to address
  individually.
- After the post-iter-171/172 cleanup, role-collisions are now
  trivially auditable via JSON `"children": [` grep — keep this in
  the post-cycle review checklist.

### Iter 230 wind-down checkpoint

**Audit progress** (set baseline at iter 181):
- Baseline: 283 children-overflow sites cross-language.
- Current (iter 230): 63 sites (-78%).
- Per-language: csharp 6, ts 5, rust 17, java 3, go 11, python 12,
  php 5, ruby 2, tsql 2.
- 46 iters since audit baseline (181-230).
- 829 tests + 149 fixtures continuously green throughout.

**Patterns successfully addressed** (cumulative across the audit
cluster):
- Cross-language `<alias>` wrapper rename to `<aliased>` (iter 184).
- Closure body archetype unification across all 8 PLs (iters 161-176).
- Ruby case/when/match list distribution (iters 177, 183).
- Ruby ternary, range, lambda outer-body collapse (iters 179, 180, 174).
- C# member-access role-wrap (iter 178); PHP class-constant (iter 214).
- Multiple cross-language `tag_multi_*_children` helpers (iters 190,
  191, 194, 196).
- TSQL post-transform addition + binary operand wrap (iters 182, 199, 211, 221).
- Per-language Custom handlers for Python ternary, Rust self_param,
  Go closure, etc.
- Bulk `distribute_member_list_attrs` extension to literal collections
  (iter 205) and follow-on container types (iters 206-212).
- Critical mid-course correction iter 213: reverted role-mixed
  parents from bulk distribute (was creating ~2,389 1-element arrays
  in JSON; reviewer caught the regression).

**Remaining 63 sites — what's left**:
- Heterogeneous-content patterns (Rust pattern variants with bare
  text leaves, Python class/decorator structure, Java switch-label
  patterns) — need per-construct Custom handlers with specific
  knowledge of tree-sitter shape.
- Role-mixed parents that need targeted role-named slot wrappers
  (PHP foreach iterable+binding, Go index operand+indices, etc.) —
  cross-language design call: `<iterable>/<binding>` vs
  `<value>/<as>` vs other.
- Singletons that the bulk approach didn't reach because parent is
  context-dependent (e.g. `<type>` is sometimes wrapper, sometimes
  container).

**Loop status**: paused at iter 230. Per-iter wins now ~1 site;
remaining work is non-mechanical and benefits from user direction.
Resume on explicit ask or when new findings/feedback land.

---

### Findings from iter-186 post-cycle review (iters 175-185 cluster)

- [x] **Rust closure multi-param overflow** — closed iter 187.
  `closure_parameters` Custom handler now tags each wrapped
  `<parameter>` with `list="parameters"` after the wrap step (was
  missing — handler returned `Flatten` which promotes children but
  didn't distribute list=). JSON: `closure.parameters: [{...}, {...}]`
  uniform array (was: 1 lifted singleton + rest in children
  overflow).

- [ ] **Investigate Rust/Python `<pattern>` overflow** *(severity
  LOW-MED)*. Pattern-binding sites overflow consistently in
  multiple JSON parents. Cite specific paths in next iter that
  picks this up.

### Findings from iter-197 post-cycle review (iters 186-196 cluster)

Audit progress: **283 → 173 (-39%)** since baseline. iters 186-196
cluster contributed -88 sites. Per-language: csharp 18 (0), ts 27
→ 15 (-12), rust 47 → 34 (-13), java 5 (0), go 40 → 20 (-20),
python 46 → 30 (-16), php 17 → 14 (-3), ruby 41 → 17 (-24), tsql
20 (0).

- [x] **Delete `tag_multi_type_children` shim** — closed iter 198.
  Removed (8 lines) from transform/mod.rs; all callers use
  `tag_multi_same_name_children` directly.

- [x] **TSQL operand re-wrap** — closed iter 199. New post-pass
  `tsql_wrap_binary_operands` walks `<compare>`/`<assign>`/`<between>`
  parents and wraps children with `field="left"`/`field="right"`
  attributes in `<left>`/`<right>` elements. Closed 9 of TSQL's
  remaining 20 audit sites. Iter-197's identified mystery (intentional
  Skip dispatch in `tsql/transform.rs:29` strips builder wrappers)
  doesn't need to change — operands kept their `field=` attributes
  so re-wrapping is local + minimally invasive.

- [ ] **Document the TSQL Skip-by-design choice in design.md**
  *(severity LOW, small)*. The trap "field-wrap config identical to
  Java but doesn't fire" deserves a paragraph so new contributors
  don't repeat the iter-185 investigation. Tag with the iter-197
  finding for traceability.

### Standing items (re-flag every cycle)

- [ ] Snapshot cold-read pass every ~5 cycles — fresh eyes on every
  blueprint, surface anything suspicious that familiarity has hidden.
- [ ] When a transform test needs `(A or B)` disjunction or
  descendant-axis fallback, log as a candidate for re-shaping the
  underlying tree.

## Addressed

(Most-recent first. Older addressed items may be pruned periodically.)

- [x] iter 292: **transform-validation phase 1 (layers 1+2).** Lands
  shared predicate engine in `tractor/src/transform/shape_contracts.rs`
  consumed by two invocation sites: cargo integration test
  (`tractor/tests/shape_contracts.rs`, layer 1) iterating every
  blueprint, plus debug-build assertion in
  `transform/builder.rs:545` (layer 2) running every transform call.
  Three starter rules: `no-children-overflow` (Advisory, 13 sites),
  `no-marker-wrapper-collision` (Error, 0 sites — pinned),
  `name-is-text-leaf` (Error, 0 sites — mirrors existing invariant
  pending phase-2 migration). Architecture documented in
  `docs/transform-validation-architecture.md`. Per user direction
  during iter 291 wind-down: "this whole improvement layer deserves
  its own design document."
- [x] iter 291: backlog wind-down note — children-overflow audit
  natural pause at 9 main-language sites (86% reduction from 64
  baseline); remaining sites all in deferred design-call classes.
- [x] iter 290: Rust assoc-type binding reuses `<type[associated]>`.
  Rust 5 → 3 overflow.
- [x] iter 289: Python dict-pattern values list-tag. Python 3 → 2.
- [x] iter 288: Go slice_expression bounds wrap. Go 2 → 1.
- [x] iter 231: pluralize all list= attribute values — added
  `pluralize_list_name(name)` helper with rules (consonant-y→ies,
  s/x/z/ch/sh→es, default+s) plus an already-plural override list.
  Routed all four dynamic helpers through it
  (`distribute_member_list_attrs`, `tag_multi_role_children`,
  `tag_multi_same_name_children`, `tag_multi_target_expressions`,
  `flatten_nested_paths`). Updated singular `with_attr` sites in
  Ruby case/when/match/in helpers and TSQL select-columns. Fixed
  `xml_to_json::strip_top_level_type` to recognize `list=` as the
  plural of `element_name` (so $type-stripping kicks in for
  `methods: [<method>...]` after the rename — without this the
  pluralization would re-introduce $type redundancy on every list
  entry). Updated 11 JSON snapshots; net -156 lines (more $type
  stripped than singular keys lengthened). Per user directive
  "the final decision was to use plurals". 21 unit assertions in
  `test_pluralize_list_name`.
  from 283 baseline; -220 sites closed across 46 iters since
  iter 181). Remaining work is design-call territory or per-
  construct surgery. Loop paused; resume on user direction.
- [x] iter 228: Rust let-chain condition expression list= — added
  `("condition", "expression")` to Rust's tag_multi_role_children.
  `if let Some(x) = a && let Some(y) = b` produces `<condition>`
  with multiple `<expression>` siblings — now uniform array. Rust
  audit 20 → 17 (-3).
- [x] iter 227: TS group-import same-name children — added a
  SECOND `tag_multi_same_name_children(["import"])` call AFTER
  `typescript_restructure_import` (the early call ran before
  restructure created the group form). `import { a, b, c }` now
  produces `import.import: [{name:"a"}, {name:"b"}, {name:"c"}]`
  array. TS audit 6 → 5 (-1). Same fix pattern as Python iter-224.
- [x] iter 226: Go interface multi-method/type list= — added
  `("interface", "method")` and `("interface", "type")` to Go's
  tag_multi_role_children. Interfaces with multiple methods and/or
  type-set elements now render as uniform arrays. Go audit 12 → 11
  (-1).
- [x] iter 225: Python with/try multi-clause list= — added
  `("with", "value")` and `("try", "except")` to the post-restructure
  tag_multi_role_children call. Python audit 14 → 12 (-2).
- [x] iter 224: Python from-import multi-name list= — added
  `("from", "import")` to a SECOND tag_multi_role_children call
  AFTER `python_restructure_imports` (the first call ran before
  restructure rewired the `<from>` element). `from typing import
  Optional, Union` now produces `from.import: [{...}, {...}]`
  array. Python audit 17 → 14 (-3).
- [x] iter 223: Go multi-name var declaration list= — added
  `("var", "name")` to Go's tag_multi_role_children.
  `var x, y = 1, 2` now produces `var.name: ["x", "y"]` array.
  Go audit 13 → 12 (-1).
- [x] iter 222: Ruby destructured parameter multi-name list= —
  added `("parameter", "name")` to Ruby's tag_multi_role_children.
  `proc { |(x, y)| ... }` now produces `parameter.name: ["x", "y"]`
  array. Ruby audit 3 → 2 (-1).
- [x] iter 221: TSQL between low/high — extended
  `tsql_wrap_binary_operands` to also wrap `field="low"` /
  `field="high"` operands (in addition to left/right). `BETWEEN
  1 AND 10` now produces `between.low: {...}, between.high: {...}`
  role-named slots. TSQL audit 3 → 2 (-1).
- [x] iter 220: C# try/catch list= — added `("try", "catch")` to
  C#'s tag_multi_role_children. Try blocks with multiple catches
  now render `try.catch: [{...}, {...}]` array. C# audit 7 → 6
  (-1).
- [x] iter 219: Python ternary then/condition/else role wraps —
  rewrote `conditional_expression` Custom handler to position-wrap
  the 3 element children (then/condition/else) instead of relying
  on field-wrap (tree-sitter Python doesn't tag conditional_expression
  operands with field=). Cross-language Principle #5: matches Ruby
  (iter 179), C# / Java / PHP / TS ternary shape with role-named
  slots. Python audit 18 → 17 (-1 net; ternary closed but other
  patterns remain). Test xpath updated.
- [x] iter 218: Java method reference list= — added
  `("reference", "name")` to Java's tag_multi_role_children.
  `String::valueOf` (`<reference>` with class + method `<name>`s)
  now renders `reference.name: ["String", "valueOf"]` array.
  Java audit 4 → 3 (-1).
- [x] iter 217: TS object-literal pair list= — added `("object",
  "pair")` to TS's tag_multi_role_children. `{a: 1, b: 2}` now
  renders `object.pair: [{name:"a", value:...}, {name:"b", value:...}]`
  array. TS audit 8 → 6 (-2).
- [x] iter 216: Go multi-return type list= — added
  `("returns", "type")` to Go's tag_multi_role_children.
  `func f() (int, error)` now produces `returns.type:
  [{name:"int"}, {name:"error"}]` array. Go audit 15→13 (-2).
- [x] iter 215: TS function-type / object-type signatures targeted
  list= via `tag_multi_role_children(&[("type", "parameter"),
  ("type", "property")])`. TS audit 12 → 8 (-4). Properly scoped
  alternative to bulk-distributing `<type>` (which would tag the
  type-wrapper case wrongly).
- [x] iter 214: PHP class-constant access (`Foo::BAR`) Custom
  handler — wraps two `<name>` siblings in `<object>` /
  `<property>` (matching iter-178 C# member-access). Removed
  `object` from bulk distribute config (was over-tagging the
  iter-178/195/214 role wrappers' `<name>` child as 1-element
  array). PHP audit 9→5 (-4); TS 10→12 (+2 from removing object
  literal distribute); net -2. JSON: `member.object: {name: "Foo"},
  member.property: {name: "BAR"}` clean.
- [x] iter 213: post-cycle review of iters 205-212 + revert role-mixed
  parents from distribute config. Review found 5 silent reversals
  of prior single-element handler decisions (iters 178/179/180/195/
  212 — `<member>`, `<ternary>`, `<range>`, Ruby `<call>`, `<binary>`/
  `<unary>`/`<logical>`) AND ~2,389 new 1-element arrays. Removed
  22 role-mixed entries from distribute configs cross-language;
  kept 18 role-uniform: body/block/unit/namespace/section/import/
  file/program/module/tuple/list/dict/array/hash/object/switch/
  literal/macro/template/string/repetition. Children-overflow
  rebounded from 37 → ~89 (still down from 283 baseline by 69%);
  saved thousands of 1-element-array regressions. 2 new Lessons:
  bulk distribute trade-off; per-name whack-a-mole check on
  config sweeps.
- [x] iter 212: extend distribute — try/with/decorator/as/logical/binary/unary.
  Closed 2 sites: csharp 2→1, python 10→8. Total 39 → 37 (-87%
  from baseline). Diminishing returns suggest the bulk-distribute
  approach is plateauing.
- [x] iter 211: extend TSQL distribute config — `select`/`insert`/
  `from`/`call`/`case`/`constraint`. TSQL has its own
  `tsql_post_transform` distribute call (separate config), so
  iter-209/210 cross-language additions weren't applied. Closes
  TSQL 11→3 (-8). Total cross-language: 47 → 39 (-86% from
  baseline). Per-language: csharp 2, ts 5, rust 7, java 1, go 7,
  python 10, php 4, ruby 1, tsql 3.
- [x] iter 210: extend distribute config — call/index/repetition/insert/
  constraint/case/condition/range/then. Closed 14 sites: csharp 3→2,
  rust 14→7, go 12→7, php 5→4. Total 61 → 47 (-83% from baseline).
- [x] iter 209: extend distribute config — `type`/`member`/`select`/`from`
  containers cross-language. Closed 17 more sites: csharp 5→3, ts
  9→5, rust 16→14, go 13→12, python 13→10, php 9→5. Total
  78 → 61 (-78% from baseline). Per-language:
  csharp 3, ts 5, rust 14, java 1, go 12, python 10, php 5,
  ruby 1, tsql 11.
- [x] iter 208: extend distribute config — `object`/`template`/`returns`/
  `generator`/`string`. Closed 13 more sites: csharp 8→5, ts 12→9,
  go 15→13, python 14→13, ruby 5→1. Total 91 → 78
  (-72% from baseline). Per-language: csharp 5, ts 9, rust 16,
  java 1, go 13, python 13, php 9, ruby 1, tsql 11.
- [x] iter 207: extend distribute config to `switch`/`literal`/`macro`
  containers cross-language. Closed 15 more sites: csharp 13→8,
  rust 24→16, go 17→15. Total 106 → 91 (-68% from baseline).
- [x] iter 206: extend distribute config to `pattern` containers
  cross-language. Closes 18 more sites: csharp 15→13, ts 13→12,
  rust 30→24, java 4→1, python 18→14, ruby 7→5. Total 124 → 106
  (-62% from baseline). Java now down to 1 site!
- [x] iter 205: extend `distribute_member_list_attrs` to literal
  collections (tuple/list/dict/array/hash) cross-language. Single
  config change in 7 post-transforms; per Principle #12 the
  always-array shape is correct. Audit reductions: csharp 16→15,
  ts 15→13, rust 34→30, java 5→4, python 29→18, php 14→9,
  ruby 17→7. Total: 158 → 124 (-34, biggest single iter).
- [x] iter 204: Python `<compare>` name siblings list= — added
  `("compare", "name")` pair to Python's `tag_multi_role_children`.
  Tree-sitter `comparison_operator` doesn't tag operands with
  field=left/right (unlike binary_operator), so multi-name chains
  `a < b < c` overflowed. Python audit 30 → 29 (-1).
  Heterogeneous-operand compares (`obj.x < y`) still overflow —
  needs uniform `<expression>` wrapping or a different shape
  decision (deferred).
- [x] iter 203: C# qualified_name → `<path>` rename — closes the
  C# pattern of bare `<name>` siblings under `<new>` / `<using>` /
  etc. Mirrors Java's `ScopedIdentifier => Rename(Path)`. Also added
  `flatten_nested_paths` to C# post_transform. JSON now:
  `new.path.name: ["System", "IO", "MemoryStream"]`.
  C# audit 18 → 16 (-2). 1 transform-test xpath updated
  (`csharp_using_renames_to_import` adds `/path` step).
- [x] iter 202: Go shared-type parameter multi-name list= —
  extended Go's `tag_multi_role_children` to include `("parameter",
  "name")`. `func f(x, y int)` now renders
  `parameter.name: ["x", "y"]`. Audit 18 → 17 (-1).
- [x] iter 201: Go struct field multi-name list= — added
  `tag_multi_role_children(&[("field", "name")])` to Go's
  post_transform. Fields like `var x, y int` (multi-name with
  shared type) now render JSON `field.name: ["x", "y"]` array.
  Go audit 19 → 18 (-1 site). Other Go shapes (interface, body
  with if-elsif residue, parameters with shared type) need
  investigation per-pattern.
- [x] iter 200: Go alias→aliased rename — closes the iter-184
  cross-language rename gap (Go was missed). `go/transformations.rs`
  `import_spec` Custom handler at line 466 used
  `Alias.as_str()` for the wrapper element; now uses literal
  `"aliased"`. Added `Aliased` variant to Go's TractorNode enum.
  Go audit 20 → 19 (-1; only one aliased import in fixture).
  iter-184 was retroactively wrong about "all 4 languages" — Go
  had the same pattern in a different file.
- [x] iter 199: TSQL operand re-wrap — `tsql_wrap_binary_operands`
  post-pass wraps compare/assign/between operands with field= attr
  in `<left>`/`<right>` slots. Closed 9 sites (TSQL 20 → 11).
- [x] iter 198: delete `tag_multi_type_children` shim — 2-line
  redundant wrapper post-iter-194's generalization. -8 lines.
- [x] iter 197: post-cycle review of iters 186-196. Audit
  283 → 173 (-39%); 88 sites closed in this 11-iter cluster.
  TSQL field-wrap mystery SOLVED — intentional Skip dispatch in
  `tsql/transform.rs:29` strips wrappers; needs re-wrap post-pass
  (queued). 2 new Lessons added: (a) field-wrap Skip dispatcher
  trap, (b) call-out cross-language premature commitments in
  commit messages.
- [x] iter 196: Python f-string interpolation + string concat list= —
  added new helper `tag_multi_role_children` for (parent, child)
  pairs where the multi-instance child has a different element name
  from the parent. Wired Python with `("string", "interpolation")`
  pair. Also added "string" to the `tag_multi_same_name_children`
  names list cross-language for implicit string concatenation
  (`"a" "b" "c"` → `<string concatenated>` with `<string>` siblings).
  Python audit 32 → 30 (-2). Tuple/list overflow deferred (mixed-
  type heterogeneous lists need a different shape decision).
- [x] iter 195: Ruby `<call>` receiver wrap — extended
  `call_expression` Custom handler to wrap `field="receiver"` child
  in `<object>` (matches iter-148 Java method_invocation shape).
  Resolves the long-standing `call.name="X" + children=[method]`
  collision (iter-170 review found `MAX_RETRIES.zero?` overflow).
  Ruby audit 39 → 17 (-22, 56% reduction). Method-name `<name>`
  stays at top level so `call.name: "method"` is the JSON key.
- [x] iter 194: generalized helper + pattern-pattern coverage —
  promoted `tag_multi_type_children` to
  `tag_multi_same_name_children(names: &[&str])` covering any
  recursive container (parent name = child name = list= name).
  Wired to all 7 post_transforms with `["type", "pattern"]`.
  Rust audit 37 → 34 (-3); Python 35 → 32 (-3) from
  `<pattern>/<pattern>` overflow in match patterns.
- [x] iter 193: Python + C# + Java + Go wired to generic
  type/multi-target helpers. Python audit 41 → 35 (-6 from
  type-union pattern). C#/Java/Go: helpers idempotent + cardinality-
  gated, no behavior change (no overflow patterns of those shapes
  in their blueprints). Cumulative wiring: all 6 programming
  languages with `<type>` containers + `<left>/<right>` slots now
  pick up multi-instance role tagging automatically.
- [x] iter 192: Rust wired to generic helpers — added
  `tag_multi_target_expressions` + `tag_multi_type_children` to
  `rust_post_transform`. Rust audit 42 → 37 (-5). Remaining 37
  in: pattern (9), macro (4), condition (3), right (2), repetition
  (2), range (2), array (2), tuple (1) — each likely needs a
  separate Rust-specific handler or per-parent list= rule.
- [x] iter 191: cross-language type-union/intersection list= — new
  generic helper `tag_multi_type_children` tags `<type>` siblings
  under `<type>` parents (PHP `int|string`, `A&B`, `(A&B)|C`; TS
  `string | number`; etc.) with `list="type"`. Cardinality
  discriminator (>=2) keeps singleton-typed parameters untouched.
  PHP audit 17 → 14 (-3); TypeScript 27 → 15 (-12, biggest TS
  cluster closed). Wired into `php_post_transform` and
  `typescript_post_transform`.
- [x] iter 190: cross-language multi-target list= — generalized
  iter-189's helper (was Go-only) into
  `crate::transform::tag_multi_target_expressions` and wired into
  Python + Ruby post_transforms. Python audit 46 → 41 (-5); Ruby
  41 → 39 (-2). Same `<left>`/`<right>`-with-multiple-`<expression>`
  pattern fixes tuple unpacking (`a, b = xs`) and parallel
  assignment cross-language.
- [x] iter 189: Go multi-target left/right expression list — new
  `go_tag_multi_target_expressions` post-pass tags multiple
  `<expression>` siblings under `<left>`/`<right>` with
  `list="expression"`. Multi-target assignments (`x, y := 1, 2`)
  now render `left.expression: [{...}, {...}]` array instead of
  first-singleton-rest-overflow. Cardinality discriminator (>=2
  exprs) keeps singleton binary operands (`a + b`) untouched. Go
  audit 40 → 20 (-20, 50% reduction). Same pattern likely benefits
  Python tuple-unpacking (queued).
- [x] iter 188: Rust `self_parameter` Custom handler — strips the
  inner `<self>` element + `<mutable_specifier>` element + bare `&`
  text leaves; prepends `<self/>` (always), `<borrowed/>` (for
  `&self`/`&mut self`), and `<mut/>` (for `mut self`/`&mut self`)
  markers. Rust audit 46 → 42 (-4). All four self-form variants now
  clean: `{self: true}`, `{self: true, borrowed: true}`,
  `{self: true, mut: true}`, `{self: true, borrowed: true, mut: true}`.
- [x] iter 187: Rust closure multi-param `list="parameters"` —
  iter-186 review finding closed. `closure_parameters` Custom
  handler now sets list= on each wrapped `<parameter>` so JSON
  renders `closure.parameters: [...]` uniformly.
- [x] iter 186: post-cycle review of iters 175-185. Audit total
  283 → 261 (-22, mostly TSQL). Cluster overall PASS — alias
  rename verified, closure archetype intact across 8 PLs, no
  whack-a-mole regressions. New Lesson added (marker-name =
  wrapper-name → JSON key collision). Two new backlog items
  surfaced: Rust closure 2-param overflow + Rust/Python
  `<pattern>` overflow.
- [x] iter 185: TSQL `<select>` / `<insert>` column-list tagging —
  targeted post-pass adds `list="column"` to `<column>` children
  (skips role-mixed singleton clauses like `<from>`/`<where>`).
  Closed 6 of 26 remaining TSQL audit sites; binary-operand
  overflow (compare/assign/between) deferred — needs root-cause
  investigation into why TSQL's left/right field-wrap doesn't fire
  despite identical config to Java.
- [x] iter 184: cross-language `<alias>` wrapper renamed to
  `<aliased>` (Rust/TS/Python/PHP). Resolves the JSON-key collision
  between the `[alias]` marker and the structural binding wrapper.
  7 sites renamed in mod.rs; spec imports-grouping.md updated; 7
  audit overflow sites closed.
- [x] iter 183: Ruby `<match>`/`<in>` list distribution — extended
  iter-177's `ruby_tag_case_when_lists` to also tag `<in>` children
  of `<match>` with `list="in"`. JSON `match.in: [...]` array now
  uniform (was `match.children: [...]` overflow).
- [x] iter 182: TSQL post-transform — added
  `distribute_member_list_attrs(["file", "transaction", "union",
  "columns", "list"])` for role-uniform containers. Closed 8 of
  34 TSQL overflow sites (-24%); remaining 26 in role-mixed
  parents (select/insert/compare/etc.) need targeted handlers
  (queued as separate backlog item).
- [x] iter 181: cross-language role-mix audit — discovery iter (no
  code change). 283 `"children": [` overflow sites swept across 9
  JSON blueprints; categorized into 7 distinct patterns. Top
  finding: TSQL has `post_transform: None` so all 34 TSQL overflow
  sites trace to a missing distribute_member_list_attrs call (one
  iter would close 12% of the total).
- [x] iter 180: Ruby `<range>` — adds `[inclusive]`/`[exclusive]`
  marker (Principle #8: source must be reconstructable; `..` vs
  `...` are semantically distinct) and wraps begin/end ints in
  `<from>`/`<to>` slots. Open-ended ranges work naturally (absent
  field → no wrapper). Forgot to declare new variants
  (`Inclusive`/`Exclusive`) as markers in `node_spec` first time;
  caught by `containers_have_content_or_are_absent` invariant —
  fixed in same iter.
- [x] iter 179: Ruby ternary `<conditional>` → `<ternary>` rename
  + `<then>`/`<else>` role wrappers via Custom handler. Lessons:
  almost shipped via field-wrap entries `("alternative", "else")` —
  that would have wrapped if/elsif alternatives too, breaking
  collapse_else_if_chain. Caught by tests on first run; reverted to
  Custom handler scoped to Conditional only.
- [x] iter 178: C# member-access role-wrap (port iter-147 to C#).
  `MemberAccessExpression` Custom handler wraps receiver in
  `<object>` and property in `<property>`. JSON
  `{member: {object: {...}, property: {name: "X"}}}` — no more
  `name`/`children` collision. 2 transform-test xpaths + 1
  integration test xpath updated. Iter-175 review's "iter 147
  missed C#" now closed.
- [x] iter 177: Ruby `<case>` / `<when>` list distribution — closes
  2 backlog items: `case.when: [...]` array (no more children
  overflow on multi-when cases) AND `when.pattern: [...]` for
  multi-pattern `when X, Y`. Targeted post-pass (not bulk via
  distribute_member_list_attrs) because role-mixed children.
- [x] iter 176: Go closure body archetype — closes iter-174's
  missed-language gap. `closure/body/...` → `closure/value/expression/...`
  (single-stmt) or `closure/body/...` (multi-stmt). Cross-language
  `//closure/value/expression/...` (Go) now parallels `//lambda/value/...`
  (other PLs).
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
