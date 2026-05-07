# Transform Validation Architecture

A layered defense against shape regressions in tractor's tree transformation
pipeline. Captures the design conversation from iter 291 onward: what
regressions actually look like, why `cargo test` + snapshot review isn't
sufficient, and how to mechanize the strict-improvement gate added to the
self-improvement loop.

This document is **architecture, not a contract** — it describes how
validation is layered, what each layer catches, and the ship order. The
canonical rule list lives in code (cargo tests + the spec-conformance
walker); this doc explains *why* those rules exist and how they relate.

---

## 1. Problem

The transformation pipeline rewrites tree-sitter output into tractor's
semantic XML vocabulary in three phases: per-kind handlers in
`walk_transform`, shared per-language `post_transform` passes, and
cross-cutting passes in `transform/mod.rs`. The tree mutates in place
across ~150 commits' worth of layered local fixes.

Local fixes have produced **whole-tree property regressions** that none of
the existing checks catch. The Lessons section of
`todo/39-post-cycle-review-backlog.md` codifies them. The canonical
example is iters 205–212: a bulk `distribute_member_list_attrs` sweep
closed 121 audit overflow sites but created **~2,389 new 1-element JSON
arrays**, silently reversing singleton-role-slot decisions from iters
178/179/180/195/212. All tests stayed green throughout. Iter 213 reverted
the sweep.

Other regression archetypes recurring in Lessons:

- **Marker / wrapper name collision** (iter 184/275/283): same name as
  empty marker `<X/>` AND structural wrapper `<X>...</X>` on the same
  parent → JSON serializer collides on the singleton key.
- **1-element JSON arrays on singleton role-slots** (iter 213): a slot
  documented as singleton (`condition`, `then`, `value`) silently
  becomes a 1-element array because a sweep tagged it `list=`.
- **Cross-language sweep gaps** (iter 152/162/174): a "sweep all
  languages" intent missed one; surfaced 25-30 iters later.
- **Snapshot dual-surface drift** (iter 285): `.json` regressed while
  `.txt` looked fine; or only one of the two was staged.
- **Whack-a-mole / flip-flop** (iter 213 archetype generalized): a later
  iter silently reversed a deliberate earlier decision because the
  same element name / shape was treated as fresh territory.

The unifying property: **every test was green and the diff looked locally
sensible at every iter.** Regressions surfaced in non-local properties:
JSON cardinality, cross-fixture consistency, cross-language alignment,
emergent collisions between independent transforms.

## 2. Why the existing checks aren't sufficient

Existing validation (as of iter 291):

| Layer | What it catches | What it misses |
|-------|-----------------|----------------|
| `cargo test` (unit) | Functions return correct values for inputs the test author thought of | Whole-tree regressions in fixtures the test doesn't touch |
| `cargo test` (transform/) | Per-construct shape claims via `claim()` xpath assertions | Anything not explicitly claimed |
| `tree_invariants.rs` (12 invariants) | Naming hygiene, marker emptiness, container non-emptiness, repeated-name nesting, kind preservation, declared-name membership, op-marker text alignment, anonymous keyword leaks | **Whole-tree structural properties** — JSON cardinality, slot cardinality, marker/wrapper collision, cross-language alignment |
| Snapshot tests | Any change to tree-text or JSON output | Nothing — but all changes are "noise" until a human classifies them |
| Manual snapshot review | Whatever the reviewer remembers to look for | Whatever the reviewer doesn't remember; entire sweeps that look uniformly correct (iter 213) |

The gap is the **whole-tree structural property** row. None of the
existing layers asserts properties like "no `<P>` ever has both an empty
`<X/>` child and a `<X>...</X>` sibling" or "no JSON `children:` overflow
key appears anywhere." These are the regression archetypes.

## 3. Layered defense in depth

Five validation layers, ordered by how *early* they fire (earlier = catches
the bug closer to where it was introduced). Each layer trades cost for
specificity — earlier layers are more granular but require more
infrastructure.

```
   ┌─────────────────────────────────────────────────────┐
   │ 5. Validating-DOM (per-mutation)        [out of scope] │
   ├─────────────────────────────────────────────────────┤
   │ 4. Typed builders for high-traffic constructs       │
   ├─────────────────────────────────────────────────────┤
   │ 3. Per-handler post-condition validation            │
   ├─────────────────────────────────────────────────────┤
   │ 2. End-of-post_transform validation (in-memory)     │ ← phase 1 ships here
   ├─────────────────────────────────────────────────────┤
   │ 1. Shape contracts as cargo integration tests       │ ← phase 1 ships here
   ├─────────────────────────────────────────────────────┤
   │ 0. Snapshot diff manual review (existing baseline)  │
   └─────────────────────────────────────────────────────┘
```

**Layers 1 + 2 share a single predicate engine** — same rules, two
invocation sites. That's a single engineering investment producing two
gates.

### Layer 0 — Snapshot diff manual review (current baseline)

Status: in place since iter 170. The strict-improvement gate (loop plan
step 7a) formalizes that every diff hunk must be classified as intended
or incidental-but-neutral. This layer is the human safety net.

Limits: misses what a human doesn't notice; doesn't scale to bulk-sweep
iters where the diff is hundreds of lines of "uniform" change (iter 213
archetype).

### Layer 1 — Shape contracts as cargo integration tests

A new file `tractor/tests/shape_contracts.rs` (or split per-language
under `tractor/tests/shape_contracts/`) iterates every blueprint
fixture, parses + transforms it via the standard `parse()` entry
point, then applies a list of `ShapeRule`s.

```rust
struct ShapeRule {
    id: &'static str,
    description: &'static str,
    /// xpath that selects violating nodes — non-empty result = failure.
    violation_xpath: &'static str,
    severity: Severity,
    /// Optional minimal-source-program demonstrating the violation.
    /// Doubles as a regression test for the rule itself: must produce
    /// a non-empty result on this source.
    invalid_examples: &'static [(&'static str /* lang */, &'static str /* src */)],
    /// Source programs that MUST produce empty results — pins the rule
    /// against false positives.
    valid_examples: &'static [(&'static str, &'static str)],
}
```

Each rule is a single xpath that returns the *violating nodes*. Empty
result = pass. Non-empty result = fail with a per-node location report.

Initial rule set (the regression archetypes from §1):

```rust
ShapeRule {
    id: "no-children-overflow",
    violation_xpath: "//children",
    description: "JSON children-overflow — same-name siblings collided on a singleton key without role-named slot wrappers or list= tagging.",
    severity: Error,
    invalid_examples: &[
        ("rust", "fn f<T: A + B + C>() {}"),  // attribute multi-args archetype
    ],
    valid_examples: &[
        ("rust", "fn f<T: A>() {}"),
    ],
},

ShapeRule {
    id: "no-marker-wrapper-collision",
    violation_xpath: "//*[\
        ./*[not(*) and not(text())][name() = name(../*[name() = name(.) and (* or text())])]\
    ]",
    description: "Parent has both an empty marker <X/> AND a structural wrapper <X>…</X> with the same name. JSON serializer collides on key X. iter 184 archetype.",
    severity: Error,
    invalid_examples: &[
        // synthesized via crafted source that previously triggered
    ],
},

ShapeRule {
    id: "name-is-text-leaf",
    violation_xpath: "//name[*]",  // <name> with element children
    description: "<name> is reserved for identifier text leaves.",
    severity: Error,
    valid_examples: &[
        ("rust", "fn x() {}"),
    ],
},
```

Rules can also be expressed as Rust closures when xpath is awkward
(e.g. cross-language consistency checks that need to compare two
sub-trees). Hybrid form:

```rust
enum RulePredicate {
    XPath(&'static str),
    Custom(fn(&Xot, Node, &str /* language */) -> Vec<Violation>),
}
```

The cargo test runs every rule against every blueprint every CI build.
Failures are normal cargo test failures with stable output.

**Why integration tests, not `tractor.yml`**: tractor.yml requires
`tractor run` to fire. Cargo tests fire automatically with `cargo test`,
which is the canonical CI gate. Cargo tests can also use full Rust
expressiveness for predicates that don't fit in xpath. (tractor.yml is
still useful for *user-facing* lint rules; it's just not the right
venue for *internal* shape contracts.)

**Coverage estimate**: ~6 rules cover the archetypes currently in the
Lessons file. Adding a new rule when a new regression archetype
surfaces is a 5-10 line YAML-equivalent diff in Rust.

### Layer 2 — End-of-post_transform validation (in-memory)

Same predicate engine as layer 1, invoked from a different site.
Each language's `post_transform` ends with:

```rust
#[cfg(debug_assertions)]
crate::transform::validate_shape_contracts(xot, root, lang)?;
```

The function consults the same `ShapeRule` table layer 1 uses. In debug
builds it panics on violation; in release it's a no-op (or returns
`Result::Err` consumed by the caller, depending on the API surface
chosen).

**Why same predicate, different site**: layer 1 fires only when
blueprint cargo tests run. Layer 2 fires *every transform invocation* —
unit test, integration test, real query, CLI run. A regression
introduced by a shared helper that affects code outside the blueprints
gets caught at first invocation. The cost is a single extra tree walk
per transform; in debug builds this is fine.

**Why debug-only**: in release builds the cost-benefit shifts —
shape contracts are correctness invariants, not user-facing checks. CI
runs debug; production transforms run release. This may be revisited
if release-time validation becomes valuable.

### Layer 3 — Per-handler post-condition validation (future)

Each per-kind handler in `transformations.rs` declares the locally
expected shape it produces, and the dispatcher validates the subtree
rooted at the handler's node before returning. E.g.:

```rust
#[shape_post(<call><object/><name/><arguments/></call>)]
pub fn method_invocation(xot: &mut Xot, node: XotNode) -> Result<...> {
    // existing handler body
}
```

The macro generates a post-condition check that runs after the body
completes. Catches handler-local regressions that don't reach
`post_transform` validation because intermediate passes mask them.

**Cost**: declarative attribute macro + per-handler annotation work.
Multi-iter rollout. Pays back in handler-level debuggability.

**When to ship**: after layers 1+2 stabilize and we have data on
which classes of regression are layer-1 / layer-2 inadequate.

### Layer 4 — Typed builders for high-traffic constructs (future)

Replace ad-hoc xot mutations for the 5–10 most-touched constructs with
typed Rust builders:

```rust
Call::new(xot)
    .with_object(receiver_node)
    .with_method("greet")
    .with_arguments([arg1_node, arg2_node])
    .build()?;
```

Each builder generates the xot mutations under the hood and is
impossible to misuse (no way to construct a `<call>` with a `<from>`
child if the type signature forbids it). Adopt incrementally: each
construct that gets a typed builder retires that many ad-hoc
mutation sites.

**Cost**: per-construct API design + call-site migration. High upfront,
high long-term maintainability win.

**When to ship**: when a specific high-traffic construct shows
recurring regression patterns that layers 1–3 are catching reactively.

### Layer 5 — Validating-DOM (per-mutation) — out of scope

A wrapper around xot that validates every mutation against a global
shape model. The most powerful and the most expensive.

**Why out of scope**: the intermediate-state problem. The transform
pipeline mutates in place, with handlers and post-passes that move
through transitional states. Every mutation between handler entry
and handler return is "in flight." A per-mutation validator either
forces pre-build-and-swap (memory + refactor cost) or escape hatches
everywhere (defeats the purpose).

The regression archetypes in §1 don't justify this cost — they're
whole-tree property bugs, not per-mutation bugs. Per-mutation
validation catches a class of bug we don't have.

Documented for completeness; revisit only if the regression mix
shifts toward construction-site bugs.

## 4. Spec-conformance walker — the predicate engine

Layers 1 and 2 share a single walker. Its job: given a tree and the
language, walk every element, look up its `NodeSpec`, and validate
the spec's declared properties.

The existing `TractorNodeSpec` (`tractor/src/languages/mod.rs:42`)
declares `marker: bool, container: bool, syntax: SyntaxCategory`. Extend
to a richer role enum:

```rust
pub enum NodeRole {
    /// Empty element only (no text, no children). E.g. `<async/>`.
    MarkerOnly,
    /// Text content only, no element children. E.g. `<name>foo</name>`.
    TextLeaf,
    /// Container with element children, no text content. E.g. `<call>`.
    ContainerOnly,
    /// Both marker AND wrapper forms valid. E.g. `<new/>` (marker on
    /// constructor) and `<new>...</new>` (allocation expression).
    /// Implies the marker+wrapper collision invariant is suppressed
    /// for this name (it's an expected pattern).
    DualUse,
    /// Singleton role-slot under a specific parent. E.g. `<condition>`
    /// under `<if>`. Implies cardinality 1 expected.
    SlotWrapper { parents: &'static [&'static str] },
}

pub struct NodeSpec {
    pub name: &'static str,
    pub role: NodeRole,
    pub syntax: SyntaxCategory,
    /// Element names this node MUST NOT contain as children.
    /// E.g. `["children"]` for everything (no overflow allowed).
    pub forbidden_children: &'static [&'static str],
}
```

The walker iterates `NodeRole` variants and validates the corresponding
property:

| Role | Property |
|------|----------|
| `MarkerOnly` | element is empty (no text, no children) |
| `TextLeaf` | element has no element children |
| `ContainerOnly` | element has at least one child (text or element) |
| `DualUse` | both shapes valid; collision invariant suppressed |
| `SlotWrapper` | element appears only under listed parents; cardinality respected |

This subsumes 5 of the existing 12 invariants in `tree_invariants.rs`
(`markers_stay_empty`, `name_element_is_text_leaf`,
`containers_have_content_or_are_absent`, partially others). The
existing invariants stay in place during migration; new invariants
ship via the spec.

**Migration**: the existing 12 invariants don't need to move. New
properties land via spec extensions. As a property's hand-coded test
becomes redundant with the spec-driven walker, retire the hand-coded
version.

## 5. Integration with the self-improvement loop

Step 7a (strict-improvement gate) currently requires *manual*
classification of every snapshot diff hunk. Layers 1+2 mechanize part
of this:

- **Pre-existing manual gate**: every diff hunk classified as intended
  or incidental-but-neutral by the engineer (preserved).
- **New automated gate**: shape contracts in `cargo test` PLUS
  end-of-post_transform validation must pass (added).

A regression that the automated gate catches doesn't require manual
classification — the test fails, the iter is incomplete, no push.

Per the loop plan, step 7a's escape hatch (spawn a fresh diff-only
reviewer subagent) remains for cases the automated gate doesn't
catch. As the rule set grows, the manual gate's load shrinks.

## 6. Ship order and migration

**Phase 1** (single iter, low risk):
- Add `tractor/tests/shape_contracts.rs` with 3 starter rules:
  `no-children-overflow`, `no-marker-wrapper-collision`,
  `name-is-text-leaf` (port from existing invariant).
- Add `validate_shape_contracts(xot, root, lang)` helper in
  `transform/mod.rs`. Same rule list; debug-only invocation from
  every `<lang>_post_transform` after the existing pipeline.
- All current blueprints must pass on first ship. If any rule fires
  on existing blueprints, that's a discovered regression — fix or
  document the false positive in the same iter.

**Phase 2** (1-3 iters, after phase 1 stabilizes):
- Extend rule list as new archetypes surface from cold-read review or
  fresh regressions. Target: 8-12 rules covering the Lessons
  archetypes.
- Migrate `markers_stay_empty`, `name_element_is_text_leaf`, and
  `containers_have_content_or_are_absent` into the unified
  spec-conformance walker driven by the richer `NodeRole`.
- Retire the hand-coded versions when the spec-driven version reaches
  parity.

**Phase 3** (when justified by data):
- Per-handler post-condition validation (layer 3) for handlers that
  show recurring regression patterns.
- Typed builders (layer 4) for high-traffic constructs whose ad-hoc
  mutations have proven error-prone.

**Out of phase**: validating-DOM (layer 5) is not on the roadmap.

## 7. Open questions

These are real design uncertainties that the first ship will help
resolve:

1. **Rule expression: pure xpath vs. hybrid Rust closure.** Initial
   rules are all expressible as xpath. Cross-language consistency
   checks (e.g. "every language emits the same shape for member
   access") may need Rust. Decision deferred until phase 2 surfaces
   the need.

2. **Severity gradations.** Errors vs. warnings vs. advisory. Some
   archetypes (1-element arrays on singleton slots) might need to
   ship as warnings until the population is fully cleaned up. The
   existing `ASSERT_*` gate pattern in `tree_invariants.rs` is the
   precedent.

3. **Per-fixture exceptions.** A rule that's valid 99% of the time
   but legitimately fails on one construct. Today's invariants use
   whitelists (e.g. `REPEATED_NAME_WHITELIST`). The shape-contract
   form should use the same pattern: each rule may declare exempt
   element names with a justification.

4. **Performance.** Running 10 xpath rules against 9 blueprints adds
   maybe 100-500ms to `cargo test`. Likely fine; revisit if it
   becomes a measurable cost.

5. **Cross-language rule sharing.** Should rules be language-scoped
   (per-language file) or universal (apply to every language unless
   explicitly exempted)? Default: universal with explicit per-language
   exemptions, since most archetypes ARE cross-language.

## 8. Relation to existing artifacts

- **`docs/self-improvement.md` / `C:\Users\Bouke\.claude\plans\mossy-riding-parrot.md`**:
  the iteration loop. Step 7a (strict-improvement gate) is what this
  architecture mechanizes.
- **`tractor/tests/tree_invariants.rs`**: the existing 12 invariants.
  Five of them migrate into the spec-conformance walker over phase 2.
- **`tractor/src/languages/mod.rs:42` (`TractorNodeSpec`)**: the
  per-language spec table. Phase 2 extends it with `NodeRole`.
- **`todo/39-post-cycle-review-backlog.md` (Lessons)**: the empirical
  record of regression archetypes. Each archetype motivates one or
  more shape contracts.
- **`tractor.yml`**: project self-lint, runs against source code via
  `tractor run`. Distinct from shape contracts, which run against
  blueprint fixtures via `cargo test`. tractor.yml may eventually
  host shape contract rules for *user-facing* code; this doc covers
  the *internal* shape contracts only.

---

*Drafted iter 292 by user request after the iter 291 wind-down
discussion. Status: design intent. Code lands phase by phase per §6.*
