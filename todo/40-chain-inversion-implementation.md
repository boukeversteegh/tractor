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

- [ ] iter 239 — **Python** pilot. Verified iter 237: shape
  already matches canonical
  (`<call><member><object/><property/></member>...args</call>`).
- [ ] iter 240 — **Go**. Same shape as Python.

### Tier B — small normalization pass first

- [ ] iter 241 — **TypeScript**. Unwrap the `<callee>` wrapper
  before extraction; the inner `<member>` becomes the canonical
  callee.
- [ ] iter 242 — **Java**. Flat call shape
  (`<call><object/>NAME...args</call>`); wrap the receiver+name in
  a synthetic `<member>` first.

### Tier C — TBD-shape (sample blueprint per iter)

- [ ] iter 243 — **C#**.
- [ ] iter 244 — **Rust**.
- [ ] iter 245 — **Ruby**.
- [ ] iter 246 — **PHP** (note `->` operator instead of `.`).

## Cross-cutting follow-ups

- [ ] iter 247 — **Renderer update** (per-language). Each
  `tractor/src/render/<lang>.rs` reconstructs source from the
  right-deep shape; teach each to traverse `<chain>` with the
  language's correct member-access operator (`.`, `->`, `::`).
- [ ] iter 248 — **design.md Decision section**. Document the
  chain shape as canonical in
  `specs/tractor-parse/semantic-tree/design.md`. Requires explicit
  user approval per the self-improvement loop's design.md rule.
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
