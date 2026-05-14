# Editable trees: mutation through XPath addressing

**Status:** Draft for iteration. Captures the design conversation that started after S13 (unified source renderer) landed. Concrete first slice is in flight (`Slice 1` below). Companion to [`docs/design-ir-and-renderings.md`](design-ir-and-renderings.md), which establishes that the typed tree is the source of truth and XML/JSON are projections — this doc says how *mutation* fits into that picture.

## The model in one paragraph

The user surface for mutation stays in the **XPath / XML domain** — same query language they already know, including its rich predicate syntax (`database[host='localhost']`, `class[name='Foo']/method`, etc.). Behind that surface, mutation happens on the **typed tree** (`SyntaxTree` / `DataTree` / `SqlTree`), not on the XML projection or on source bytes. The typed tree is the source of truth for both shape and content; the typed renderer (S13) reproduces source from it. XML is the addressing layer; the typed tree is the data layer; source is the output layer.

This is already how `set` works for `json` / `yaml` today (see `xpath_upsert::update_existing_via_data_ir`). The work below extends the same model to every other tree-pipeline language.

## Why this model

Three properties we don't want to give up:

1. **One sublanguage for the user.** XPath addresses *and* declares (`name[first='Ada'][last='Lovelace']` says both "find this name" and "set its parts to Ada / Lovelace"). The declarative-set parser (`parse_set_expr`) already exposes this elegantly. Introducing a parallel typed-mutation surface (`--set key=value`, `--push key item`, etc.) duplicates the language without adding power.

2. **The renderer is authoritative.** S13 made the typed renderer the canonical-form authority. A mutation path that bypasses the typed tree (byte-splicing on source, or XML-to-source direct emission) would have to *agree* with the typed renderer on every formatting decision — a maintenance liability that compounds on every language refactor. Mutation through the typed tree always renders through the same engine.

3. **Synthetic insertions work the same as parsed ones.** S13-Z2 made `ByteRange.anchored` carry "did this come from real source?" Mutation against an anchored tree may produce mixed trees (some synthetic, some anchored); the renderer handles both. Any mutation path that bypasses typed nodes loses this for new content.

## The addressing problem

XPath queries operate on XML (xot) projections of the typed tree. To mutate, we need to map an XPath match back to the typed-tree node that produced it. Three options were considered:

- **Byte-range mapping.** Use the xot node's source position + the typed node's `ByteRange` to find the typed-tree node by overlap. Works for anchored nodes; fragile under batched mutations (ranges shift on the first set) and impossible for synthetic insertions (no source bytes to anchor against).
- **Parallel walk.** Walk xot and typed trees in lockstep, recording correspondences. Order-sensitive; gets brittle when projections introduce wrappers that have no typed antecedent.
- **Node IDs.** Each typed node carries a stable identity. The xot projection knows which typed node it came from. XPath match → ID → typed node is a direct lookup.

**Node IDs win.** They're the only scheme that survives batched mutations and synthetic insertions in one design.

## NodeId design

```rust
pub type NodeId = u32;  // 0 reserved as "unassigned" sentinel
```

Lives on [`Span`](../tractor/src/tree/types.rs), not as a sibling field per variant. Same trick as `ByteRange.anchored` (S13-Z2): pile light metadata into the existing wrapper rather than birthing new fields. Threads through every existing accessor for free.

**Internal in intent, present on xot elements as a side channel.** Stamped onto every xot element as an `@id` attribute during projection so the XPath engine — which returns xot node references — can recover the typed-tree ID from any match without a parallel `HashMap<XotNode, NodeId>` plumbed through the whole engine. Not promoted as a user-facing XPath feature (`//*[@id='42']` is not part of the documented surface), not preserved across format conversions (XML/JSON output for users strips it), not stable across re-parses. The mutation pipeline reads it; nothing else should rely on it.

**Assigned by a post-construction walk**, not during lowering or tree construction. Rationale:

- Construction-time assignment requires deterministic build order; a future parallel-subtree-transform refactor would break it.
- A post-walk is one function, ~50 LOC per tree type (`SyntaxTree`, `DataTree`, `SqlTree`). No call-site churn in lowerings.
- The invariant is one line: **"a tree exiting the parser always has IDs assigned."** Synthetic / mutated trees re-run `assign_ids` at their next merge point. Parallel transforms re-stamp at their join.

**Counter scope:** per-session, starting from 1. IDs are stable across the parse → query → mutate → render lifecycle of one invocation. Not stable across re-parses (the file content has shifted anyway).

## The universal mutation primitive

**Decision: replace the whole subtree.** That's the only mutation operation. No per-variant `set` / `push` / `pop` / `insert` / `remove`. No typed-slot API leaking into the CLI.

- Rename a class → replace its `<name>` subtree.
- Add a method → replace the surrounding `<body>`.
- Delete a list element → replace the parent collection.
- Reorder → replace the parent.

Cost: small changes get verbose (`<name>NewName</name>` instead of `--set name=NewName`). Win: enormous — no per-variant API to design, no `Vec` vs `Box` vs `Option` semantics in the CLI, no edge-case explosion. The functional-update pattern: describe the new state, not the transition.

Today's declarative-set predicate syntax (`class[name='Foo'][returns='int']`) is still the simple-leaf case — the predicate parser flattens it into per-leaf replacements. Big mutations write XML literals.

## Reverse projection: XML → typed tree

Substitution requires *some* way to express "this is the new subtree." Two paths considered:

**A. XML → source directly.** A second source renderer per language, taking xot as input. Lower per-step cost; doubles the renderer maintenance surface (two emitters per language that must agree). Loses validation (errors surface at render time, not at substitution time). **Rejected.**

**B. XML → typed tree → source.** Per-language inverse of `to_xot`. Single renderer (the existing typed one) does the final source emission. Cost: ~500–1000 LOC per language, symmetric to `to_xot`. Gives validation at parse time, unlocks future semantic operations if ever wanted, single source of truth for canonical form. **Chosen.**

A meaningful optimisation within (B): **slot-aware projection.** When replacing a `<name>` subtree, we know the slot expects `SyntaxTree::Name { text }`. The reverse projection doesn't need full variant dispatch; it just reads the expected shape. Most realistic substitutions are slot-aware, so the full-generality XML → typed projection can be deferred until subtree-injection use cases demand it.

## Slice plan

### Slice 1 — `NodeId` + `assign_ids` walker

- Add `id: NodeId` to `Span`. Default `0` ("unassigned"). One-field struct edit, no per-variant churn.
- Define `pub type NodeId = u32`. Newtype later if useful.
- Write `assign_ids(&mut SyntaxTree)`, `assign_ids(&mut DataTree)`, `assign_ids(&mut SqlTree)` — depth-first walks that stamp every node from a fresh counter.
- Wire into the parser exit point (`parse_string_to_xot` or wherever lowering finishes) so the invariant *"every tree exiting the parser has IDs"* holds without callers thinking about it.

### Slice 2 — xot-side IDs + locator

- Project IDs through `to_xot`: each xot element corresponding to a typed node carries that node's ID. Stored via xot's user-data slot if available, or a parallel `HashMap<XotNode, NodeId>` built during projection.
- `find_by_id(&mut SyntaxTree, NodeId) -> Option<&mut SyntaxTree>`. Simple DFS walker. Hashmap upgrade later if needed.
- Equivalents for DataTree, SqlTree.

### Slice 3 — wire SyntaxTree `set` through the locator

Today's `apply_set_mapping` falls through to `apply_set_to_string` (byte splicing) for SyntaxTree-pipeline languages. After this slice:

1. Parse source → typed tree (IDs assigned).
2. Project → xot (IDs propagated).
3. XPath query → matches.
4. For each match: locate ID → `find_by_id` → mutate the matched node's scalar text.
5. Render typed tree → source via the unified renderer.

The CLI surface doesn't change. Same `parse_set_expr`, same predicate syntax. Just: SyntaxTree-pipeline `set` goes through the typed renderer instead of byte splice.

This is enough for every "change one identifier / one literal" use case the current `set` already supports for json/yaml.

### Slice 4 — `replace` CLI verb (API syntax TBD)

Replace an entire subtree by ID with an XML literal. API syntax not yet committed. Open questions:

- How is the XML literal supplied? Stdin? `--xml '...'`? A separate file?
- Does `replace` accept multiple `(xpath, xml)` pairs in one invocation, like declarative set?
- What's the validation message when the XML literal doesn't shape-match the slot?

Defer until 1–3 land.

### Slice 5 — full XML → typed projection

Deferred until **full code synthesis** lands as a roadmap item. The slot-aware projection from slice 4 covers most realistic substitution. Generic XML → typed (a `SyntaxTree`-from-arbitrary-XML parser per language) is only needed when the substitution shape can't be inferred from the parent slot — i.e. when the user is constructing trees that don't exist anywhere in the source.

## Open questions

- **ID stability across batched mutations.** Slice 1's `assign_ids` re-runs from a fresh counter each time. For batched ops that need stable IDs across multiple `set` calls within one transaction, we'd need an "only stamp zeros, preserve existing IDs" variant. Out of scope for slice 1; revisit if a use case demands it.

- **`replace` API syntax.** Open. Captured under slice 4.

- **SqlTree integration.** Slices 1 and 2 now cover SqlTree at parity with SyntaxTree / DataTree: recursive `assign_ids_sql`, recursive `find_by_id_sql`, and `@id`-stamped xot elements (including the inline `<name>` leaves inside `<relation>` / `<reference>` / `<column>` containers). SqlTree's canonical renderer is still mostly placeholders for composite variants (Tier 3 work) — the mutation pipeline for SqlTree works for scalar leaf mutations against an anchored tree, where the byte-slice fast path emits the unchanged source for non-mutated subtrees.

  QuoteStyle has been unified across the three trees: SqlTree's local `{None, Brackets, DoubleQuote, Backtick}` enum was retired and re-exported from the shared `tree::types::QuoteStyle` (now carrying `Plain`, `Single`, `Double`, `TripleSingle`, `TripleDouble`, `Backtick`, `Brackets`, `Raw`, `Heredoc`, `Block`). The shared `write_quoted_scalar` primitive ([Layer A](design-ir-and-renderings.md#34-renderer-engine-layering-whats-shared-across-trees-and-where-it-stops)) drives both SyntaxTree and SqlTree identifier emission.

- **xot user-data vs parallel map.** Need to check whether xot exposes a per-element user-data slot. If not, the parallel `HashMap<XotNode, NodeId>` works fine but adds an extra parameter through the projection.

## Cross-references

- [`docs/design-ir-and-renderings.md`](design-ir-and-renderings.md) — the canonical statement that the tree is the source of truth and renderings are projections. This doc extends that to mutation.
- [`tractor/src/mutation/xpath_upsert.rs`](../tractor/src/mutation/xpath_upsert.rs) — the existing DataTree-only mutation path that this doc generalizes.
- [`tractor/src/mutation/declarative_set.rs`](../tractor/src/mutation/declarative_set.rs) — the predicate-as-declaration parser that's the user-facing syntax for slice 3.
- `TODO.md` § S13 — the renderer work that made typed-tree mutation viable.
