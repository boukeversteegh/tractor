# Transform validation — regression archetypes and shape contracts

A field guide to the kinds of shape regressions that have actually happened in tractor's transformation pipeline, and how the current validation layers catch them. Salvaged from the iter-291 design conversation; the 5-layer speculative architecture in that conversation has been superseded by what was actually built (layers 0+1+2; layers 3–5 are not on the roadmap).

## 1. Regression archetypes

These are the bug classes that escaped `cargo test` + manual snapshot review during the imperative-pipeline era. Each one is a property of the *whole tree* (or of the cross-fixture / cross-language population), not of any single handler's output. Local tests passed; local diffs looked sensible; the bug was visible only by reading across the population.

- **Marker / wrapper name collision** (iter 184 / 275 / 283). Same name appears as empty marker `<X/>` *and* as structural wrapper `<X>…</X>` on the same parent. The JSON serializer collides on the singleton key `X`. Caught now by the `marker-wrapper-collision` shape contract.

- **1-element JSON arrays on singleton role-slots** (iter 213). A slot documented as singleton (`condition`, `then`, `value`) silently becomes a 1-element array because a sweep tagged it `list=`. Canonical example: iters 205–212 ran a bulk `distribute_member_list_attrs` sweep that closed 121 audit overflow sites but created **~2,389 new 1-element JSON arrays**, reversing decisions from iters 178/179/180/195/212. All tests stayed green. Iter 213 reverted the sweep.

- **Cross-language sweep gaps** (iter 152 / 162 / 174). A "sweep all languages" intent missed one language; the gap surfaced 25–30 iters later. Cross-language consistency tests (`tractor/tests/cross_language_*.rs`) are the catch.

- **Snapshot dual-surface drift** (iter 285). The `.json` snapshot regressed while the `.txt` snapshot looked fine; or only one of the two was staged. Caught by snapshot-pair completeness checks.

- **Whack-a-mole / flip-flop** (iter 213 generalised). A later iter silently reverses a deliberate earlier decision because the same element name or shape was treated as fresh territory. Caught by anchoring decisions in shape contracts whose violation is a test failure regardless of which side of the flip-flop is "current".

- **JSON `children:` overflow** (iter 184 archetype). Same-name siblings collide on a singleton JSON key because no role-named slot wrapper or `list=` tag distinguished them; the projection emits a synthetic `children` key. Caught by the `no-children-overflow` shape contract.

The unifying property: **every test was green and the diff looked locally sensible at every iter.** Regressions surfaced in non-local properties — JSON cardinality, cross-fixture consistency, cross-language alignment, emergent collisions between independent transforms.

## 2. The validation gap (why local tests aren't enough)

Local validation as it existed before iter 291:

| Layer | Catches | Misses |
|---|---|---|
| `cargo test` (unit) | Functions return correct values for inputs the test author thought of | Whole-tree regressions in fixtures the test doesn't touch |
| `cargo test` (transform/) | Per-construct shape claims via `claim()` xpath assertions | Anything not explicitly claimed |
| `tractor/tests/tree_invariants.rs` (12 invariants) | Naming hygiene, marker emptiness, container non-emptiness, repeated-name nesting, kind preservation, declared-name membership, op-marker text alignment, anonymous keyword leaks | **Whole-tree structural properties** — JSON cardinality, slot cardinality, marker/wrapper collision, cross-language alignment |
| Snapshot tests | Any change to tree-text or JSON output | Nothing — but all changes are "noise" until a human classifies them |
| Manual snapshot review | Whatever the reviewer remembers to look for | Whatever the reviewer doesn't remember; entire sweeps that look uniformly correct (iter 213) |

The missing row is **whole-tree structural property** — "no `<P>` ever has both an empty `<X/>` child and a `<X>…</X>` sibling," "no JSON `children:` overflow key appears anywhere." Shape contracts close that row.

## 3. What's in place today

Three layers of validation are wired in:

### Layer 0 — manual snapshot review

The strict-improvement gate in the self-improvement loop: every snapshot diff hunk is classified as intended or incidental-but-neutral. Misses what a human doesn't notice; doesn't scale to bulk-sweep iters.

### Layer 1 — shape contracts as cargo integration tests

`tractor/tests/shape_contracts.rs` iterates every blueprint fixture, parses via the standard `parse()` entry point, then applies a list of `ShapeRule`s. Each rule is an xpath (or Rust predicate) that selects *violating nodes*: empty result = pass, non-empty = fail with a per-node location report.

Rule definition lives in `tractor/src/transform/shape_contracts.rs`. Adding a new rule when a new regression archetype surfaces is a small Rust diff.

### Layer 2 — in-memory shape-contract assertions

`crate::transform::shape_contracts::assert_shape_contracts` is invoked in debug builds from the tree builder, consulting the same rule list layer 1 uses (`tractor/src/transform/builder.rs:546`). Layer 1 fires when blueprint cargo tests run; layer 2 fires on *every transform invocation* — unit tests, integration tests, real queries. A regression introduced by a shared helper that affects code outside the blueprints gets caught at first invocation.

Cost: one extra tree walk per transform in debug; release builds are no-ops.

## 4. NodeRole — the spec-conformance walker driver

The walker behind layers 1 and 2 reads each element's role from the per-language `TractorNodeSpec` table (`tractor/src/languages/mod.rs::TractorNodeSpec`, currently `marker: bool, container: bool` derived to `NodeRole`).

`NodeRole` (`tractor/src/languages/mod.rs`):

| Role | Property the walker enforces |
|---|---|
| `MarkerOnly` | element is empty (no text, no children). E.g. `<async/>`. |
| `ContainerOnly` | element has at least one child (text or element). E.g. `<call>`. |
| `DualUse` | both marker and wrapper shapes valid; the marker/wrapper collision invariant is suppressed for this name. E.g. `<new/>` and `<new>…</new>`. |
| `Unspecified` | falls back to `ContainerOnly` for now; tightened to explicit `TextLeaf` / `SlotWrapper` declarations in the future. |

Five of the original 12 `tree_invariants.rs` properties are subsumed by the spec-driven walker (markers stay empty, name is text-leaf, containers have content, repeated-name nesting, declared-name membership). The remaining hand-coded invariants stay until their spec form reaches parity.

## 5. How this evolves with the IR migration

In the imperative pipeline, shape contracts had to walk the rendered xot tree because that's where shape decisions accumulated across `walk_transform` + `post_transform` + cross-cutting passes. The IR pipeline pushes most of that work upstream:

- Shape decisions made in `lower_<lang>_root` are *type-level* — slot cardinality is `Box<Ir>` vs `Vec<Ir>`, marker presence is an enum variant. Many regressions that were runtime walks become unrepresentable.
- TODO.md S3E (move shape contracts to type-level where provable) shrinks the runtime walk to genuinely runtime rules (`op-marker-matches-text` is the canonical example — the rule depends on the actual text content).

The archetypes in §1 don't go away — they're properties of the emitted XML, regardless of whether it came from imperative mutation or IR rendering. The validation layers stay relevant; their *substrate* migrates from xot-walks to type checks where possible.

## 6. Where to read further

- **Shape contracts (live rule list):** `tractor/src/transform/shape_contracts.rs` and `tractor/tests/shape_contracts.rs`.
- **Tree invariants (legacy 12):** `tractor/tests/tree_invariants.rs`.
- **NodeRole + per-language spec:** `tractor/src/languages/mod.rs` (`TractorNodeSpec`, `NodeRole`).
- **Active work moving validation to type-level:** TODO.md slice S3E.
