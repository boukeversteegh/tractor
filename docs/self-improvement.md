# Self-Improvement Loop for Tree Transformations

A repeatable process for identifying and fixing single, principle-grounded
issues in tractor's tree-shape transformations. Each iteration is one focused
commit that lands green, ships independently, and references the design
principle it serves.

The loop is **scoped to transformations only** — per-language `rules.rs` /
`transformations.rs`, the shared `transform/` post-walk passes, and the
per-language specs at `specs/tractor-parse/semantic-tree/transformations/*.md`.
CLI, output formatting, query engine, and build infrastructure are out of
scope unless a transformation change forces work there.

## Loop process — one iteration

1. **Refresh context.** Re-read the relevant slice of
   `specs/tractor-parse/semantic-tree/design.md` (Goals #1–#7,
   Principles #1–#17) and any per-language spec that bears on the target.

2. **Pick a target.** *Exactly one* item from a discovery source (below).
   Prefer items that are:
   - clearly grounded in a stated Goal/Principle violation, OR
   - flagged by the user inline, OR
   - bounded enough to land in a single commit.

3. **Frame the change.** Write down (in the commit-message draft):
   the problem, the current shape, the proposed shape, and which
   Goal/Principle it serves. Mechanical extensions of an existing
   pattern can be brief.

4. **Subagent review** when the change has tradeoffs to weigh. Spawn one
   `general-purpose` Agent with the proposal, the relevant spec slice,
   and any parallel commits as context. Ask for ship/amend/reject in
   <300 words. **Skip when:** the change is a clear bug fix mirroring a
   prior decision, or a mechanical extension of an existing pattern.
   **Always use a subagent when:** the change introduces a new shape,
   touches Principle #11 / #13 / #15 territory, or affects more than
   one language at once.

5. **Decide.** Fold subagent feedback in. Don't defer to it
   uncritically — a reviewer's *framing* isn't load-bearing. Check
   their conclusion against the cited principle's actual text. If you
   disagree, push back via SendMessage and arrive at a joint
   conclusion before acting. If ship: continue. If amend: revise and
   re-evaluate. If reject: write a one-line note in the iteration
   commit/notes explaining why and pick a different target.

6. **Implement.** Touch only:
   - `tractor/src/languages/<lang>/{rules.rs,transformations.rs,output.rs}`
   - `tractor/src/languages/mod.rs` (post-transform wiring)
   - `tractor/src/transform/mod.rs` (shared post-walk helpers)
   - `tractor/tests/transform/**` (transform tests)
   - `tractor/tests/core_integration_tests.rs` (only when an existing
     test is invalidated)
   - `specs/tractor-parse/semantic-tree/transformations/<lang>.md`
     (when documenting a per-language decision)
   - `specs/tractor-parse/semantic-tree/design.md` (only with explicit
     user approval — design decisions are not autonomous)

7. **Tests + snapshots.**
   - `cargo test` must end green for the iteration's stated scope. (See
     "Multi-iteration sweeps" below for when a single test is held red
     across multiple iterations.)
   - Update transform tests whose XPath assertions no longer match the
     new shape.
   - `cargo run --release --bin update-snapshots` to refresh snapshots.
   - Review the snapshot diff manually — confirm every change is the
     stated shape change, not a regression elsewhere.

8. **Document new node shapes in fixtures.** When the iteration
   introduces a new emitted shape (a new element name, marker, or
   structural variant), extend the relevant blueprint (e.g.
   `tests/integration/languages/<lang>/blueprint.<ext>`) so the snapshot
   captures the shape. New shapes that no fixture exercises are
   undocumented; they're trivial to break later because the snapshot
   regen has nothing to anchor.

9. **Commit + push.** One focused commit per iteration:
   - Title: `Self-improvement loop iter N: <one-line summary>`
   - Body: problem → fix → which Goal/Principle it serves → tests
     touched → "Decided autonomously per the self-improvement loop"
     when no human ratified it; "Per user feedback" when the user
     flagged the gap inline.
   - Push immediately — the user has explicitly asked for prompt pushes.

If tests stay red outside the iteration's stated scope, **do not commit**:
either keep iterating until green, or revert with no commit.

## Multi-iteration sweeps

Some changes (e.g. clearing a category of violations across all 13+
languages) span many iterations. During such sweeps:

- The strict gate test for that category may stay red across iterations
  while you chip through it. That's expected; mention the remaining
  violation count in each commit body.
- Each iteration still leaves *its* language clean.
- All other tests stay green per iteration.
- Don't bundle two languages into one commit even when the work is
  identical — separate commits keep the cumulative diff bisectable and
  the per-language decisions reviewable.

## Discovery sources

Listed roughly in order of authority. The loop sweeps all of them
periodically; not every iteration consults every source.

### A. User flags (highest authority)

Inline messages like *"this conditional shape isn't isomorphic"* or
*"we should also fix X here."* Always honour and prioritize these.

### B. Snapshot review

The per-language fixtures at
`tests/integration/languages/<lang>/blueprint.<ext>.snapshot.txt`. Read
each periodically and look for shapes that:

- Use raw tree-sitter kind names with underscores (Principle #2 / #17
  violation — the strict static-side gate in `tree_invariants.rs`
  catches the rule-table side; runtime drift can still slip through).
- Use a different structural shape for "the same concept" across
  contexts (Principle #5 violation).
- Use a wrapping element name where a marker would compose better
  (Principle #15 violation).
- Place markers on text-only leaves (Principle #13 violation).
- Bury the operand under an irrelevant parent path (Goal #6
  broad-to-narrow regression).

### C. Strict gates in `tractor/tests/tree_invariants.rs`

The invariant tests are the loop's mechanical conscience. They surface
violations the moment they appear:

- `no_underscore_in_node_names_except_whitelist` — Principle #2 / #17.
  Walks both fixtures (runtime) AND every language's rule table
  (static side) for `Rule::Passthrough` kinds whose snake_case name
  contains an underscore not on `ALLOWED_UNDERSCORE_NAMES`. Catches
  drift before it surfaces in output.
- `op_marker_matches_text` — every canonical operator carries the
  declared marker.
- `all_names_declared_in_semantic_module` — emitted names live in the
  language's TractorNode enum or in `OPERATOR_MARKERS`.
- `kind_attribute_is_non_empty`, `name_element_is_text_leaf`,
  `markers_stay_empty`, `all_node_names_are_lowercase`,
  `no_grammar_kind_suffixes` — companion checks.

When a gate flips from advisory to asserted, expect a sweep of
iterations to clear the resulting backlog.

### D. TODO comments in rule tables

`tractor/src/languages/<lang>/rules.rs` files mark unhandled or
passthrough grammar kinds with proposed shapes. Each TODO is one or
several iterations of work; design-call items want a subagent review
before ship.

### E. Per-language transformation specs

`specs/tractor-parse/semantic-tree/transformations/{rust,csharp,
typescript,java,go,python,ruby,php}.md`. "Open questions" / "Pending"
/ "Future" sections in these flag known suboptimal shapes where a
design decision was deliberately deferred. Several already have
proposed shapes written out — those are low-friction targets.

### F. The design doc

`specs/tractor-parse/semantic-tree/design.md`. Goals and Principles are
the *evaluation criteria*, not generally a discovery source — but
**scan for "audit candidates" lists** (e.g. Principle #15 has one).
Items marked there are explicitly known violations.

### G. Failing or fragile transform tests

Tests that need awkward `(self::A or self::B)` disjunctions, or tests
whose XPath uses descendant-axis fallbacks where a direct child should
work, are evidence the tree shape isn't serving the rule cleanly. Each
such site is a candidate.

### H. Subagent or independent re-readings

Periodically (every ~5 iterations) ask a `general-purpose` Agent to
re-read snapshots cold and report shapes it finds suspicious. After
each iteration commits + pushes, also spawn a subagent to (a) review
the diff against `design.md` and (b) report any *new* issues it
surfaces — append findings to the active backlog.

## Evaluation criteria

Every proposal is checked against:

- **Goal #1** Intuitive Queries — would a developer naturally write
  this query?
- **Goal #4** Minimal Query Complexity — does the change reduce
  disjunctions / awkward predicates?
- **Goal #5** Match the Developer's Mental Model — is the primary
  node a *concrete developer concept*?
- **Goal #6** Broad-to-Narrow Query Refinement — does `//concept`
  still match the broad case after the change?
- **Principle #5** Unified Concepts — same concept, same name? *Not
  the same as "unique names"* — names can be reused across roles
  when parent context disambiguates (see Principle #5 corollary).
- **Principle #11** Specific Names Over Type Hierarchies — no
  abstract supertype wrappers without justification.
- **Principle #13** Annotation Follows Node Shape — markers stay off
  text-only leaves.
- **Principle #15** Markers Live in Stable Predictable Locations —
  does the change preserve modifier-agnostic parent-position queries?
- **Principle #17** Avoid Compound Node Names — single-word names
  unless `else_if`-style genuinely-multi-word concepts.

If a change requires writing down a *new* design decision (i.e. one
not deducible from existing principles), use the "documenting it and
the rationale" rule: add the rationale to the per-language spec file
and tag the commit message with "Decided autonomously per the
self-improvement loop" so the user can re-evaluate later.

## Iteration scope

**One iteration = one commit + push, in scope-green state.** Sizings:

- *Mechanical extension* (e.g. add Phase-3 migration to one more
  language): 1 iteration.
- *Bug fix mirroring a prior decision* (e.g. C# `obj!` → non-null host
  after the TS analog landed): 1 iteration.
- *Cross-language audit follow-through* (e.g. fix `<op>` extraction in
  three languages that share a missed pattern): 1 iteration if the fix
  is identical per language; otherwise split per language.
- *New shared helper or transform pass*: 1 iteration if focused;
  otherwise split (helper in iter N, per-language adoption in N+1, …).
- *Single-language gate-clear sweep* (e.g. clear all Principle #17
  violations in language X): 1 iteration per language.
- *Design decision*: exit autonomy — escalate to user with subagent
  review attached.

**Anti-pattern: bundling unrelated fixes in one commit.** Even if two
items are queued, prefer two commits.

## Working with reviewer subagents

When a subagent flags something:

- **Trust their fresh eyes** for shape findings ("look at this
  snapshot — these two paths are inconsistent"). Independent reading
  catches what familiarity hides.
- **Don't trust their framings** uncritically. A reviewer who says
  "this is a Principle #5 violation" is making *two* claims —
  (1) here's a concrete shape concern and (2) it violates Principle
  #5. Re-read the principle. If their (2) is wrong, the underlying
  concern (1) might still be worth addressing — or might not.
- **Push back via SendMessage** when their framing seems off. Cite the
  principle text, propose an alternative reading, ask for their take.
  This is cheap and often surfaces a sharper joint understanding.
- **Defer carries cost.** Acting on a misread framing wastes an
  iteration on an unnecessary rename or restructure, and adds noise
  to the project history.

## Operating directives (settled with the user)

1. **Cadence**: run until the active backlog is empty.
2. **Backlog ordering**: simple issues first (whether narrow scope or
   broad-but-mechanical), then progressively harder design-heavy
   items. Re-order the backlog as new items are discovered.
3. **Subagent usage**: spawn a subagent **every time** a solution has
   tradeoffs that must be weighed. Mechanical fixes (single-language
   migration mirroring a prior decision; clear bug fixes; well-
   precedented mechanical extensions) skip the subagent.
4. **Post-cycle review**: after each iteration commits + pushes, spawn
   a subagent to (a) review the diff against
   `specs/tractor-parse/semantic-tree/design.md`, and (b) report any
   *new* issues it surfaces for the backlog. Append the findings
   before starting the next iteration.

These directives run without further user check-ins until the backlog
is empty or the user pauses the loop.

## Critical files

The loop touches these files repeatedly:

- `specs/tractor-parse/semantic-tree/design.md` — read-only reference;
  only edited with explicit user approval.
- `specs/tractor-parse/semantic-tree/transformations/{rust,csharp,
  typescript,java,go,python,ruby,php}.md` — per-language decision
  records; written when an iteration documents a non-trivial
  per-language choice.
- `tractor/src/languages/<lang>/{rules.rs,transformations.rs,
  output.rs}` — per-language transform code.
- `tractor/src/languages/mod.rs` — per-language `LanguageOps` registry
  and post-transform wiring.
- `tractor/src/transform/mod.rs` and `tractor/src/transform/operators.rs`
  — shared helpers and the OPERATOR_MARKERS table.
- `tractor/tests/transform/**` — transform tests; XPath assertions
  here track tree shape and need updating when shape changes.
- `tractor/tests/tree_invariants.rs` — invariant gates; occasionally
  needs whitelist amendments (rare; bias toward fixing the underlying
  shape instead).
- `tests/integration/languages/<lang>/blueprint.<ext>.snapshot.txt` —
  regenerated each iteration; reviewed manually. Extend the source
  blueprint when an iteration introduces a new emitted shape.

## Verification

How to confirm the loop is working as designed:

- **Per iteration**: cargo test green within the iteration's stated
  scope; reviewed snapshot diffs match the stated shape change; push
  lands without errors.
- **Per ~5 iterations**: backlog has shrunk net (items closed > items
  added) OR backlog growth is from validated discovery (cold-read
  pass, user flags) rather than scope creep.
- **Cumulative**: the original motivating XPath
  (`//body/expression[.//field[value//name='xot' and starts-with(name,'with_')]] / following-sibling…`)
  still works — it's the canary. If it breaks, the loop has regressed
  something.
- **Loop health**: each iteration's commit message clearly maps the
  change to a stated Goal/Principle. Autonomous decisions are flagged
  so the user can re-evaluate later.
