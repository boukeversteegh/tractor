# Chain Inversion — Implementation Checklist

Design doc: [`docs/design-chain-inversion.md`](../docs/design-chain-inversion.md).

This file tracks the per-iter rollout of the inverter across the 8
supported languages. The design (target shape, helper API, rationale,
trade-offs) is settled in the design doc; this file is just a running
checklist.

## Helper primitives — DONE

- [x] iter 232 — design doc (initial draft, FLAT proposal).
- [x] iter 235 — `emit_chain` primitive + 14 unit tests.
- [x] iter 236 — `extract_chain` primitive + 11 unit tests.
- [x] iter 237 — `invert_chain_nesting` round-trip wrapper + 11
  integration tests.
- [x] iter 238 — chosen NESTED shape ratified by user; design
  promoted to `docs/design-chain-inversion.md`; this file slimmed
  to a running checklist.

## Per-language rollout

Each iter:
1. Verify the language's current `<member>`/`<call>` shape against
   the canonical input (see design doc § "Canonical input shape").
   Add a normalization pass if needed.
2. Add a chain-root finder (see design doc § "Per-language
   rollout").
3. Wire `invert_chain_nesting` into `<lang>_post_transform`.
4. Update transform tests whose XPaths assume the right-deep shape.
5. Regenerate snapshots; review tree-text AND JSON diffs.
6. Commit + push.

### Tier A — canonical input, no normalization

- [x] iter 239 — **Python**. Canonical shape, no normalization.
- [x] iter 242 — **Go**. Same shape as Python. Iter 245 fixed
  `is_chain_root` to also invert `<member>` arguments
  (non-first-child of `<call>`); this retroactively cleaned up
  Go member-as-arg cases that pre-iter-245 stayed un-inverted.

### Tier B — normalization pass

- [x] iter 243 — **TypeScript**. `typescript_unwrap_callee`
  pre-pass strips the `<callee>` wrapper. Required tweaks to
  `walk_call` for bare-name callees (now opaque Receiver).
- [x] iter 244 — **Java**. `java_wrap_call_member` pre-pass wraps
  flat `<call><object/>NAME...</call>` into canonical
  `<call><member><object/><property/></member>...</call>`.

### Tier C — TBD-shape (sample blueprint per iter)

- [x] iter 245 — **C#** (partial). Canonical shape works for the
  common case + 38 chains in blueprint. KNOWN GAPS captured below
  in "Open chain follow-ups": conditional-access `?.` shape uses
  `<condition>` slot (un-canonical); implicit-`this` member
  access without `<object>` slot left untransformed.
- [x] iter 246 — **Ruby**. `wrap_flat_call_member` helper
  extracted from the Java pilot and applied to Ruby; every `.X`
  parses as a method call so intermediate accesses also emit
  `<call>` rather than `<member>`. Documented exception in
  `tests/transform/chain.rs::ruby`.
- [x] iter 247 — **PHP**. `php_wrap_member_call_slots` pre-pass
  wraps the receiver in `<object>` on `<member>` (and only the
  object on `<call>`, leaving the bare `<name>` for
  `wrap_flat_call_member`). Receiver carries the `<variable>`
  wrapper because PHP variables hold the `$` sigil.
- [x] iter 248 — **Rust**. `rust_normalize_field_expression`
  pre-pass converts `<field><value><expression>RECV</expression>
  </value><name>X</name></field>` to canonical
  `<member><object>RECV</object><property><name>X</name></property>
  </member>`. All 8 PLs now invert chains.

## Open chain follow-ups (from iter 245 reviewer + cold-read)

- [x] iter 250 — **C# conditional-access (`?.`).** Pre-pass
  `csharp_normalize_conditional_access` converts the
  `<condition>` slot to canonical `<object>` slot. Inverter also
  fixed to propagate `<member>`-callee markers onto the resulting
  `<call>` step (so `[instance]` / `[optional]` ride the call,
  not the absent member).
- [x] iter 249 — **Subscript chains.** `walk_subscript` added,
  handles both slot-wrapped (TS / Python) and bare-children
  (Go/Java/C#/Ruby/Rust/PHP) shapes. TS subscript chains now
  invert cleanly.
- [x] iter 256 — **C# `base.X` and implicit-`this` member access.**
  Approach (a) shipped: `member_access_expression` detects a
  `"base."` / `"this."` text leak (tree-sitter inlines these as
  text rather than as structural elements) and synthesises an
  `<object>` slot containing a `<base/>` / `<this/>` empty
  element. After chain inversion the empty element rides as a
  marker on the `<object[access]>` chain root —
  `//object[access and base]/member` finds the 2 prior sites
  (`get => base.Priority` / `set => base.Priority = value`).
  Marker vocabulary matches the existing `:base(id)` /
  `:this(...)` constructor-initializer transform.
- [x] iter 255 — **C# `<instance/>` marker policy.** Resolved by
  dropping the marker entirely from chain steps. The
  `<object[access]>` chain root already signals "this is access,"
  so the per-step marker carried no information. Codified in
  design.md § "Hierarchical access nests top-down" → Rejected
  alternatives → "Per-step `<instance/>` markers." Java never
  needed it; the asymmetry is gone.
- [x] iter 249 — **Subscript chains extracted** (this entry
  duplicated the iter 249 closure above). `walk_subscript`
  produces `ChainSegment::Subscript`; TS subscript chains invert
  cleanly at `typescript/blueprint.ts.snapshot.txt:219-225`.

## Cross-cutting follow-ups

- [x] iter 256 — **Renderer update** (per-language). Audit
  result: only `csharp.rs`, `json.rs`, `yaml.rs` exist as
  renderers. `csharp.rs` is declaration-only (handles
  Class/Struct/Property/Field/Unit/Namespace/Import/Comment;
  no expression rendering at all). `json.rs` / `yaml.rs` are
  data-format renderers (chains don't apply). Chains never
  enter any renderer today, so the chain rollout requires zero
  renderer changes. Re-evaluate when expression-level rendering
  is added to `csharp.rs` or new languages gain renderers.
- [x] iter 257 — **design.md Decision section**. Landed as
  "Hierarchical access nests top-down" in
  `specs/tractor-parse/semantic-tree/design.md` (after the
  expression-host decision). Framed prescriptively: the principle
  is that source-order maps to tree depth for hierarchical
  access (member access, method chains, subscript indexing).
  Tree-shape restructuring is the implementation consequence
  when a parser produces operator-precedence shape; languages
  whose grammar already produces top-down (Java, Ruby) require
  no restructuring. Cross-language quirks, per-language
  pre-passes, and Dart cascade compatibility remain in
  `docs/design-chain-inversion.md`.
- [ ] (deferred — Dart) **Cascades support**. When Dart joins the
  supported PLs, extend `ChainSegment` with `Cascade` variant and
  `emit_chain` to group consecutive cascades under `<cascades>`.
  Design is forward-compatible per design doc § "Cascades".

## Per-iter checklist (per pilot language)

- [ ] Sample the current `<member>`/`<call>` blueprint shape
  (`tests/integration/languages/<lang>/blueprint.<ext>.snapshot.txt`).
- [ ] Note the field= attributes on `<object>`/`<property>` slot
  wrappers; note where the callee lives in `<call>`.
- [ ] Identify normalization needed (e.g. unwrap `<callee>`, wrap
  flat callee).
- [ ] Add chain-root finder + invocation to
  `<lang>_post_transform`.
- [ ] Run `cargo test`. Update transform tests whose XPaths assume
  the right-deep shape (queries like `//member/object/...`).
- [ ] `cargo run --release --bin update-snapshots`. Review BOTH
  surfaces:
  - `<blueprint>.snapshot.txt` — tree shape.
  - `<blueprint>.snapshot.json` — JSON cardinality and `$type`
    stripping for the new `<chain>` element.
- [ ] Verify the canary XPath still works
  (the iter-1 `//body/expression[…]` query on the original test
  fixture).
- [ ] Commit with title `Self-improvement loop iter N: <lang>
  chain inversion pilot`. Push.
