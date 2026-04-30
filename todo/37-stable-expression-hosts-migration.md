# 37 — Stable Expression Hosts Migration

Migrate the semantic tree to **uniform `<expression>` hosts at every expression
position**, with closed-set expression modifiers as empty markers on the host.

Reference design:

- [`docs/design-transformation-expression-hosts-analysis.md`](../docs/design-transformation-expression-hosts-analysis.md) — full rationale
- [`specs/tractor-parse/semantic-tree/design.md`](../specs/tractor-parse/semantic-tree/design.md) — Goal #6 (Broad-to-Narrow), Principle #15 (Stable Expression Hosts)

## Why

The original motivating case: a rule that flags two consecutive `xot.with_*()`
calls in the same body silently misses pairs like `xot.with_a(node)?; xot.with_b();`
because `?` wraps the first call in `<try>`, so the calls are no longer siblings
of `<body>`. The same pattern bites every closed-set expression modifier
(`await`, `!`, `?.`, `await`, deref/ref).

The fix: every expression position gets a uniform `<expression>` host; the
operand keeps its specific name inside; modifiers attach as markers on the
host. Position-sensitive queries become modifier-agnostic.

---

## Current state per language

C# alone preserves `expression_statement` as `<expression>`. Everyone else
drops it. Modifier wrappers exist as their own elements (`<await>`, `<try>`,
`<ref>`, …) and need to become markers.

| Language | `expression_statement` | Modifier wrappers in scope |
|---|---|---|
| **Rust** | `Skip` (dropped) → migrate to `Rename(Expression)` | `try_expression` → `<try>` (line 158); `await_expression` → `<await>` (119); `reference_type` → `<ref>` (custom, 110) |
| **C#** | `Rename(Expression)` ✓ already done | `await_expression` → `<await>` (124); `conditional_access_expression` already a `<conditional/>` marker on `<member>` (45) — partial precedent; `cast_expression` unhandled (TODO 240) |
| **TypeScript** | `Skip` (dropped) → migrate | `await_expression` → `<await>` (95); `non_null_expression` → `<unary>` (130) **buggy — currently looks like a unary `!` op**; `as_expression` → `<as>` (94); `satisfies_expression` → `<satisfies>` (143); `type_assertion` → passthrough (192) |
| **Python** | `Skip` (dropped) → migrate | `await` → `<await>` (91) |
| **Java** | `Skip` (dropped) → migrate | `cast_expression` unhandled (TODO 209) |
| **Go** | `Skip` (dropped) → migrate | none in scope (no async/try-suffix) |
| **PHP** | `Skip` (dropped) → migrate | `cast_expression` → `<cast>` (103) — borderline (carries type data) |
| **Ruby** | (no obvious analog) | safe-navigation `&.` — TODO check shape |

Audit grep used:

```sh
grep -nE 'Await|TryExpression|NonNull|ExpressionStatement|CastExpression|AsExpression|SatisfiesExpression|reference_type' \
  tractor/src/languages/*/rules.rs
```

---

## Phased plan

### Phase 0 — Spec landing (DONE)

- Added Goal #6 (Broad-to-Narrow Query Refinement)
- Added Principle #15 (Stable Expression Hosts)
- Added analysis doc with rationale and rejected alternatives

### Phase 1 — Rust statement-level pilot

Smallest scope that proves the design. Stop at statement level; nested
expression positions come in Phase 2.

- [ ] Add a shared helper (working name `wrap_in_expression_host`) in
  `tractor/src/languages/` that wraps a node in `<expression>` and lifts
  one or more empty markers onto the host. Source order preserved
  (prefix markers before, postfix after) so renderability holds.
- [ ] In `tractor/src/languages/rust_lang/rules.rs`:
  - [ ] `RustKind::ExpressionStatement` — change from `Custom(skip)` to
    `Rename(Expression)`.
  - [ ] `RustKind::TryExpression` — replace `Rename(Try)` with a custom
    that lifts the inner operand into the surrounding `<expression>`
    host and adds a `<try/>` marker (postfix order).
  - [ ] `RustKind::AwaitExpression` — same pattern, prefix `<await/>`
    marker (note Rust's `.await` is postfix; mark order from source).
- [ ] Add `Expression` to the `RustName` enum if not present.
- [ ] Snapshots: `task test:snapshots:update`. Review changes by hand.
- [ ] Update `tractor.yml` `chain-fluent-xot-with` rule to drop the
  `(self::call or self::try)` workaround and target `<expression>`
  siblings of `<body>`. The rule's existing `expect:` blocks act as the
  acceptance test — the `?`-form invalid example must still flag.
- [ ] `task test` green.

Exit criterion: `xot.with_a(node)?; xot.with_b();` flags as cleanly as
`xot.with_a(); xot.with_b();` with the same xpath.

### Phase 2 — Rust nested expression positions

Wrap every expression position, not just statements. Argument values, return
values, condition slots, binary operands, member receivers, etc.

- [ ] Identify the field-wrapping points in the Rust transform that
  currently emit raw expression output (some may already wrap via the
  shared `with_wrap_child` helper).
- [ ] Extend the helper to handle "wrap unconditionally if not already
  inside an `<expression>`" so nested grammars like `try_expression /
  await_expression / call_expression` collapse to one host with two
  markers.
- [ ] Verify `await foo()?` produces:
  ```xml
  <expression>
    <await/>
    <call>foo()</call>
    <try/>
  </expression>
  ```
- [ ] Snapshots reviewed; integration tests pass.

### Phase 3 — Roll out to remaining languages

Order chosen by precedent and complexity:

1. **C#** — already preserves `<expression>`; complete the modifier migrations
   (`await_expression`, fix `cast_expression` TODO, align `conditional_access`
   to the host shape). Fix the `obj!` bug along the way (currently parsed as
   `<unary>` with `<op>!</op>` — should be a `<non_null/>` marker).
2. **TypeScript** — `await_expression` → marker, `non_null_expression` →
   `<non_null/>` marker (fixes the bug above), `as_expression` /
   `satisfies_expression` / `type_assertion` decisions:
   - `as`/`satisfies` carry a target `<type>` child → likely stay as named
     wrappers per Principle #15's "open-set / structured" carve-out.
3. **Python** — only `await`. Smallest surface.
4. **Java** — `expression_statement` migration; `cast_expression` decision.
5. **Go, Ruby, PHP** — minimal expression-modifier surface; mostly an
   `expression_statement` lift. Ruby `&.` — investigate; likely a marker.

### Phase 4 — Cleanup

- [ ] Audit remaining rules across `tractor.yml` and integration tests for
  `(self::X or self::Y)` patterns left over from the old shape.
- [ ] Update per-language `specs/tractor-parse/semantic-tree/transformations/*.md`
  rationale tables to reflect the new shape.
- [ ] Update `transformations.md` cross-cutting index to mention the
  `<expression>` host convention.
- [ ] Note the deferred "kind-as-marker" experiment (no named `<call>`
  element) as a separate todo if still relevant.

---

## Out of scope for this todo

- The radical "kind-as-marker" shape (`<expression><call/>...</expression>`
  without a named `<call>`). Recorded as a deferred experiment in #15;
  separate todo if pursued.
- Cast / type-assertion decisions (whether `<cast>`, `<as>`, `<satisfies>`
  stay as named wrappers or migrate). They carry structured data
  (a target type), so the host-with-marker pattern doesn't fit cleanly.
  Decide per language during Phase 3.
- TypeScript `obj!` bug fix can land independently of the broader migration
  if needed — it's a one-off mis-classification.

---

## Acceptance test

The `chain-fluent-xot-with` rule in `tractor.yml`, after Phase 1, should
flag both forms identically:

```rust
fn t() {
    xot.with_a();    // unmodified
    xot.with_b();
}

fn t() -> Result<(), E> {
    xot.with_a(node)?;   // try-modified
    xot.with_b(node);
    Ok(())
}
```

Both invalid examples already exist in the rule's `expect:` block.
