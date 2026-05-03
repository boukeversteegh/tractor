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
  kind instead. Iter 179 was caught by test; would have been a quiet
  regression otherwise. (Sister to the "all N languages done" lesson:
  changes that look universal often aren't.)
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
