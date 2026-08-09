---
name: architecture
author: boukeversteegh
description: Use when implementing a feature that needs changes in underlying layers, when reviewing code, when refactoring or migrating, or when discussing a technical design.
---
# Architecture Skill

_Portable methodology — no project-specific paths or conventions inside._

## When to use

Reach for this skill when:

- you're implementing a feature whose foundation isn't shaped for it — the underlying layer needs to change before the feature can land cleanly,
- you're refactoring or migrating structure,
- you're reviewing code (always — the questions here are the right lens for any review),
- the user wants to discuss or design a technical solution.

It guides the agent to extract invariants first, assign clear ownership boundaries, refactor structure before layering on patches when needed, normalize inputs once, remove bypasses and duplication, implement in stable incremental slices, and validate behavior with semantic tests before broad snapshots and doc updates. It is especially useful when there is risk of local fixes causing architectural drift, concern leakage, inconsistent behavior across entry points or output forms, or an end result with more factors rather than fewer.

---

## Purpose

This workflow is for complex tasks that involve feature work, refactoring, architectural changes, migration behavior, or broad plumbing across a codebase.

Its goal is not just to make the task "work." Its goal is to leave the codebase simpler, more coherent, and easier to extend than before.

---

## Core Principles

1. Design invariants first, not implementation details.
2. Normalize inputs once, close to the boundary.
3. Keep concepts owned by one layer.
4. Refactor structure before adding behavior when the old structure is fighting the new model.
5. Delete duplication instead of wrapping it in more layers.
6. Prefer one principled path over many special cases.
7. Test semantics directly; use snapshots only as broad regression coverage.
8. Commit when a stable invariant becomes true.

---

## Default Mindset

Treat the task as a system-design problem first and an editing problem second.

Before changing code, answer these questions:

- What concepts are being introduced or changed?
- What must become true everywhere when the task is complete?
- Which existing shortcuts, bypasses, or exceptions conflict with that model?
- Which layer should own each concern?
- What behavior is part of the public contract?
- What edge cases could force a redesign if ignored too long?

Do not begin by patching the first visible symptom.

---

## Before Treating a Bug as Local

Before fixing a bug, ask: is this an instance of a class?

If the component you're fixing is one of several with the same role (caches, retries, validators, serializers, rate limiters, auth checks, deserialization paths), the defect is likely shared. The locality is often an illusion — you noticed it here because this is where it surfaced, not because this is where it lives.

Do this:

- name the role the component plays,
- locate the siblings that play the same role,
- check whether each one has the same defect,
- decide whether to fix the class or document why this instance is genuinely special.

A one-spot fix to a class-level bug guarantees you will fix it again, differently, in each sibling — and the inconsistencies between those fixes become their own architectural problem.

The same prompt applies when adding behavior: if the new behavior is the kind of thing every sibling should also have (e.g. single-flight load coalescing on a cache, timeout on an outbound call, idempotency on a write), deciding now is cheaper than retrofitting later.

---

## Phase 1: Extract the Invariants

Read the task, design notes, failing tests, and surrounding code until you can write a short list of global truths.

Examples of good invariants:

- "User inputs, defaults, and implicit behavior are normalized in one place."
- "All externally visible behavior flows through one primary execution path."
- "The internal model represents the concept directly instead of encoding it through scattered special cases."
- "Boundary-specific requirements are handled at the boundary layer, not mixed into domain logic."
- "Warnings, validation, and error behavior are determined from the normalized plan, not re-inferred later."
- "The same feature behaves consistently across modes, entry points, and output forms."

A good invariant is:

- global, not local,
- stable across entry points and modes,
- strong enough to simplify future decisions.

If you cannot state the invariants clearly, you are not ready to implement.

---

## Phase 2: Map Ownership Boundaries

For each important concern, decide where it belongs.

Common concerns to assign:

- input parsing,
- normalization and default resolution,
- domain model construction,
- validation,
- execution orchestration,
- rendering or serialization,
- boundary adapters,
- side effects,
- migration behavior,
- warnings and diagnostics.

Use this rule:

A concern should have one obvious owner.

Warning signs that ownership is broken:

- the same concept is interpreted in multiple layers,
- entry-point code is shaping domain behavior,
- boundary layers are compensating for model inconsistencies,
- mode-specific code reimplements shared behavior,
- one feature behaves differently for accidental rather than intentional reasons.

If ownership is unclear, fix structure before adding more logic.

---

## Phase 3: Decide Whether to Refactor First

Refactor first when one or more of these are true:

- the feature would require special cases in several files,
- existing shortcuts bypass the main pipeline,
- the same transformation already exists in multiple places,
- adding the feature on top would increase coupling,
- the current model cannot express the intended design cleanly,
- edge cases would be awkward or inconsistent under the current structure.

Do not preserve bad structure just because it already exists.

If the old structure fights the new concept, the refactor is part of the feature.

---

## Phase 4: Plan the Work as Invariant-Bearing Slices

Break the task into slices where each slice establishes a stable improvement.

Good slice types:

- add the new model, type, enum, or state representation,
- introduce normalized planning or execution context,
- unify duplicate paths,
- move special cases into the main pipeline,
- update boundary layers to consume the unified model,
- remove obsolete code,
- add targeted tests for tricky contracts,
- regenerate broad regression artifacts,
- update docs and migration notes.

Avoid slices that are only "some edits in many places." Each slice should have a purpose that can be described in one sentence.

---

## Phase 5: Implement from Structure Toward Behavior

Preferred order:

1. Introduce or reshape the model.
2. Add normalization or planning.
3. Route code through one path.
4. Remove old bypasses and duplicate shaping.
5. Add behavior on the unified path.
6. Tighten edge cases and warnings.
7. Expand test coverage.
8. Update docs and broad regressions.

This order reduces rework because later decisions are made on a cleaner foundation.

Do not implement surface behavior first if the infrastructure underneath is known to be wrong.

---

## Phase 6: Handle Input Normalization Carefully

Complex systems often fail at boundaries.

When flags, modes, config, defaults, and implicit behavior interact, create one normalization step that produces an internal plan.

That plan should answer questions such as:

- what mode is actually active,
- what feature variant is selected,
- which defaults remain in effect,
- which user inputs are ignored, replaced, or contradictory,
- which warnings must be emitted,
- which limits or implied values apply.

Rules for normalization:

- do it once,
- make it explicit,
- make contradictions fail early,
- keep warnings consistent with actual behavior,
- avoid recomputing policy later in the pipeline.

If multiple downstream layers must "figure out what the user meant," the normalization is incomplete.

---

## Phase 7: Remove Bypasses Early

Bypasses are often the source of long-term inconsistency.

Examples:

- early returns that skip the main pipeline,
- entry-point-specific shaping,
- boundary-specific special paths,
- direct printing or emission of special values,
- one-off optimizations that change semantics.

When possible:

- route the special case into the common model,
- let the normal pipeline produce the result,
- keep special behavior only where it is truly intrinsic.

A feature becomes easier to reason about when fewer things are "handled specially."

---

## Phase 8: Prefer Direct Semantic Tests

Add small focused tests for behavior that defines the contract.

Examples of semantic contracts:

- shape of output,
- singular vs sequence behavior,
- error and warning conditions,
- empty-result behavior,
- exit codes,
- stderr vs stdout routing,
- interactions between explicit and implicit settings,
- mode-specific behavior,
- migration behavior.

Use broad snapshots or regression artifacts after the semantics are nailed down.

Snapshots are useful for regression coverage, but they are weak at explaining why behavior is correct.

If a bug would be hard to understand from a broad diff alone, write a direct test.

---

## Phase 9: Update Documentation Last, but Not as an Afterthought

Only mark documentation, checklists, or validation items complete when they are:

- covered by tests,
- directly verified by execution,
- or mechanically implied by already-verified behavior.

Treat docs as design intent, not infallible truth.

If you find contradictions:

- resolve them against the intended conceptual model,
- preserve the architectural principle,
- and adjust the doc explicitly rather than silently drifting from it.

Documentation should reflect the final ownership model and behavior contracts, not the implementation history.

---

## Communication Pattern During Work

When working in steps, communicate like this:

1. State the current objective.
2. State what you are checking or changing next.
3. Surface any architectural decision that constrains future work.
4. Mention when a previous assumption turned out to be wrong.
5. Distinguish clearly between verified behavior and planned work.

Do not narrate every file edit.
Do not claim completion until behavior, tests, and docs agree.

---

## Commit Strategy

Commit when a stable invariant becomes true.

Good commit examples:

- "Introduce unified planning for feature selection"
- "Route behavior through a single execution path"
- "Remove bypass path for special-case output"
- "Add contract tests for edge-case behavior"
- "Regenerate regressions and update docs"

Bad commit examples:

- "WIP"
- "Fix stuff"
- "More changes"
- "Try again"

Each commit should make the codebase easier to understand, even if the feature is not fully complete yet.

---

## Heuristics for Good Structural Decisions

Choose the refactor when it reduces future branching.
Choose the common path when special handling would duplicate policy.
Choose explicit internal models over implicit combinations of inputs.
Choose deletion over compatibility layers when the old layer is redundant.
Choose orthogonality over convenience when two concerns are starting to leak into each other.

Ask repeatedly:

- What is the owner of this concept?
- Is this logic duplicated?
- Is this policy being inferred too late?
- Am I preserving a shortcut that should disappear?
- If someone adds a related feature later, will this design help or hurt them?

---

## Common Failure Modes to Avoid

1. Local patching without a global model.
2. Adding wrappers around duplicated logic instead of removing duplication.
3. Letting entry-point code, model code, and boundary code all participate in policy decisions.
4. Preserving legacy shortcuts that conflict with composability.
5. Writing only snapshot tests and missing behavioral contracts.
6. Treating design docs as line-by-line law even when they contradict the actual model.
7. Making every edge case a special case instead of improving the abstraction.
8. Delaying refactors until after feature logic has already spread everywhere.
9. Mixing normalization, validation, and rendering decisions across layers.
10. Counting files changed instead of measuring conceptual simplification.
11. Fixing a class-level defect in only the instance where it surfaced, leaving the same bug latent in sibling components.

---

## Quality Bar for Completion

A complex task is done when all of these are true:

- the feature works,
- the main invariants are visible in the structure,
- old bypasses or duplicate paths are removed or intentionally isolated,
- edge cases are tested,
- warnings and errors match the actual semantics,
- docs match the implemented model,
- the final design has fewer moving parts than the likely patch-based alternative.

The standard is not "it passes."
The standard is "the next related change will now be easier."

---

## Short Operating Checklist

Use this before and during implementation:

- Ask whether the bug or feature is class-level before treating it as local; locate sibling components that play the same role.
- Identify the global invariants.
- Identify the owner of each concern.
- Decide whether the current structure can support the feature cleanly.
- Refactor first if the structure would otherwise force exceptions.
- Normalize inputs once into an internal plan.
- Route behavior through one principled path.
- Remove bypasses and duplicate shaping.
- Add semantic tests for tricky contracts.
- Regenerate broad regressions.
- Update docs only where verified.
- Commit when an invariant becomes true.

---

## One-Sentence Summary

Do not build the feature by teaching every layer a new exception; build it by making one clean model true and routing the system through it.
