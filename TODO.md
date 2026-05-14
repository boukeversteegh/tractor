# TS → tree → {xot,xml,json,yaml,source} — architectural cleanup

## How to read this

Each slice introduces one or more invariants. Slices are ordered by recommended attack sequence (next up at the top of the open work), but dependencies are explicit per slice — parallel work is encouraged.

**Status markers.** `[ ]` open · `[/]` in progress · `[x]` closed (with a one-line "Done:" note when useful).

**Slice header fields.**
- **Goal.** One sentence. The state of the world the slice creates.
- **Why now.** What's broken / costing us today. References a `Wn` weakness where applicable.
- **Depends on.** Must land first; with reason.
- **Unblocks.** What becomes tractable once this lands.
- **Independent of.** Slices that touch unrelated surfaces — explicit, so parallel work is obvious.
- **Size.** XS / S / M / L / XL plus a one-clause reason.
- **Reversibility.** High / medium / low.
- **Invariants when closed.** Each is checkable against the repo by reading code or running a command.

**Task headlines.** Each task headline is a *checkable invariant* — when the task is closed, the headline reads as a true statement about the codebase. Sub-bullets carry the implementation path and verification.

**Editing this list.**
- Append `??` to a line you want clarified. On the next turn Claude rewrites that single line and removes the `??`.
- Append `++` to a line you want decomposed. Sub-task checkboxes appear underneath; the parent line stays as the tracker.
- Appending -- means it should be less decomposed, more high level
- DROP means we drop this, you can remove it, and so you know that it was removed.
- Reordering or merging happens only on explicit request.

**Choose-one decisions.** When Claude presents options for a decision the user needs to make, the parent line carries an `OPTIONS:` prefix and the options are written as checkbox children. Pick one by marking it `[x]`; that signals approval and Claude proceeds with that option. Mark with `[/]` to start work on the chosen option. Other (unchosen) options stay `[ ]` for context; once the decision is made and acted on, Claude collapses the unchosen ones with a one-line "rejected — <reason>" note (or removes them if the rationale is already captured in the chosen entry).

## Findings — pipeline as it actually runs

Library entry point: `parser::parse(ParseInput, ParseOptions)` (`parser/mod.rs:1106`). Three downstream branches, gated by `use_ir_pipeline(lang, mode)` until S2-Z4 collapses it (`parser/mod.rs:380`):

| Path | Languages | Where |
|---|---|---|
| **tree (programming)** — `SyntaxTree` | csharp, python, java, ts/js/tsx/jsx, rust, go, ruby, php | `parse_with_ir_pipeline_to_xee` |
| **tree (data)** — `DataTree` | json, yaml, toml, ini, env, markdown (Structure mode) | same fn, data branch |
| **tree (sql)** — `SqlTree` | tsql | same fn, sql branch |
| **Legacy imperative** — `XeeBuilder::build_with_options` + `walk_transform` | Raw mode for everything; c, cpp, html, css, bash, scala, lua, haskell, ocaml, r, julia; json/yaml in Data mode | `parser/mod.rs:933` |
| **WASM** — `XotBuilder` + `walk_transform` | All web-app parses | `wasm/mod.rs` |

The tree path renders to xot, **serialises the xot to a string, and re-parses it into xee `Documents`** (acknowledged "v1 stepping stone" at `parser/mod.rs:641`). After that it runs each language's `post_transform` for any remaining shape work plus the `list="X"` attribute pass.

`tractor render` and `tractor set/update`'s value-rewrite use a *different*, parallel reverse pipeline: `render::parse_xml` / `parse_json` → `XmlNode` → `render::render(node, lang, TreeMode::Data, opts)` (`render/mod.rs`). It speaks `XmlNode`, not tree, and supports csharp/json/yaml only.

## Findings — weaknesses (W → slice index)

- **W1.** Three parallel tree shapes, three parallel everything (`SyntaxTree`, `DataTree`, `SqlTree` each with their own `lower_*`, `to_xot`, `to_json`, dispatch arm, projection). → S2 + S5. *(Partial relief landed via the `TreeNode` trait — see S15-Z1/Z2 follow-ups: per-variant traversal knowledge now lives once per tree, not per walker. `assign_ids` and `locator` collapsed by ~1450 LOC combined. Renderer / projection / dispatch still parallel.)*
- **W2.** Module documentation lied about production status (`tree/mod.rs` claimed "experimental sketch on Python"; it's the production path). → S1 (closed).
- **W3.** Reverse path is on the wrong substrate (`render/*.rs` speaks `XmlNode`, supports 3 languages; `tree/source/*.rs` speaks `SyntaxTree`, supports 9, but is unwired). → S4.
- **W4.** Cross-cutting transforms still mutate rendered xot. `chain_inversion` was removed (S3A). What remains in per-language `post_transform.rs` is a mix of language-specific rewrites and shared cross-language helpers parameterised by per-language data. → S3.
- **W5.** WASM and CLI produce different output for migrated languages. → S6 (closed: WASM now routes `Syntax`/`Sql` languages through the typed pipeline; parity verified by `tests/wasm_parity.rs`).
- **W6.** Six places know the language list, none authoritative; alias-mapping (`csharp`/`cs`, `python`/`py`, …) duplicated five ways. → S2.
- **W7.** `XmlNode` mixes XML markup (Element/Text/Comment/PI) with XPath atomic data (Map/Array/Number/Boolean/Null) in one enum. → S8.
- **W8.** Cardinality plumbed twice — typed `Vec<SyntaxTree>` slots *and* `list="X"` attributes on rendered xot. → S3D.
- **W9.** `field_wrappings` and per-language `TractorNode` strum enums are dead-data for migrated languages. → C1, C2.
- **W10.** Old parse API surface (`parse_string_to_xot`, `parse_file_to_xee`, …) still exposed alongside the unified `parse()`. → S9.

The tree design itself (typed slots, byte-range anchoring, `Inline`/`Unknown` escape hatches, coverage audit) is sound — none of these slices refactor it. The three tree families staying separate as types is also fine; the cost only goes away if dispatch + projection + rendering stop being copy-pasted three ways (S2 + S5).

---

## S1 — tree module documentation reflects production reality ✅

**Closed.** `tree/mod.rs` § Status no longer claims "experimental sketch"; it states the production fact.

---

## S2 — One language registry (kills W6) ✅

**Goal.** A single source of truth answers "given this string, what is this language?" — extensions, grammar, tree family, transforms, and vocabulary all live in one row of `LANGUAGES`. Adding a language is one row.

**Why now.** Aliasing (`csharp`/`cs`, `typescript`/`ts`/`tsx`/`jsx`, …) is independently re-listed in five tables: `SUPPORTED_LANGUAGES`, `detect_language`, `get_tree_sitter_language`, `LanguageOps.ids`, `use_ir_pipeline`, plus four match arms in `parse_with_ir_pipeline*`. Concrete failure mode: forget to update one of the six sites when adding a language and the file parses but routes to the wrong pipeline silently.

**Depends on.** Nothing — foundation slice.
**Unblocks.** S3B-Z1b (shared-helpers pipeline needs `LanguageOps` to carry per-language data tables as first-class fields), S3B-Z11 (drop `post_transform` field cleanly), S6 (WASM consults registry), S10A/B (lowering and render-canonical pointers update one row).
**Independent of.** S4, S5, S7, S8, S9.

**Size.** L. Touches `parser/mod.rs` heavily; every language row gets new fields. ~70% complete.
**Reversibility.** Medium. Each Z-step is independently revertible.

**Invariants when closed:**
- `parser::SUPPORTED_LANGUAGES`, `detect_language`, `get_tree_sitter_language` all derive from `LANGUAGES`.
- `parser::use_ir_pipeline` does not exist.
- No `match lang { "csharp" => …, "python" => … }` exists in `parse_with_ir_pipeline*` or `tree/source/mod.rs::render`; all per-language dispatch is registry-keyed.
- `LanguageOps` carries `extensions`, `grammar`, `tree_kind` alongside its existing fields.
- Adding a new language is one row in `LANGUAGES` plus its grammar shim.

### Tasks

- [x] [S2-Z1] **`LanguageOps` carries `extensions`, `grammar`, `tree_kind` for every existing row.** Done — `TreeKind { None | Programming | Data | Sql }` enum + 17 grammar shim fns; TS/TSX/JS split into 3 rows.
- [x] [S2-Z3] **`parser::SUPPORTED_LANGUAGES`, `detect_language`, `get_tree_sitter_language`, `get_language_abi_versions` derive from `LANGUAGES`.** Done — XML kept as one-line passthrough; surfaced `info.rs::LANGUAGES` as a second registry (tracked as C5).
- [x] [S2-Z4] **No language-keyed `match lang { … }` arm exists in `parse_with_ir_pipeline*`; dispatch reads `match l.tree_kind { … }`.**
  - Done 2026-05-08. Reshaped `TreeKind::Data { structure: DataParser, content: DataParser }` to model two tree modes per data lang. Wired Data mode through the tree pipeline; data-branch shape now spec-compliant (no `<object>`/`<array>` wrappers; `list="<key>"` per Principle #12). Companion: collapse the `match lang { … }` in `tree/source/mod.rs::render` as part of S10B.
- [x] [S2C] **`parser::use_ir_pipeline` does not exist; both callers consult `LanguageOps::uses_tree(TreeMode)`.** Done — function deleted; new mode-aware `uses_tree` method on `LanguageOps`.
- [x] [S2-Z5] **Build green; `cargo test --lib` passes after the consolidation.** Done — 373/373.

- ~~S2-Z2~~ **(dropped 2026-05-08)** — c/cpp/html/css/bash/scala/lua/haskell/ocaml/r/julia/xml aren't really supported; don't promote them to `LANGUAGES`.

---

## S3 — Eliminate xot post-passes; tree is canonical from lowering (kills W4 + W8)

**Goal.** No xot post-pass mutates the rendered tree. `lower_<lang>_root` returns canonical tree; `to_xot` is mechanical. Cross-cutting transforms (chain inversion, conditional flattening, slot wrapping, list-tagging) are encoded natively — in the lowering, in typed `SyntaxTree` variants, or in `to_xot` driven by typed slots.

**Why now.** `transform::chain_inversion` (1826 LOC) was deleted in S3A. What remains in per-language `post_transform.rs` is a mix of:
1. **Genuinely language-specific xot rewrites** (e.g. `attach_ir_where_clauses` for C#, `python_restructure_imports` for Python) — these fold into the language's tree lowering.
2. **Shared cross-language helpers parameterised by per-language data** (`collapse_conditionals`, `tag_multi_role_children`, `wrap_expression_positions`, `flatten_nested_paths`, `strip_body_braces`, `wrap_relationship_targets_in_type`, `flatten_single_declarator_children`, `distribute_member_list_attrs`) — the *same algorithms* replicated across nine language post_transforms with only the data lists changing. This is the bulk of what's left.

Cardinality is also still plumbed twice — typed `Vec<SyntaxTree>` slots *and* `list="X"` attributes on rendered xot (W8).

**Strategy.** Two complementary moves, run in parallel:
1. **Per-language fold (Z1a, Z2…Z9).** Each language's *language-specific* passes go into its lowering or new typed `SyntaxTree` variants, the same way S3A did for `SyntaxTree::Access`.
2. **Shared-helper consolidation (Z1b).** The shared cross-language helpers migrate into a single registry-driven post-render pipeline, with their per-language data lists hoisted onto `LanguageOps` as first-class fields. As S3D (typed-slot cardinality) and similar work absorbs each helper's purpose into the tree itself, the corresponding data tables shrink and the global pass progressively retires.

The slice closes when both halves leave nothing standing — no per-language `post_transform.rs` file *and* no global post-render pipeline beyond what `to_xot` does mechanically.

**Depends on.** S2-Z4 (registry-driven dispatch — needed so Z1b's data tables on `LanguageOps` are first-class registry fields).
**Unblocks.** S10 (per-language directories shed `post_transform.rs`).
**Independent of.** S4, S5, S6, S7, S8, S9.

**Size.** XL. Per-language audit across nine languages, plus the new shared-pipeline scaffolding (Z1b is itself a major architectural step), plus progressive retirement as S3D lands.
**Reversibility.** Medium per language; the cross-cutting deletes (S3C, the field removal) are single-commit reverts. Z1b is medium — it introduces new `LanguageOps` fields that are easy to revert but touch every language row.

**Invariants when closed:**
- `transform::chain_inversion`, `transform::conditionals`, `transform::operators`, `transform::generic_type` modules do not exist.
- No `languages/<lang>/post_transform.rs` file exists.
- `LanguageOps::post_transform` field does not exist; `get_post_transform()` does not exist.
- `parse_with_ir_pipeline*` invokes no post-pass on rendered xot — neither per-language nor global.
- `to_xot` emits cardinality from typed `Box<SyntaxTree>` / `Vec<SyntaxTree>` slots; no `list="X"` attribute pass exists.
- `transform::shape_contracts` runtime walks are minimised; provable rules are type-level.

### Tasks

- [x] [S3A] **No `chain_inversion` module exists; every language's lowering constructs left-deep `SyntaxTree::Access` directly from right-deep CST.**
  - Done. `tractor/src/transform/chain_inversion.rs` (1826 LOC) and its tests deleted; all 8 callsites removed; Go/PHP single-step `arr[0]` rewritten to fold into `SyntaxTree::Access`.

- [x] [S3B] **No `languages/<lang>/post_transform.rs` file exists; per-language shape decisions live in the lowering or in typed `SyntaxTree` variants; no global post-render pipeline survives either.**
  - Done 2026-05-08. All 9 per-lang sub-tasks closed; conditionals module deleted; `LanguageOps::post_transform` field removed.

  - [x] [S3B-Z1] **C#: lowering produces canonical tree end-to-end.** Done — closed by Z1a + Z1c.
    - [x] [S3B-Z1a] **C# `where T : ...` constraints attach during lowering; no `attach_ir_where_clauses` post-walk.** Done — `fold_csharp_where_clauses_into_generics` in tree lowering.
    - [ ] [S3B-Z1b] **~~~Shared cross-language post-render helpers run from a single registry-driven pipeline.~~~** Superseded by the delete-and-fix path used for Z1a/Z1c.
    - [x] [S3B-Z1c] **`languages/csharp/post_transform.rs` does not exist.** Done — fixed `csharp_null_forgiving_postfix_unary` by typifying `SyntaxTree::Variable.value: Option<Expression>`.

  - [x] [S3B-Z2] **Rust lowering produces canonical tree; `languages/rust_lang/post_transform.rs` does not exist.** Done — `lower_rust_use` replaces post-walk; ratchet 609→639.
  - [x] [S3B-Z3] **TypeScript/JS/TSX lowering produces canonical tree; `languages/typescript/post_transform.rs` does not exist.** Done — `Expression::wrap` in `lower_ts_declarator_parts` fixes the one breakage.
  - [x] [S3B-Z4] **Python lowering produces canonical tree; `languages/python/post_transform.rs` does not exist.** Done — `set_python_class_member_visibility` in `lower_class`; ratchet 603→605.
  - [x] [S3B-Z5] **Java lowering produces canonical tree; `languages/java/post_transform.rs` does not exist.** Done — `scoped_identifier` recursion + `Expression::wrap` in for/declarator slots; ratchet 600→603.
  - [x] [S3B-Z6] **Go lowering produces canonical tree; `languages/go/post_transform.rs` does not exist.** Done — `Expression::wrap` per returned value in `return_statement`.
  - [x] [S3B-Z7] **T-SQL lowering produces canonical SqlTree; `languages/tsql/post_transform.rs` does not exist.** Done — zero tests broke.
  - [x] [S3B-Z8] **PHP lowering produces canonical tree; `languages/php/post_transform.rs` does not exist.** Done; ratchet 607→609.
  - [x] [S3B-Z9] **Ruby lowering produces canonical tree; `languages/ruby/post_transform.rs` does not exist.** Done — promoted `collapse_conditionals` to unconditional cross-language pass; ratchet 605→607.
  - [x] [S3B-Z10] **The flat conditional shape (`<if><else_if/><else/>`) is produced by lowering or `to_xot`, not by a separate xot walk.** Done — added `lower_ruby_if`; `transform/conditionals.rs` deleted.
  - [x] [S3B-Z11] **`LanguageOps::post_transform` field does not exist; no post-pass runs in the tree pipeline.** Done — field, type alias, and call sites removed.

- [x] [S3C] **`parse_with_ir_pipeline*` invokes no post-pass on rendered xot.** Done — closed by S3B-Z11.

- [x] [S3D] **`to_xot` emits cardinality from typed slots; no `list="X"` attribute pass exists for tree languages.** Done — four no-op post-pass helpers + `ROLE_MIXED_PARENTS` deleted (~130 LOC). Legacy `Rule::Flatten` retained for unmigrated walk_transform langs.

- [ ] [S3E] **`transform::shape_contracts` runtime walks are minimised; provable rules live at the type level.**
  - Most rules become unrepresentable at the `SyntaxTree` enum level (per `tree/types.rs:31`). Keep runtime-only walks for genuinely runtime rules (e.g. `op-marker-matches-text`).

---

## S4 — tree-based reverse rendering (kills W3)

**Goal.** Forward and reverse paths share the same substrate. `tractor render`, `tractor set`, `tractor update` all dispatch through `tree::source::render(tree, lang, anchor)` and work on every language with an tree — no more 3-language ceiling.

**Why now.** The XmlNode-based reverse renderer at `tractor/src/render/` was retired in S4D and `tractor render` now dispatches via `tree::render::render` (S4A done). For `tractor set` / `tractor update`, the json/yaml/yml paths flow through `tree::render::data_json` + `data_yaml` with span tracking (S4B-Z1..Z3 done). The remaining gap is csharp upsert: when S4D deleted the XmlNode renderer it took the csharp upsert path with it, and there's currently no replacement — `mutation/xpath_upsert.rs::lang_supports_upsert` allowlists only `json`/`yaml`/`yml`. Re-enabling csharp (and adding the other SyntaxTree languages) needs `SyntaxTree` mutation primitives + a span-tracking renderer, which is the same machinery S13 (unified renderer) is building.

**Depends on.** Nothing structural — `tree/source/render` already exists.
**Unblocks.** Removes `render/` entirely; reduces ~1800 LOC of XML→source code that duplicates the tree-side work.
**Independent of.** S2, S3, S5, S6, S7, S8, S9.

**Size.** M. Two callers (`cli/render.rs`, `mutation/xpath_upsert.rs`) switch over; ~1800 LOC of dead code follows.
**Reversibility.** High per task. Easy to leave the old renderers behind a feature flag if needed.

**Invariants when closed:**
- `tractor render` works for every language with an tree (9 today, growing as tree coverage expands).
- `tractor set` / `tractor update` work for every tree-supported language.
- `render::parse_xml` and `render::parse_json` do not exist.
- `render/{csharp,json,yaml}.rs` do not exist.
- The reverse path does not re-parse tractor's own output back through XML.

### Tasks

- [x] [S4A] **`tractor render` reads source, parses to tree, and emits via `tree::source::render(tree, lang, anchor)`.** Done — `cli/render.rs` dispatches by tree family; anchored mode byte-identical; non-tree languages return a clear error.

- [ ] [S4B] **`mutation/xpath_upsert.rs` value-rewrite uses anchored tree re-render with span tracking, not `render_with_spans(xml_node, lang, TreeMode::Data, …)`.**
  - Today: `mutation/xpath_upsert.rs` calls `render::render_with_spans` 9× over a `XmlNode` derived from xot. Works today because of transitional shims in `render::json`/`render::yaml` (`is_property_element` accepts `field`/`list` or `name != "item"`; `property_key` priority adds `list`).
  - After: mutation finds the matched tree node by byte position, mutates `SyntaxTree`/`DataTree` directly, re-renders the modified subtree via an tree-aware span-tracking renderer, splices into original source.
  - Decomposes into:
    - [x] [S4B-Z1] **DataTree mutation primitives.** Done — `ScalarKind`, `synthetic_scalar`, `find_at_offset[_mut]`, `set_scalar`, `set_pair_value`, `insert_nested_pair` on `DataTree`; 7 unit tests.
    - [x] [S4B-Z2] **tree-aware span-tracking render for `DataTree`.** Done — `tree/source/data_json.rs` and `tree/source/data_yaml.rs` expose `render_*_with_spans -> (String, DataSpanMap)` keyed by `(line, col)`; 14 unit tests including end-to-end mutate→render→splice roundtrip.
    - [x] [S4B-Z3] **Wire upsert json/yaml paths.** Done — `mutation/xpath_upsert.rs` `update_existing` and `insert_new` branch to `*_via_data_ir` helpers for json/yaml/yml. Insert path uses two-phase `find_insertion_target_at_offset` (deepest-container, not deepest-leaf).
    - [x] [S4B-Z4] **SyntaxTree upsert (csharp + all 7 other tree-pipeline languages).** Done via S15-Z3 (Slice 3, 2026-05-14) — `mutation::xpath_upsert::update_existing_via_syntax_ir` routes all 8 SyntaxTree-pipeline languages (csharp / java / python / typescript / rust / go / ruby / php) through the typed pipeline. Pipeline: parse → typed tree (IDs assigned via `assign_ids_syntax`) → project to xot (`@id` stamped) → XPath query → `Match.node_id` → `find_by_id` (generic over `TreeNode`) → `set_scalar_text` → per-leaf `render(&leaf, lang, None)` → splice into source at the match's byte range. Mutation is **update-only**; structural insertion (creating missing path segments) lands with Slice 4 / `replace` verb (S15-Z4).
    - [x] [S4B-Z5] **Retire transitional shims in `render::json`/`render::yaml`.** Done — strict `field=` checks restored; `render::yaml::tests::sequence` rewritten to legacy shape. (Modules deleted entirely at S4D shortly after.)

- [x] [S4C] **`render::parse_xml` and `render::parse_json` do not exist.** Done — removed alongside S4D.

- [x] [S4D] **`render/{csharp,json,yaml}.rs` do not exist (~1800 LOC deleted).** Done — whole `tractor/src/render/` directory retired (~2200 LOC); `xpath_upsert.rs` slimmed in lockstep; new `lang_supports_upsert` allowlist (json/yaml/yml).

---

## S6 — WASM uses the unified parse (kills W5)

**Goal.** The web playground and the CLI produce *byte-identical* semantic XML for the same source on every migrated language. WASM runs the typed-tree pipeline (lower → SyntaxTree → render_to_xot), not `XotBuilder` + `walk_transform`. The imperative transform machinery retires once unused.

**Why now.** `wasm/mod.rs` imports `XotBuilder` + `walk_transform` + `get_transform` and never touches `crate::tree`. The web playground (`web/src/tractor.ts`) calls into it. Since the tree migration changed the semantic XML shape for migrated languages, the playground silently produces *different* output than the CLI for the same source — queries that work in one fail in the other.

**Architectural framing (added 2026-05-14).** The imperative pipeline had a clean layering property: tree-sitter was *one producer* of raw xot, and everything semantic happened on xot. That made parsing source pluggable — native tree-sitter, JSON-from-`web-tree-sitter`, hand-crafted XML — all flowed into the same transform chain. The typed pipeline collapsed that layer by having `lower_<lang>_root(root: tree_sitter::Node, source: &str)` reach into `tree_sitter::Node`'s native Rust API directly. By loosening the dependency on xot (as semantic substrate), we tightened the dependency on the tree-sitter Rust crate. WASM doesn't link tree-sitter Rust — that's why this slice exists. The fix restores the "tree-sitter is one producer" property at the lowering input layer, the same way `XotBuilder` already does at the raw-xot layer.

**Depends on.** S2 (registry-driven dispatch makes the WASM-side fan-out one place).
**Unblocks.** WASM playground accuracy — currently a quiet correctness bug for users. Retirement of the imperative transform machinery (`walk_transform`, per-language `transform.rs`, `apply_field_wrappings`, dual-entry `XotBuilder`).
**Independent of.** S3, S4, S5, S7, S8, S9.

**Size.** M–L. Mechanical type/signature migration across 9 lowerings (csharp / java / python / typescript / rust / go / ruby / php / tsql) + 5 DataTree lowerings, plus the 11 `.parent()` call sites that need refactoring.
**Reversibility.** Medium. Each language migration is independently revertable, but once the WASM path flips and the imperative machinery is deleted, going back is a re-introduction.

**Invariants when closed:**
- `wasm::parse_to_xml` does not import `XotBuilder` or `walk_transform`.
- The web playground produces identical output to `tractor -x` on the same source for every migrated language (verified by S6A fixtures).
- `walk_transform`, `apply_field_wrappings`, per-language `TransformFn`, and `XotBuilder::build_raw_from_serialized` are deleted (or routed through `RawNode` adapters).

### Plan: `RawNode` as shared lowering input

The chosen approach (debated 2026-05-14): introduce a tractor-owned `RawNode` type that serves as the lowering input on **both** native and WASM. Tree-sitter goes back to being *one producer* of `RawNode`, alongside JSON-from-`web-tree-sitter`.

```
                                      ┌──► tree_sitter::Node ──► RawNode::from_tree_sitter ──┐
parsing source ─►  RawNode  ◄─────────┤                                                       ├─► lower_<lang>_root(&RawNode, source) ──► SyntaxTree ──► render_to_xot ──► xot
                                      └──► web-tree-sitter (JS) ──► JSON ──► serde ──────────┘
```

`RawNode` is a small owned struct with the methods `lower_*` actually calls: `kind`, `is_named`, `byte_range`, `start_byte`/`end_byte`, `start_position`/`end_position`, `utf8_text(source)`, `children()`, `named_children()`, `child_by_field_name(name)`, `field_name`. Serde-derived JSON shape (already mostly defined by `wasm/ast.rs:SerializedNode` — that type gets promoted to `tractor/src/raw.rs:RawNode`). The boundary between TypeScript and Rust is JSON conforming to RawNode's shape; no Rust trait crosses the language line.

**Why not the trait alternative.** A `TsNodeLike` trait abstracted over `tree_sitter::Node` and `&SerializedNode` was considered. Rejected because (a) Rust traits don't cross language boundaries, so the JS side needs a JSON shape regardless, making the trait redundant once `RawNode` exists; (b) generics ripple through every lowering signature; (c) the API surface is small enough (~10 methods) that hand-mirroring is cheaper than the generic machinery.

**Why not source generation.** Considered and rejected. The differences between `tree_sitter::Node` and `RawNode` are *representational* (Copy vs !Copy, lifetime vs owned, `Option<Node>` vs `Option<&RawNode>`, cursor vs no-cursor children API, `Result<&str>` vs `&str` for `utf8_text`) — not nominal. Generation can normalize method names but cannot close semantic gaps. Plus `tree_sitter::Node` lives in a foreign crate; macro-introspection isn't available.

**Mechanical friction points** (one-time grep/sed across the lowerings):
- Drop `let mut cursor = node.walk();` lines before `children(&mut cursor)` calls; iterate `node.children()` directly.
- Remove `?` / `.unwrap()` on `utf8_text(source)` since `RawNode` returns `&str` directly.
- Adjust references where `tree_sitter::Node` was copied (it's `Copy`; `&RawNode` is not).
- Refactor 11 `.parent()` call sites across csharp/go/java/python to thread parent context through descent (parent-less tree).

### Tasks

- [x] [S6A] **A fixture set captures WASM↔CLI parity for every migrated language.**
  - Landed at `tractor/tests/wasm_parity.rs`. Builds the AST natively via tree-sitter, simulates the WASM JSON serialisation step (RawNode → SerializedNode JSON round-trip), then routes both paths through `parse_ast_to_xml` and compares against `parse_string_to_xot` + `render_document`. 9 fixtures (one per migrated programming language family, including T-SQL) pass; run with `cargo test -p tractor --features wasm --test wasm_parity`.

- [x] [S6B-RawNode] **Introduced `tractor/src/raw.rs:RawNode`.** Owned struct with `kind`, `is_named`, `byte_range`, `start_byte`/`end_byte`, `start_position`/`end_position` (returning `raw::Point`), `utf8_text(source)→&str`, `children()`, `named_children()`, `child_by_field_name`, `field_name`, plus `id()` (pointer identity, replaces the 24 `tree_sitter::Node::id()` call sites) and `field_name_for_child(u32)` (mirrors tree-sitter's API for the few callers that index into children). Native-only `RawNode::from_tree_sitter(node, source)` walks via cursor to capture per-child field names.

- [x] [S6B-Pilot+Rest] **Migrated all 14 lowerings to `&RawNode`.** Mechanical refactor scripted via `scripts/migrate_to_rawnode.py` (regex rewrites for `TsNode<'_>`/`Option<TsNode>`/`Vec<TsNode>` types, cursor declarations, children/named_children calls). Hand-fixes: csharp's 8 `utf8_text(source.as_bytes())` Result-shaped call sites, csharp/java's 4 `.parent()` walk-up sites (refactored to a `thread_local!` parent-map populated at the root of each `lower_*_root` call, scoped to the lowering — see `enclosing_type_kind` in csharp/java's `lower.rs`). Lowerings are no longer gated on `feature = "native"`.

- [x] [S6B-WASM] **WASM crossing wired through the typed pipeline.** `wasm/mod.rs:parse_ast_to_xml` now routes `TreeKind::Syntax` / `TreeKind::Sql` languages through `serde_json::from_str::<RawNode>` → `lower(&raw, source)` → `assign_ids_*` → `render_to_xot` / `render_sql_to_xot` → `render_document`. Both `parse_to_xml` and `get_schema_tree` use the same path. Data-language modes and `TreeKind::None` languages fall back to the legacy `XotBuilder + walk_transform` path; that fallback is what S6B-Retire still has to address.

- [ ] [S6B-Retire] **Delete the imperative transform machinery.** Blocked on **S6R** (typed passthrough for `TreeKind::None` languages) and on ungating the data-tree path for WASM. Once both land, `walk_transform`, `apply_field_wrappings`, per-language `transform.rs`, and `XotBuilder::build_raw_from_serialized` have no callers and can be deleted.

- [x] [S6C] **The S6A fixtures pass: WASM↔CLI divergence is zero for every migrated language.** Verified by the 9-test `wasm_parity` suite; all green.

---

## S6R — Typed passthrough for unmigrated languages (unblocks S6B-Retire)

**Goal.** Every language flows through the typed pipeline. Languages without a hand-written semantic lowering get a generic `lower_raw_passthrough` that wraps each CST node in a single `Raw` variant — close to the bare CST, no semantic decisions. This kills the last consumers of `walk_transform` + `XotBuilder` and unblocks S6B-Retire.

**Why now.** After S6, the typed pipeline covers `TreeKind::Syntax` and `TreeKind::Sql`. Eleven languages (HTML, CSS, C, C++, bash, scala, lua, haskell, ocaml, r, julia) still route through `walk_transform` because they have no typed lowering. Data languages on WASM also fall back to the imperative path (separate issue — see follow-up below). As long as anything hits `walk_transform`, the legacy machinery can't be deleted.

The user's intent (recorded 2026-05-14): "stick as close to raw as possible, only introduce types when we're explicitly modeling structure." Typing the passthrough makes it testable like the rest of the pipeline; field-wrappings and other semantic shape decisions stay opt-in (a language gets them when someone writes a real `lower_<lang>_root`).

**Depends on.** S6 (RawNode + typed pipeline + registry-driven dispatch) — landed.
**Unblocks.** S6B-Retire (delete imperative machinery).
**Independent of.** S7, S8, S9.

**Size.** S. One new variant + one passthrough lowering per tree family + registry updates for 11 rows.
**Reversibility.** High. Adding a variant is additive; the passthrough can be swapped for `TreeKind::None` if it doesn't work out.

**Invariants when closed:**
- `SyntaxTree::Raw { kind, children, range, span }` (or equivalent) exists and renders as `<kind>{children…}</kind>`.
- `lower_raw_passthrough(&RawNode, &str) -> SyntaxTree` exists and is registered for every previously-`TreeKind::None` row in `LANGUAGES`.
- No `LanguageOps` row carries `TreeKind::None` on the native build.
- `apply_field_wrappings` is not called for passthrough languages (conscious behaviour change — see below).

### Behaviour change to accept

Passthrough languages currently get `apply_field_wrappings` from the imperative path (children of certain kinds wrapped in `<name>`/`<body>`/etc.). The typed passthrough deliberately omits this — wrappings are a per-language semantic shape decision that belongs in a real `lower_<lang>_root`, not in a generic catch-all. Net effect: XML for HTML/CSS/C/C++/etc. drops the field-wrap layer. These languages are barely used in queries today; the simplification is worth the small shape shift.

### Tasks

- [ ] [S6R-Variant] **A `SyntaxTree::Raw` variant exists with a `to_xot` arm.**
  - Renders `<kind>{children…}</kind>`. Add to `tree::types`, `tree::to_xot`, `tree::to_json`, `tree::render::*`. Treat `kind` as a `Cow<'static, str>` or `String` (the grammar kind names aren't a closed set across all languages).
  - Add the matching `TreeNode::children`/`children_mut` arm so `assign_ids` walks it.

- [ ] [S6R-Lower] **`lower_raw_passthrough` exists for `SyntaxTree` and produces the `Raw` variant recursively.**
  - Walks named children only (or all children — pick to match the prior imperative output as closely as possible).
  - Lives at `tree::lower_raw_passthrough` so it's reachable from the registry without a per-language file.

- [ ] [S6R-Register] **Every `TreeKind::None` row in `LANGUAGES` now carries `TreeKind::Syntax(lower_raw_passthrough)`.**
  - HTML, CSS, C, C++, bash, scala, lua, haskell, ocaml, r, julia.

- [ ] [S6R-Tests] **Snapshot or property tests cover the passthrough output for at least three of the new languages.**
  - Cheap inputs (`<div>x</div>`, `body { color: red; }`, `int main(){}`). Goal is to lock the shape so future refactors can't silently break it.

### Follow-up (not in this slice)

WASM-side data-language parsing still falls back to the imperative path because `tree::data` and the data renderers are `#[cfg(feature = "native")]`-gated. Ungating them is its own slice — call it **S6D**. With S6R + S6D landed, S6B-Retire's invariants hold and the imperative machinery can go.

---

## S7 — Drop xot serialise/reparse (kills the "v1 stepping stone" tax)

**Goal.** The tree pipeline constructs xee `Documents` directly from xot, with no intermediate XML serialisation + reparse step.

**Why now.** The tree pipeline renders the tree to a fresh `xot::Xot`, calls `xot.to_string(...)` to serialise it to XML text, then feeds that string to `documents.add_string(...)` so xee re-parses it into a queryable `Documents`. Acknowledged "v1 stepping stone" in code at `parser/mod.rs:641`. Pays a serialise + parse cost on every parse and discards the typed in-memory tree just to reconstruct it.

**Depends on.** Nothing structural; may need an upstream change to xee.
**Unblocks.** Per-parse latency (no serialise + parse), simpler memory profile.
**Independent of.** Other slices.

**Size.** S in code; potentially L if xee needs an upstream change.
**Reversibility.** High. Self-contained in `parser/mod.rs`.

**Invariants when closed:**
- The `xot.to_string(...) → documents.add_string(...)` block at `parser/mod.rs:776` (and the matching ones in data/SQL branches) does not exist.
- xee `Documents` can ingest an existing `xot::Xot` + node handle directly.

### Tasks

- [ ] [S7A] **A clear yes/no answer exists on whether xee `Documents` can ingest an existing `xot::Xot` + node handle today.**
  - Documented at the top of S7B's PR. May require xee upstream change.

- [ ] [S7B] **Direct tree → xee Documents construction is implemented.**
  - Replaces the `xot::Xot::new()` → render → `to_string()` → `documents.add_string()` chain.

- [ ] [S7C] **The `xot.to_string(...) → documents.add_string(...)` block at `parser/mod.rs:776` does not exist.**
  - Depends on S7B.

---

## S8 — Split `XmlNode` from XPath atomic data (kills W7)

**Goal.** `XmlNode` represents XML markup only (Element / Text / Comment / PI). XPath atomic and structured values (Map / Array / Number / Boolean / Null) live in a separate `XpathValue` type. Renderers no longer pattern-match across two abstractions in one enum.

**Why now.** `xpath::XmlNode` glues two unrelated abstractions into one enum (`xpath/match_result.rs:20`). Every renderer that walks a match has to branch on whether the variant is markup or a value. Type signatures lie about what a function accepts.

**Depends on.** Nothing structural.
**Unblocks.** Cleaner renderer signatures; precondition for further `Tree` simplification.
**Independent of.** Other slices.

**Size.** S–M. A type split + every callsite update.
**Reversibility.** Medium. Touches public API.

**Invariants when closed:**
- `XmlNode::{Map, Array, Number, Boolean, Null}` variants do not exist.
- `Tree` variant set is `Tree::Xml(XmlMarkup) | Tree::Atom(XpathValue) | Tree::SyntaxTree{..} | Tree::DataTree{..} | Tree::SqlTree{..}`.
- No renderer pattern-matches across markup-and-atom in one expression.

### Tasks

- [ ] [S8A] **`XmlMarkup` and `XpathValue` types exist; `XmlNode` is `XmlMarkup` only.**

- [ ] [S8B] **`Tree` is `Tree::Xml(XmlMarkup) | Tree::Atom(XpathValue) | Tree::SyntaxTree{..} | Tree::DataTree{..} | Tree::SqlTree{..}`.**

- [ ] [S8C] **No renderer branches on `XmlNode::{Map, Array, Number, Boolean, Null}`.**
  - Likely sites: `format/json.rs`, `format/xml.rs`, `output/*`.

---

## S9 — Retire the old parse API surface (kills W10)

**Goal.** The library exposes one parse function: `parse(ParseInput, ParseOptions)`. The legacy multi-function API is gone.

**Why now.** `parser/mod.rs` exposes a unified `parse()` *and* the legacy multi-function API (`parse_string_to_xot`, `parse_file_to_xot`, `parse_string_to_xee`, `parse_file_to_xee`, `load_xml_string_to_documents`, `load_xml_file_to_documents`, plus `*_with_options` variants). All re-exported from `lib.rs`; tests still call them. The "one principled parse entry point" comment at `parser/mod.rs:1091` is intent, not fact.

**Depends on.** Nothing.
**Unblocks.** Library API stability.
**Independent of.** Other slices.

**Size.** S. Mechanical migration + delete.
**Reversibility.** High in test code; medium for the public API removal.

**Invariants when closed:**
- No caller in the repo uses `parse_string_to_xot`, `parse_file_to_xee`, etc.
- `lib.rs` re-exports only `parse()`.
- The `parse_*_with_options` functions do not exist in `parser/mod.rs`.

### Tasks

- [ ] [S9A] **No test calls `parse_string_to_xot` / `parse_file_to_xee` / their variants.**
  - Files: `tests/ir_csharp_parity.rs`, `tests/ir_python_parity.rs`, `tests/ir_python_blueprint.rs`, `tests/ir_*_missing_kinds.rs`, `tests/coverage_report.rs`.

- [ ] [S9B] **`lib.rs` does not re-export the legacy parse functions.**
  - Targets at `lib.rs:87–99`. Depends on S9A.

- [ ] [S9C] **`parse_*_with_options` functions do not exist in `parser/mod.rs`; only `parse()` remains.**
  - Depends on S9B.

---

## S10 — Per-language directory consolidation

**Goal.** All language-specific code for one language lives in one directory: `languages/<lang>/` carries the kinds, vocabulary, lowering, and canonical-source emitter. The shared tree machinery is small and clearly demarcated under `tree/`.

**Why now.** Today, language-specific code is split across two top-level directories — `languages/<lang>/` for kinds + vocabulary + post-transform, and `tree/<lang>.rs` (1.5–3K LOC each) + `tree/source/<lang>.rs` for the lowering and canonical emitter. The split reflects the tree migration history (the new pipeline grew sideways under `tree/`), not current ownership. The principle "languages decide what transformations they use, nothing forced top-down" survives if and only if the per-language code physically clusters.

The shared tree machinery — `tree/types.rs` (the unified `SyntaxTree` enum), `tree/to_xot.rs`, `tree/to_json.rs`, `tree/to_data.rs`, `tree/coverage.rs`, `tree/lower_helpers.rs`, `tree/source/common.rs` (the `Syntax` + `write_ir` engine) — is language-agnostic by design and stays in `tree/`. The cross-language unification at the tree layer is the architectural commitment of the design and isn't undone by this reorg. What changes is *where the per-language code physically lives*.

**Depends on.** S2-Z4 (registry pointers in place — the moves update one row each), and ideally S3B-Z1c (so the moved directories don't carry `post_transform.rs` files about to be deleted).
**Unblocks.** A reader can answer "show me all csharp-specific code" with one `ls`.
**Independent of.** S4, S5, S6, S7, S8, S9 — they don't read these paths.

**Size.** L. ~14 KLOC of file moves across 8 languages × 2 file types, plus directory regrouping for data-tree (9 files) and sql-tree (5 files).
**Reversibility.** High. Pure file moves with import updates; one-commit revert.

**Invariants when closed:**
- `tree/<lang>.rs` does not exist for any programming language; `languages/<lang>/lower.rs` exists instead.
- `tree/source/<lang>.rs` does not exist for any programming language; `languages/<lang>/render_source.rs` exists instead.
- `tree/data/` is a directory containing all `DataTree` types, lowering, and renderers.
- `tree/sql/` is a directory containing all `SqlTree` types and renderers (T-SQL lowering is at `languages/tsql/lower.rs`, consistent with S10A).
- Per-language `input.rs` is `kinds.rs`; per-language `output.rs` is `vocabulary.rs` (or fold into `mod.rs`).
- `tree/source/` (post-rename: `tree/render/`) holds only `common.rs` + `mod.rs`.

### Tasks

- [x] [S10A] **No `tree/<lang>.rs` lowering file exists for any programming language; `languages/<lang>/lower.rs` exists in its place.**
  - Each move: `git mv tree/<lang>.rs languages/<lang>/lower.rs`; add `pub mod lower;` to `languages/<lang>/mod.rs`; drop `pub mod <lang>;` from `tree/mod.rs`; update the `tree_kind: Syntax(<lang>::lower::lower_<lang>_root)` pointer in the `LANGUAGES` registry.
  - Per-language sub-tasks:
    - [x] [S10A-Z1] csharp — `tree/csharp.rs` (2886 LOC) → `languages/csharp/lower.rs`.
    - [x] [S10A-Z2] python — `tree/python.rs` (2238 LOC) → `languages/python/lower.rs`.
    - [x] [S10A-Z3] java — `tree/java.rs` (2053 LOC) → `languages/java/lower.rs`.
    - [x] [S10A-Z4] typescript — `tree/typescript.rs` (2079 LOC) → `languages/typescript/lower.rs` (covers ts/tsx/js/jsx).
    - [x] [S10A-Z5] rust_lang — `tree/rust_lang.rs` (1967 LOC) → `languages/rust_lang/lower.rs`.
    - [x] [S10A-Z6] go — `tree/go_lang.rs` (1507 LOC) → `languages/go/lower.rs`.
    - [x] [S10A-Z7] ruby — `tree/ruby.rs` (688 LOC) → `languages/ruby/lower.rs`.
    - [x] [S10A-Z8] php — `tree/php.rs` (1563 LOC) → `languages/php/lower.rs`.

- [x] [S10B] **No `tree/render/<lang>.rs` per-language emitter exists; `languages/<lang>/render_source.rs` exists in its place.** Closed by S13-Z7: the unified dispatcher in `tree/render/mod.rs::render` consumes each language's `syntax()` config; the per-language `match lang` fork is gone, so the originally-deferred `render_canonical` registry field is moot — there's no dispatch left to convert.
  - Per-language sub-tasks: [S10B-Z1..Z8] csharp / java / python / typescript / rust_lang / go / ruby / php — all moved.
  - SQL handled by S10D.

- [x] [S10C] **`tree/data/` is a directory; the nine flat `data*.rs` / `*_data.rs` files at `tree/` root do not exist.**
  - Today's flat layout: `data.rs` (226), `data_to_xot.rs` (501), `data_to_json.rs` (222), `to_data.rs` (465 — `SyntaxTree → DataTree` projection, stays at tree root), `json_data.rs` (187), `yaml_data.rs` (265), `toml_data.rs` (367), `ini_data.rs` (152), `markdown_data.rs` (324).
  - Target: `tree/data/{types.rs, to_xot.rs, to_json.rs, lower_json.rs, lower_yaml.rs, lower_toml.rs, lower_ini.rs, lower_markdown.rs}`.

- [x] [S10D] **`tree/sql/` is a directory; the five flat `sql*.rs` files at `tree/` root and `tree/source/sql.rs` do not exist. T-SQL lowering lives at `languages/tsql/lower.rs`.**
  - Today's flat layout: `sql.rs` (741), `sql_lower.rs` (2243), `sql_to_xot.rs` (908), `sql_to_json.rs` (684), `tree/source/sql.rs` (147).
  - Target: `tree/sql/{types.rs, to_xot.rs, to_json.rs, render_source.rs}` + `languages/tsql/lower.rs`.

- [x] [S10E] **No `languages/<lang>/input.rs` file exists; `kinds.rs` exists in its place.**
  - Files contain only the generated `CsKind` / `PyKind` / `JavaKind` / etc. enum (CST-kind catalogue), used by `tests/kind_catalogue.rs` and the `SyntaxTree::Unknown` audit. The "input" name dates from the retired imperative pipeline.
  - Update `task gen:kinds` codegen to write `kinds.rs`. Update test imports.

- [ ] [S10F] **No `languages/<lang>/output.rs` file exists; `vocabulary.rs` exists in its place (or contents are folded into `mod.rs`).**
  - Cross-reference C2: if C2 chooses (a) "drive shape contracts off `SyntaxTree` variants alone and delete `TractorNode`", S10F is moot — delete the files instead.

- [x] [S10G] **`tree/source/` does not exist; `tree/render/` exists in its place (or `tree/source/mod.rs::render` is gone entirely if S10B's registry field replaces it).**

- [ ] [S10H] **Either `transform/` has a clear sole-purpose role (with a name that matches), or it is gone.**
  - After S3A + S3B + S3D + S3E + C3, the survivors are: `walk_transform`, `apply_field_wrappings`, possibly `singletons.rs`, possibly `builder.rs`. These serve only the legacy `XeeBuilder` path (data languages JSON/YAML's syntax branch + Raw mode + WASM until S6).
  - Decide: rename `transform/` to `legacy_xot/` (or similar) to mark it non-tree, or delete what's actually dead.
  - Depends on S3, S6, C3.

---

## S11 — tree-as-data-shape: drop accidental wrappers, normalize fields (kills the `to_data.rs` boilerplate at the source)

**Goal.** The `SyntaxTree` variants ARE the data shape. Reading `SyntaxTree::If` tells you exactly what the JSON output looks like. Wrappers exist only when the data view needs them; XML-only structural wrappers (`<body>`, `<expression>`, `<decorator>`) get inserted at xot-render time, not stored in tree. Field names match output keys. Boolean fields become marker arrays. Operator text+marker collapses to enum.

**Why now.** S5A finished projection coverage with ~700 LOC of mechanically-similar match arms. Audit (2026-05-09 conversation) showed most of the per-variant logic encodes accidental tree-shape ↔ data-shape mismatches, not real domain distinctions. The boilerplate-reduction OPTIONS under S5A (macro / MappingBuilder / per-variant methods) all paper over those mismatches. Fixing them at the tree layer makes the data projection trivial AND simplifies queries (the tree is the answer to "what shape is the data?", not a separate Rust-ergonomic shape).

**Depends on.** S5A-Z7 (exhaustiveness landed) + S5C (caller wired) — both done. Each Z step is independently mergeable.
**Unblocks.** Replaces S5A's "OPTIONS: Boilerplate reduction" with a structural fix; drops `to_data.rs` from ~700 LOC → ~150 LOC of genuine special cases (atoms, sequence vs mapping shape).
**Independent of.** S2 (registry), S6 (WASM parity), S7 (xee-direct), S8 (XmlNode split), S9 (parse API).

**Size.** L. ~10 surgical tree refactors, each touching every language's lowering. Each Z is small but the slice is wide.
**Reversibility.** Per-Z high (revert one wrapper-removal commit). Slice-level medium — once committed, queries that target the dropped wrappers break.

**OPTIONS: Scope of the slice.** Pick one (mark `[x]`).

- [ ] **(a) Full tree redesign + matching XML shape changes.** Drop wrappers from tree AND from xot output. Pro: cleanest end state — tree, XML, and JSON all share one shape. Con: breaks every external XPath query that targets `//body` / `//expression` / `//decorator`; snapshots shift across XML, JSON, tree-text. Months of work + downstream coordination.
- [ ] **(b) tree redesign with XML stability preserved (recommended).** Drop wrappers from tree; have `to_xot` re-insert them at render time so XML output stays bit-identical. Pro: no XPath query breakage; gain "tree == data shape" principle; data-side projection collapses. Con: `to_xot` grows the wrapping logic that used to live in tree variants — the wrapping moves but doesn't disappear. Net LOC about flat; cleanly partitioned.
- [ ] **(c) Stay the course (S5A's OPTIONS).** Keep tree structure; reduce data-projection boilerplate via macro/builder/per-variant-method. Pro: small, contained. Con: never resolves the tree-shape ↔ data-shape mismatch; future variants keep adding boilerplate.

**Invariants when closed (under option b):**
- No `SyntaxTree::Body`, `SyntaxTree::Expression`, `SyntaxTree::Decorator`, `SyntaxTree::FieldWrap`, `SyntaxTree::Aliased` variants exist.
- `Modifiers` struct does not exist; modifiers are `Vec<Modifier>` (enum).
- No `is_X: bool` fields on `SyntaxTree` variants; bool flags become marker entries.
- `SyntaxTree::Binary` / `Unary` / `Comparison` carry `op: BinaryOp` (enum) instead of `op_text: String + op_marker: &'static str`.
- tree field names match JSON key names (`if_true → then_`, `iterables → right`, etc.).
- `to_data::project` is < 200 LOC: scalar atoms, sequence-shape (anonymous-vec variants), mapping-shape (everything else, fields → pairs via mechanical convention).
- XML output (`to_xot`) bit-identical to pre-slice for every blueprint — wrappers re-inserted at render time.

### Tasks

Listed roughly by blast radius (smaller first). Each Z step:
- Updates `SyntaxTree` types (one variant or one struct).
- Updates every language's lowering site that constructs the affected variant.
- Updates `to_xot` to insert the moved wrapper at render time (option b only).
- Updates `to_data::project` to drop the now-unnecessary special case.
- Verifies XML snapshots unchanged; JSON snapshots may shift (those are the wins).

- [ ] [S11-Z1] **No `is_X: bool` fields on `SyntaxTree` variants; bool flags carry through as `&'static str` markers in a `markers: Vec<&'static str>` slot or equivalent.**
  - Targets: `SyntaxTree::For.is_async`, `SyntaxTree::Foreach` (the `in` flag is already a marker), `SyntaxTree::From.relative`, `SyntaxTree::Import.has_alias`, `SyntaxTree::FromImport.has_alias`, `SyntaxTree::Namespace.file_scoped`, `SyntaxTree::Using.is_static`, `SyntaxTree::Comment.{leading, trailing}`. Replace each with marker-list membership; lowering sites push `"async"` / `"relative"` / `"alias"` / `"file"` / `"static"` / `"leading"` / `"trailing"` literally instead of toggling a bool.
  - Smallest blast radius — purely additive on the tree side; `to_data::project` arms drop their `if *is_async { … }` branches and read the marker list mechanically.

- [ ] [S11-Z2] **`Modifiers` struct does not exist; modifiers are `Vec<Modifier>` where `Modifier` is an enum.**
  - `enum Modifier { Public, Private, Protected, Internal, Static, Async, Override, Abstract, Sealed, Const, Readonly, … }`. `SyntaxTree::Class.modifiers: Vec<Modifier>`. Lowering sites push enum values directly; no `Modifiers::default()` + setter dance.
  - `to_data::project` arms drop `push_modifier_flags`; modifier projection becomes `for m in modifiers { pairs.push(make_flag(m.as_str(), …)) }` — but even that collapses into the generic Vec-of-enum convention.
  - Larger blast: every declaration variant + every lang's lowering touches `Modifiers`.

- [ ] [S11-Z3] **`SyntaxTree::Decorator` does not exist; `decorators: Vec<SyntaxTree>` slots carry expressions directly.**
  - Lowering sites that construct `Decorator { inner: x }` simplify to pushing `x` itself into the `decorators` slot. `to_xot` wraps each decorator child in `<decorator>` at render time (option b) or drops the wrapper (option a).
  - Probably the cleanest single-variant removal — Decorator is purely a positional wrapper.

- [ ] [S11-Z4] **`SyntaxTree::FieldWrap` does not exist.** Pure parity-track holdover; remove the variant. Each construction site rewrites to express the wrapped slot via the parent's typed field. `to_xot` keeps the original element name when applicable.

- [ ] [S11-Z5] **`SyntaxTree::Aliased` does not exist; aliasing carries on the parent variant (`Import { name, alias: Option<Box<SyntaxTree>> }`, `FromImport` already has `alias`).**
  - Replace `Aliased { inner }` siblings with the parent's `alias` slot. Lowering simplifies; rendering unchanged.

- [ ] [S11-Z6] **`SyntaxTree::Expression` does not exist as a stored tree node; `<expression>` host appears at xot-render time only.**
  - The biggest of the wrapper removals. `Expression` today wraps every value-position slot per Principle #15. To honor stable-XPath-host semantics: have `to_xot` insert `<expression>` automatically when rendering value-position slots (`Variable.value`, `Binary.left/right`, `Return.value`, ...). The `Expression::wrap` helper retires; lowering sites pass the raw inner expression directly into typed slots.
  - Marker case (`non_null` / `await`): the marker becomes a sibling slot or an enum variant on the parent; explicit at the lowering layer.
  - Largest internal refactor in the slice — touches every value-position lowering across all 9 languages.

- [ ] [S11-Z7] **`SyntaxTree::Body` does not exist; declaration variants carry `children: Vec<SyntaxTree>` directly.**
  - `Class { children, ... }`, `Function { body: Option<Vec<SyntaxTree>>, ... }`, `If { body: Vec<SyntaxTree>, ... }`, etc. The `<body>` wrapper appears at xot-render time when the language convention requires (Python: `<body>` always; C#: `<body>` only inside method/property accessors).
  - `to_data::project` arms for Class/Function/If/While/For collapse — no body-inlining special case; `children` are simply the variant's pairs.
  - High value: removes the body-inlining override that today is the most awkward convention exception.

- [ ] [S11-Z8] **`SyntaxTree::Binary` / `Unary` / `Comparison` carry `op: BinaryOp` (or per-variant `Op*` enum) instead of `op_text: String + op_marker: &'static str + op_range: ByteRange`.**
  - `enum BinaryOp { Plus, Minus, Lt, Gt, Eq, Ne, And, Or, … }`. Range stored separately (`op_range: ByteRange`); display text derived from `op.as_str()` or from `range.slice(source)` for source preservation.
  - `to_data::project` arms drop the `make_op_mapping` helper; op projection becomes a generic enum-to-marker pair.

- [ ] [S11-Z9] **tree field names match JSON key names.**
  - Renames: `If.if_true → then_`, `If.if_false → else_`, `Ternary` same, `For.targets → left`, `For.iterables → right`, `Foreach.target → left`, `Foreach.iterable → right`, `Foreach.type_ann → type`, `Returns.type_ann → type`, `Variable.type_ann → type`, etc.
  - Rust keyword conflicts (`then`, `else`, `type`) get a trailing underscore; the data projector strips it. (`then_` → `then` JSON key.)
  - Pure mechanical search-and-replace; no semantic change.

- [ ] [S11-Z10] **(optional) Scalar literals unify into `SyntaxTree::Scalar { kind: ScalarKind, range, span }`.**
  - Drops `SyntaxTree::Int`, `Float`, `String`, `True`, `False`, `None`, `Null` in favor of one variant. Cost: loses exhaustive-match-by-literal-kind; future per-kind substructure (concatenated strings, f-strings) needs a different mechanism (e.g. `Scalar::FString { parts }`).
  - Lowest priority — the seven separate variants are mostly fine, the projection is already trivial for them.

- [ ] [S11-Z11] **`to_data::project` is < 200 LOC; the only special cases are scalar atoms + sequence-shape variants. Mapping-shape variants project via a generic field-walker.**
  - The closing condition. Once Z1–Z9 (and optionally Z10) land, the special cases enumerated under "Genuinely fundamental" earlier in this convo are the only arms that remain. The rest fall through to the generic walker.

---

## S13 — Unified source renderer (one path, three input sources)

**Goal.** One `render(tree, lang, source: Option<&str>)` function. The renderer assembles output text from three sources: language keywords and structural punctuation driven by the tree's node type and the per-language Syntax config; literal text stored on scalar tree nodes; and gaps/whitespace sliced from source where anchored byte ranges exist, defaulted per language otherwise. Today's "anchored mode vs canonical mode" distinction disappears — there is one rendering path, with a graceful fallback for the third source.

**Why now.** Canonical-mode rendering is partly built and entirely unreachable. Every production caller of `tree::render::render` passes `Some(source)`, which short-circuits to byte-slicing in `super::to_source` before the per-language match runs. The eight per-language `render_source.rs` scaffolds under `tractor/src/languages/<lang>/` were moved through S10B-Z1..Z8 and the S12 terminology rename, but their code paths are dead. The blocker is structural: the tree's scalar variants (`Name`, `Int`, `Float`, `String`, `Atom`, `True`, `False`, `None`, `Null`) carry only `range: ByteRange`, not the text — so a freshly-built synthetic tree literally has no way to know what identifier to emit for a `Name` node. The doc-comment at `tractor/src/tree/render/mod.rs:36–41` admits this: atoms "emit placeholders in canonical mode because their text is only available via the source-anchor."

Finishing this unlocks the structural-mutation work that `tractor set` / `tractor update` need (inserting a new method into a class has no source anchor for the new bytes), the cross-language codegen usecase (parse C# → transform tree → render as TypeScript — no TS source ever existed), and any synthesis-from-query work.

**Depends on.** Nothing structural — independent of S11 (tree shape redesign) and S5 (JSON projection). **Coordinates productively with S11-Z8**: that task replaces `Binary::op_text: String + op_marker: &'static str` with a typed `op: BinaryOp` enum; the enum's canonical text is the operator-rendering equivalent of this slice's scalar-text storage. Land either order. If S11 hasn't run, this slice still works — operator text becomes a special case of scalar text.

**Unblocks.** Structural mutation in `tractor set` / `tractor update`; cross-language codegen (`docs/usecase-csharp-to-typescript-codegen.md`); programmatic synthesis from queries.

**Independent of.** S2 / S3 / S4 (all closed or mostly closed), S5, S6, S7, S8, S9, S10, S11, S12.

**Size.** L. Touches every scalar variant in `SyntaxTree`, every per-language lowering for SyntaxTree / DataTree / SqlTree (~13 lowering modules total), every per-language Syntax config, and every test that builds a tree manually. The renderer code itself is a single function; the work bulk is in lowering text-population and per-language formatter completeness.

**Reversibility.** Medium. Per-Z reversible. Once tree variants change shape, every constructor in tests and lowerings updates with them; no external-API churn beyond the renderer's signature staying the same.

**Invariants when closed:**

- Every `SyntaxTree::{Name, Atom, Int, Float, String, True, False, None, Null}` variant carries a `text: String` (or equivalent representation that uniquely determines the literal's source form).
- `SyntaxTree::String` carries a `quote_style: QuoteStyle` field; `QuoteStyle` covers at least `Single`, `Double`, `TripleSingle`, `TripleDouble`, `Backtick`, `Raw { prefix: String }`, `Heredoc { delimiter: String }`. The renderer reproduces the original style.
- Byte ranges across `SyntaxTree`, `DataTree`, `SqlTree` are explicitly optional (either `Option<ByteRange>` or `ByteRange` paired with an `anchored: bool` flag). Synthetic nodes constructed without source anchoring carry the un-anchored state.
- `tractor/src/tree/render/mod.rs::render(tree, lang, source)` has one code path. The `Option<&str>` controls per-gap behaviour, not whole-function dispatch.
- Per-language `render_source.rs` modules emit valid source — no placeholder text — for every variant when source is `None`.
- **Round-trip property:** for any parsed source `S` in any tree-supported language, `render(parse(S), lang, Some(S)) == S` byte-for-byte.
- **Canonical property:** for any synthetic tree `T`, `parse(render(T, lang, None), lang)` produces a tree structurally equivalent to `T`.
- **Mixed property:** for a tree partly anchored in source `S` and partly synthetic, `render(T, lang, Some(S))` slices anchored regions from `S` and emits canonical defaults for synthetic regions. The boundary between an anchored sibling and a synthetic sibling uses the canonical default (simple option; sibling-history recovery is a future refinement).
- The orphan canonical-mode scaffolding (the `if source.is_some() → return early` fork in `render`, the unused `common::render_generic` if it stays unused) is gone — the unified path is the only path.

### Tasks

- [x] [S13-Z1] **Scalar variants carry typed text.** Done — every `SyntaxTree::{Name, Atom, Int, Float, String, True, False, None, Null}` carries `text: String`; `String` also carries `quote_style: QuoteStyle` with the required variants (`Single`, `Double`, `TripleSingle`, `TripleDouble`, `Backtick`, `Raw { prefix, inner }`, `Heredoc { delimiter }`). Escape encoding: implementers landed option (a) — `text` stores decoded text. File: `tractor/src/tree/types.rs`.

- [x] [S13-Z2] **Byte ranges are explicitly optional via an `anchored` flag.** Done — the flag lives *inside* `ByteRange` (smaller blast radius than per-variant siblings): `ByteRange { start, end, anchored }` with `new`/`empty_at` defaulting to `anchored: true` (lowerings keep their existing call shape) and new `synthetic`/`synthetic_empty` constructors for programmatic synthesis. `SyntaxTree::is_anchored()` exposed for the renderer.

- [x] [S13-Z3] **Lowerings populate scalar text at parse time.** Done — `lower_helpers.rs` gained per-variant constructors (`name_of`, `int_of`, `string_of`, …) that take a `TsNode + source` and stamp `text + range + span + anchored: true` in one place. All 8 SyntaxTree lowerings (csharp/go/java/php/python/ruby/rust_lang/typescript) converted via the helpers. DataTree already had `value`/`text` fields pre-S13; SqlTree has its own QuoteStyle on string atoms. Per-language quote-style detection beyond the conservative `Double` default is Z5 follow-up.

- [x] [S13-Z4] **Synthetic-tree test constructors compile and pass.** Done — most match arms used `..` and pick up the new fields transparently; the only explicit constructors needing updates (`to_data.rs::name` test helper) were patched. `tractor/tests/` had no direct scalar constructions to migrate.

- [x] [S13-Z5a] **Per-language `syntax()` config exposed for the unified dispatcher.** Done — each `render_source.rs` exposes `pub fn syntax() -> Syntax` consumed by `syntax_for(lang)` in the unified renderer. Existing Syntax fields (keywords, block open/close, terminator, indent, paren_conditions, typed_param_pre, …) carry over from the canonical-only emitters.

- [ ] [S13-Z5b] **Syntax config covers richer gap-context conventions.** Pending — blank-line policy between top-level / class-member / statement siblings, separator characters, modifier spacing. Natural pairing with S13-Z6b's full gap engine.

- [x] [S13-Z6a] **Per-subtree byte-slice shortcut for anchored regions.** Done — `write_ir_with_source(tree, source, …)` short-circuits to `range.slice(source)` for any subtree whose node *and* direct children are anchored. Covers the common "everything parsed, edit a leaf" case and gives mixed-mode trees byte-identical output for anchored regions.

- [ ] [S13-Z6b] **Full gap-context fallback engine.** Pending — per-gap context lookup against the Syntax config (Z5b), anchored↔synthetic boundary policy (canonical-default per chat decision 2026-05-13), modifier spacing, blank-line conventions between statement / member siblings. Today the canonical walker fills synthetic regions without this richer per-gap awareness.

- [x] [S13-Z7] **`render(tree, lang, source)` has one code path.** Done — the `if let Some(source) = source_anchor { return super::to_source(...) }` fork in `tractor/src/tree/render/mod.rs::render` is gone. The function now: dispatches once via `syntax_for(lang)`; calls `common::write_ir_with_source` which carries the optional source through the walk; uses the byte-slice fast path only as a private optimisation when `tree_fully_anchored(tree)` AND `source.is_some()`. `render_sql` retains the older two-branch shape pending an SqlTree gap engine (noted in its doc-comment).

- [x] [S13-Z8] **Three rendering properties have test coverage.** Done — `tractor/tests/render_unified.rs` exercises: **round-trip** (every blueprint fixture, anchored render == source byte-identical, 8 languages); **canonical** (synthetic `Module` with a synthetic `Name` round-trips to a structurally-equivalent re-parsed `Module`); **mixed** (parse a Python source, programmatically push a synthetic `Name` child, assert the synthetic text appears in the rendered output). All three pass.

- [x] [S13-Z9] **Orphan canonical-mode scaffolding is gone.** Done — `tree/render/mod.rs::render`'s early-return fork is replaced by the unified dispatcher; `common::render_generic` deleted (no callers); the stale "atoms emit placeholders" module-level doc-comment is rewritten to describe the unified three-source pipeline; `#![allow(dead_code)]` removed from `tree/render/mod.rs`, `tree/render/common.rs`, and all 8 per-language `render_source.rs` files (no new dead-code warnings surfaced).

### DataTree extension (paired with S13)

The original S13 entry called out `DataTree` as out-of-scope ("revisit unifying them with the SyntaxTree side after S13 lands"). The data-model parity work was done alongside:

- [x] **DataTree scalars carry typed text + quote style.** `DataTree::String` gained `quote_style: QuoteStyle` so JSON `"x"`, YAML `'x'`, YAML plain `x`, TOML basic vs literal vs multiline all round-trip distinctly. `DataTree::Bool` and `DataTree::Null` gained `text: String` so YAML `yes`/`no`/`True`/`true` and `~`/`null` survive the parse → render boundary. `QuoteStyle` extended with `Plain` (unquoted scalars) and `Block { folded, chomp }` (YAML `|` / `>`).
- [x] **DataTree byte-range anchoring** is automatic — anchoring lives inside `ByteRange` itself, so DataTree and SqlTree pick it up for free. `DataTree::synthetic_scalar` switched to `ByteRange::synthetic_empty()`.
- [x] **All 5 data-format lowerings populate the new fields**: `lower_json` (Double-quoted strings, canonical bool/null text), `lower_yaml` (`yaml_quote_style` helper maps CST kinds to QuoteStyle Plain/Single/Double/Block), `lower_toml` (Double basic + Plain bare/keys), `lower_ini` (Plain), `lower_markdown` (Plain).
- [x] **DataTree accessors**: `DataTree::is_anchored()` and `scalar_text()` mirror their `SyntaxTree` counterparts; `scalar_text()` also covers `Comment` for consistency.
- [x] **Property tests** in `tractor/tests/render_unified.rs`:
  - `datatree_round_trip_is_byte_identical` — anchored `to_source()` is byte-identical for inline JSON / YAML / TOML / INI fixtures.
  - `datatree_scalars_carry_text_and_quote_style` — parsed JSON populates `quote_style: Double` on strings, canonical `text` on bool / null, and `is_anchored()` / `scalar_text()` agree with the structure.
  - `datatree_synthetic_scalars_unanchored` — `synthetic_scalar` produces unanchored nodes with the right `scalar_text()`.
- [x] **Layer A scalar-emit primitive + JSON/YAML plumbing share.** Done (post-S13 — captured under S14 below): `write_quoted_scalar` lifted to `tree::render::common` and routed through SyntaxTree / DataTree-JSON / DataTree-YAML / SqlTree identifier emission; `DataRenderOptions` / `DataSpanMap` / `scalar_text` co-located in `tree::render::data_common`.

- [ ] **Full DataTree↔SyntaxTree engine unification.** Pending — DataTree continues to use its own `tree::render::data_json` / `tree::render::data_yaml` walkers rather than the SyntaxTree `write_ir_with_source` engine. Layer-B in the analysis (`docs/design-editable-trees.md` references). The data-model parity from S13 makes the merge mechanical when a use case (e.g. structural mutation crossing tree types) demands it.

### Notes for the implementing agent (you can ignore once you've read them once)

- The current renderer is `tractor/src/tree/render/mod.rs::render` (line 67). Two early returns: `Some(source)` → byte-slice via `super::to_source`; otherwise → match per-language. This slice eliminates the fork.
- `super::to_source` does the anchored byte-slicing. Confirm its current location at slice start; refactor target is to make it an internal optimisation, not the API.
- Shared Syntax struct + walk engine: `tractor/src/tree/render/common.rs`.
- Per-language Syntax structs: 8 SyntaxTree languages under `tractor/src/languages/<lang>/render_source.rs`, plus `tractor/src/tree/sql/render_source.rs` for SQL.
- DataTree has its own renderer pair (`tree::render::data_json`, `tree::render::data_yaml`) wired through `mutation/xpath_upsert.rs`. Those work today and follow the same three-source idea informally. Out of scope for this slice — revisit unifying them with the SyntaxTree side after S13 lands.
- Z3 sequencing tip: do C# or Python first as the pilot; mirror to the other 7 SyntaxTree languages once the shape is set.
- Z5 is the biggest single piece of work — per-language formatting completeness. Start with one language end-to-end (probably the same one as Z3's pilot) so Z6 / Z8 can validate against it before fanning out.

---

## S14 — Renderer-engine consolidation (post-S13 cleanup)

**Goal.** After S13 unified the SyntaxTree source renderer, lift the truly-shared primitives so SyntaxTree / DataTree / SqlTree all consume the same quote-style emission, the same `Indent` / `Span` plumbing, and the same anchored / synthetic fallback predicate. Co-locate the DataTree JSON+YAML pair so their shared plumbing (options shape, span map, scalar-text helper) lives in one module while their distinct per-format walks remain side-by-side.

**Layer terminology.** The "Layer A / B / C" labels used in the tasks below are defined in [`docs/design-ir-and-renderings.md` § 3.4](docs/design-ir-and-renderings.md#34-renderer-engine-layering-whats-shared-across-trees-and-where-it-stops). Layer A = shared leaf primitives (no walk semantics); Layer B = shared orchestration shell with per-tree variant handlers; Layer C = single generic walker (rejected, not a target).

**Why now.** S13 brought the SyntaxTree side into a single unified path; with DataTree's data model already in parity (S13's DataTree extension), the smallest mechanical wins are obvious — `emit_string` was duplicated across `data_json.rs`, `data_yaml.rs`, and `tree/sql/render_source.rs`; per-format `JsonRenderOptions` / `YamlRenderOptions` structs were nominally identical; `scalar_text` was duplicated. Lifting these is low-risk and unlocks future Layer-B unification when a use case demands it.

**Depends on.** S13. **Unblocks.** Editable trees (S15) — Layer A primitive carries through to slice-3 mutation rendering.
**Size.** S. **Reversibility.** High; the changes are mechanical.

### Tasks

- [x] [S14-Z1] **Layer A: shared `write_quoted_scalar(text, &QuoteStyle, escape, out)` primitive.** Done — lives in `tree::render::common`; replaces inline string emit in SyntaxTree's `write_ir` (now respects `quote_style` instead of hardcoding `Double`), JSON's `emit_string` (delegates with JSON's escape table), YAML's `emit_scalar_string` (delegates with the auto-promote-Plain-to-Double-when-needed logic), and SqlTree's identifier emission (via the QuoteStyle unification — Z3 below). `yaml_quote_string` deleted; the YAML key-quoting path routes through the shared primitive.

- [x] [S14-Z2] **Co-located DataTree renderer plumbing.** Done — new `tree::render::data_common` module hosts `DataSpanMap`, `DataRenderOptions`, and `scalar_text(&DataTree) -> String`. `JsonRenderOptions` / `YamlRenderOptions` are now type aliases for `DataRenderOptions`. `mutation/xpath_upsert.rs` callers keep compiling unchanged. The per-format walks (`data_json::render_json_*`, `data_yaml::render_yaml_*`) stay separate — JSON's bracket-delimited block layout and YAML's significant-indent shape are structurally too different to share a walker without ceremony.

- [x] [S14-Z3] **QuoteStyle unification across SyntaxTree / DataTree / SqlTree.** Done — added `Brackets` variant to the shared `tree::types::QuoteStyle` (T-SQL `[Users]`) and a `marker_name()` accessor mapping every variant to its xot marker element. `tree::sql::types::QuoteStyle` (the local 4-variant enum: `None`/`Brackets`/`DoubleQuote`/`Backtick`) is retired; `tree::sql::QuoteStyle` is now a re-export of the shared type. Mapping during migration: `None → Plain`, `DoubleQuote → Double`, others unchanged. `SqlTree::QuoteStyle::wrap()` deleted — the shared `write_quoted_scalar` primitive replaces it.

- [ ] [S14-Z4] **Layer B: full DataTree↔SyntaxTree engine unification.** Pending — DataTree still uses its own walkers. The split is the right level of separation today (the per-variant arms genuinely diverge); Layer B becomes worth doing when structural mutation lands on SyntaxTree with a span-map output, at which point a uniform entry-point shape `render(tree, source) -> (String, Option<SpanMap>)` is worth the trait scaffolding.

- [ ] [S14-Z5] **Re-evaluate Layer C in light of `TreeNode`.** S14 / [`docs/design-ir-and-renderings.md` § 3.4](docs/design-ir-and-renderings.md#34-renderer-engine-layering-whats-shared-across-trees-and-where-it-stops) classified "Layer C = single generic walker" as **rejected, not a target** for the renderer. That decision pre-dates the `TreeNode` trait introduced for S15-Z1/Z2 follow-ups, which proved out a generic pre-order walker on two non-trivial cases (`assign_ids`: -573 LOC; `locator`: -874 LOC). The rejection's substance — that per-variant *emission shapes* genuinely diverge (synthetic `<expression>` / `<left>` / `<right>` wrappers for SyntaxTree, format-specific punctuation for DataTree renderers) — still stands for the *renderer*. But the rejection's *generality* (a generic walker is wrong for trees) is now demonstrably false for traversal-shaped passes. Audit: is there a hybrid where the *traversal* is `TreeNode`-generic but the *emission* is per-variant via an associated method (`fn emit(&self, ctx: &mut RenderCtx)`)? That's strictly more structure than Layer A and strictly less than Layer C as originally framed. If yes, it could unify the three `to_xot.rs` files (~4000 LOC) without violating the per-variant-emission constraint. **Status:** open question; no work scheduled. Decision should land before any to_xot refactor.

---

## S15 — Editable trees: NodeId-based mutation pipeline

**Goal.** `tractor set` (and future mutation verbs) flow through the typed tree, not byte-splicing on source. Address by XPath against the xot projection; the matched xot element carries an `@id` that resolves to the typed-tree node via a stable per-node `NodeId`; mutation happens on the typed tree; render via the unified renderer (S13) reproduces source. Same pipeline for SyntaxTree, DataTree, SqlTree.

**Why now.** S13 made typed-tree rendering authoritative for source emission. Today's `apply_set_mapping` still byte-splices for SyntaxTree-pipeline languages (the DataTree side already mutates the typed tree). Lifting SyntaxTree to the same shape closes the last "two source-of-truth" gap and removes ~1k LOC of legacy splice code.

**Depends on.** S13 (typed renderer authoritative). **Unblocks.** S4B-Z4 (SyntaxTree upsert for all 8 tree-pipeline languages); future structural-mutation verbs (replace, clone-and-modify).

**Size.** M. **Reversibility.** High — each slice independent; revert is one commit.

**Design doc.** [`docs/design-editable-trees.md`](docs/design-editable-trees.md).

### Tasks

- [x] [S15-Z1] **Slice 1 — `NodeId` on `Span` + `assign_ids` walkers.** Done — `pub type NodeId = u32` with `0` as the unassigned sentinel; `id: NodeId` field added to `Span` (threads through every existing accessor for free, same trick as S13-Z2's `anchored` flag on `ByteRange`). `tree::assign_ids` module exports `assign_ids_syntax` / `assign_ids_data` / `assign_ids_sql`, each a thin wrapper around one generic depth-first walker `walk<T: TreeNode>` that stamps every node from a fresh counter. Per-variant child knowledge lives on each tree's `TreeNode::children_mut` impl (see follow-up below), not in the walker. Wired into both `parse_with_ir_pipeline` (xot output) and `parse_with_ir_pipeline_to_xee` (xee path); every tree exiting either parser path satisfies "non-zero unique id on every node".

  **Follow-up (post-original-landing):** introduced `pub trait TreeNode { span, span_mut, range, children, children_mut, is_anchored (default), to_source (default) }` in `tree/types.rs`, implemented on `SyntaxTree`, `DataTree`, `SqlTree`. `assign_ids.rs` collapsed from 731 → 158 lines: three parallel per-variant walkers became one generic `fn walk<T: TreeNode>(tree: &mut T, next: &mut NodeId)`. Adding a new tree variant requires no change in `assign_ids` — only in the tree's `children_mut` arm.

- [x] [S15-Z2] **Slice 2 — xot-side `@id` + `find_by_id` locators.** Done — `set_span_attrs` in `tree/to_xot.rs`, `tree/data/to_xot.rs`, and `tree/sql/to_xot.rs` stamps `@id="N"` on every xot element when the typed node carries a non-zero NodeId; omitted for synthetic / pre-`assign_ids` paths. `tree::locator` exports one generic `find_by_id<T: TreeNode>(tree: &mut T, id: NodeId) -> Option<&mut T>` plus `parse_id_attr` for converting an XML attribute string to a `NodeId`. End-to-end test `xpath_match_id_resolves_to_typed_syntax_node` exercises the full parse → query → `@id` → `find_by_id` → typed-mutation pipeline.

  **Follow-up (post-original-landing):** the original implementation had three parallel `find_by_id_syntax` / `_data` / `_sql` walkers, each with its own per-variant descent (~1000 LOC total in `locator.rs`). After `TreeNode` landed (see Z1 follow-up), all three walkers collapsed to one generic function reading `span()` + `children_mut()`. `locator.rs`: 1025 → 151 lines. The three per-tree shims were removed entirely; callers ([`mutation/xpath_upsert.rs`](tractor/src/mutation/xpath_upsert.rs), [`tests/render_unified.rs`](tractor/tests/render_unified.rs)) now call `find_by_id` directly — Rust's type inference picks the tree type from the `&mut tree` argument.

- [x] [S15-Z2-Sql] **SqlTree Tier 1 — bring SqlTree to Slice 1+2 parity.** Done — recursive `assign_ids_sql` covering all 57 variants (was skeletal "stamps root only"); recursive `find_by_id_sql` covering all 57 variants; `set_span_attrs` added to SqlTree's xot projection (SqlTree xot previously carried no `@line`/`@column` attrs either — added in this slice); inline `<name>` leaves inside `<relation>` / `<reference>` / `<column>` / standalone Identifier/Schema/Alias paths stamped with the corresponding typed node's NodeId via the `name_leaf_with_span` helper. End-to-end test `xpath_match_id_resolves_to_typed_sql_node` validates the SqlTree XPath → `@id` → typed-node round-trip.

  **Follow-up (post-original-landing):** with `TreeNode` (see Z1 / Z2 follow-ups), SqlTree's 57-variant walks no longer exist as separate functions — `assign_ids_sql` and the former `find_by_id_sql` share the generic walker with the other two trees. The 57 variants are now declared *once*, in the `TreeNode::children` / `children_mut` impl on `SqlTree` in [`tree/sql/types.rs`](tractor/src/tree/sql/types.rs). The `name_leaf_with_span` helper for inline `<name>` leaves inside relations / references / columns is unchanged.

- [x] [S15-Z3] **Slice 3 — wire SyntaxTree `set` through `find_by_id`.** Done — XPath matches against syntax-tree projections now carry the typed node's `NodeId` (xot `@id` extracted in `xpath::engine::extract_location_and_id_from_xot` and surfaced on `Match.node_id`); `mutation::xpath_upsert::update_existing_via_syntax_ir` walks each match, locates the typed node via the generic `find_by_id` (originally `find_by_id_syntax` — see Z2 follow-up), mutates its `text` field via the new `SyntaxTree::set_scalar_text`, renders that single leaf via `tree::render::render(leaf, lang, None)`, and splices the result into the original source at the match's byte range. `lang_supports_upsert` now includes all 8 SyntaxTree-pipeline languages; CLI surface unchanged. No-match for syntax-tree languages returns a no-op result (Slice 3 is update-only — insertion is Slice 4). Property tests in `tests/render_unified.rs`: `syntax_tree_matches_carry_node_id`, `upsert_renames_python_function_via_typed_pipeline`, `upsert_renames_typescript_string_literal`, `upsert_no_match_on_syntax_tree_is_noop`. Closes S4B-Z4 for the 8 SyntaxTree-pipeline languages. SqlTree-pipeline `set` is **not** wired through here (`lang_uses_syntax_tree("tsql")` is `false`) — SqlTree mutation lands with Tier 2 once a canonical scalar-surround renderer exists.

- [ ] [S15-Z4] **Slice 4 — `replace <xpath> <xml-literal>` CLI verb.** Pending. API syntax TBD (open questions: how is the XML literal supplied — stdin / `--xml '...'` / file path? batched multiple `(xpath, xml)` pairs in one invocation?). The "universal mutation primitive" decision: subtree replacement is the only structural mutation operation; no per-variant `--set` / `--push` / `--remove` API. Rename / add / delete are all expressed as "replace this subtree with that one." Defer until Slice 3 lands.

- [ ] [S15-Z5] **Slice 5 — full XML → typed projection.** Deferred until the full code-synthesis roadmap item lands. The slot-aware projection from Slice 4 (which projects an XML literal against the parent slot's expected variant) covers most realistic substitution; generic XML → typed (a `SyntaxTree`-from-arbitrary-XML parser per language) is only needed when the substitution shape can't be inferred from the parent slot.

- [ ] [S15-SqlTier2] **SqlTree Tier 2 — canonical renderer for scalar mutations on anchored trees.** Pending. Today's SqlTree renderer mostly emits `/*todo:*/` placeholders for composite variants. With Slice 3, scalar mutations work via the byte-slice fast path for anchored subtrees (the renderer slices source for unchanged Select/From/Join/etc., emits typed text for the mutated scalar). Tier 2 is the per-variant `Syntax`-config-style emission only for the structural surrounds of scalars (~200–300 LOC). Composite variants stay byte-sliced; Tier 3 (full canonical SQL renderer) is reserved for synthesis / subtree replacement on SQL.

---

## S5 — Single JSON projection: `SyntaxTree → DataTree → JSON` (kills part of W1)

**Goal.** One JSON projection algorithm for all three tree families. The principled `data_to_json` (typed `DataTree` reader) is the only path; the heuristic blocks in `tree_to_json` are gone.

**Why now.** Three concurrent JSON projection strategies coexist: legacy `xml_node_to_json` (XmlNode-based fallback), heuristic `tree_to_json` (1087 LOC of `$inline`/`$skip`/`$type`/plural-collapse rules papering over tree↔JSON impedance), and the principled `data_to_json` reading a typed `DataTree`. The `to_data` projection was completed in S5A-Z7: every `SyntaxTree` variant has its own arm with compile-enforced exhaustiveness (verified 2026-05-13). What remains is a snapshot-reconciliation audit (S5A-Z8), retiring `tree_to_json` and `tree/to_json.rs` (S5B + S5D), and routing `SqlTree` JSON output through `DataTree` (S5E).

**Depends on.** Nothing structural.
**Unblocks.** Removing ~1000 LOC of heuristics (`tree/to_json.rs`).
**Independent of.** S2, S3, S4, S6, S7, S8, S9.

**Size.** M. The bulk is rule migration and parity audit.
**Reversibility.** Medium. Requires snapshot regen for JSON output; revert is one commit but disruptive if downstream readers exist.

**Invariants when closed:**
- `tree::to_data::lower_to_data_ir` covers every `SyntaxTree` variant.
- `tractor/src/tree/to_json.rs` does not exist.
- `Tree::to_json` for `Tree::SyntaxTree` calls `data_to_json(to_data(tree, source))`.
- `SqlTree` JSON output also flows through `DataTree`.

### Tasks

- [x] [S5A] **`tree::to_data::lower_to_data_ir` covers every `SyntaxTree` variant deterministically.** Coverage invariant met 2026-05-08 (S5A-Z7); independently re-verified 2026-05-13 (74/74 variant arms in `tractor/src/tree/to_data.rs::project`; no `unhandled:` markers in production code paths). Z8 (snapshot reconciliation) and the boilerplate-reduction OPTIONS remain — both subsumed by S11, which restructures the tree so the per-variant arms collapse into a generic walker. The status note below is kept for the historical record of how coverage was sliced.
  - **Direction (chosen 2026-05-08).** Wire S5C first (done — see below) with a `has_unhandled` fallback that keeps documents on `tree_to_json` while any of their variants still hit the catch-all. Migrate variants here incrementally; documents flip to the typed path organically as their last unhandled variant gets covered, giving per-variant snapshot bisectability. The strict invariant ("no fixture ever trips `has_unhandled`") closes the slice and lets S5D delete `tree_to_json`.
    - [x] [S5A-Z1] **Foundation: `Name`, `Atom`, `Module`, `Class`, `Inline`, `Skip` projected.** Done in iter 39 + S5A start.
    - [x] [S5A-Z2] **Declarations batch: `Body`, `Function`, `Variable`, `Property`, `Returns`, `Parameter`, `SimpleStatement` (non-marker), `Comment`, `Return`, `Decorator`, `Import`.** Done 2026-05-08.
    - [x] [S5A-Z3] **Expressions batch: `Expression`, `Binary`, `Unary`, `Comparison`, `Call`, `Ternary`, `Is`, `Cast`, `KeywordArgument`, `ListSplat`, `DictSplat`, `Pair`.** Done 2026-05-08 (`Access` deferred to Z6). New `make_op_mapping` helper for op-bearing variants.
    - [x] [S5A-Z4] **Collections + scalar literals: `Tuple`, `List`, `Set`, `Dictionary`, `Int`, `Float`, `String`, `True`, `False`, `None`, `Null`.** Done 2026-05-08. Scalars map to natural `DataTree` variants; collections to `Sequence`/`Mapping`.
    - [x] [S5A-Z5] **Control flow: `If`, `ElseIf`, `Else`, `For`, `Foreach`, `CFor`, `While`, `DoWhile`, `Break`, `Continue`, `Try`, `ExceptHandler`.** Done 2026-05-08. Body's children flatten directly into the parent.
    - [x] [S5A-Z6] **Misc tail: `Lambda`, `ObjectCreation`, `Constructor`, `Generic`, `TypeParameter`, `GenericType`, `TypeAlias`, `Enum`, `EnumMember`, `Accessor`, `Using`, `Namespace`, `From`, `FromImport`, `Path`, `Aliased`, `Assign`, `FieldWrap`, `PositionalSeparator`, `KeywordSeparator`, `Unknown` + the `Access` chain.** Done 2026-05-08. New `project_access_segment` helper.
    - [x] [S5A-Z7] **`has_unhandled` returns `false` for every fixture; `Tree::SyntaxTree::to_json` reaches `tree_to_json` from no test path.**
      - Done 2026-05-08. Catch-all removed from `project()`; Rust's exhaustiveness check enforces every `SyntaxTree` variant has its own arm. Legacy `tree_to_json` fallback unreachable. New tests: `has_unhandled_predicate_trips_on_unhandled_marker`, `projection_is_exhaustive_for_all_ir_variants`.
    - [ ] [S5A-Z8] **Snapshot reconciliation: any JSON snapshots that move when fixtures flip from legacy to typed path are reviewed and updated.**
      - Status: needs audit. New path is LIVE post-Z7 but no snapshots broke under `cargo test` — likely because most snapshot tests render via XPath→XmlNode→`xml_node_to_json` rather than `Tree::SyntaxTree::to_json` (only reached via `format::json.rs::Projection::Tree`, i.e. `--projection=tree`). Enumerate fixtures that hit that path, regenerate, review diff. Expected: `$type` keys disappear; `$inline` / `$skip` / plural-of-self cease to exist.
  - **Coverage status.** Complete after Z7: every `SyntaxTree` variant has its own arm in `tractor/src/tree/to_data.rs::project`; compile-enforced exhaustiveness.
  - **OPTIONS: Boilerplate reduction in `to_data.rs`** (2026-05-08, raised by user). The `project` function is ~500 LOC of mechanically-similar match arms — declaration variants do `push_modifier_flags + named slots`, control-flow variants do `condition + body-flatten + else?`, op-bearing expressions do `op + operands`, etc. Three plausible reduction approaches; pick one (mark `[x]`) to commit. **Note 2026-05-13:** This OPTIONS block is subsumed by S11 — the structural fix there obviates the boilerplate-reduction question. Leave unselected; revisit only if S11 is not pursued.
    - [ ] **(a) `project!` macro.** Takes a variant name and a list of `(slot_key, expr)` pairs, expands to the boilerplate. Reduces 5–10 lines per variant to 2–3. Cost: introduces a macro layer that obscures projection rules; harder to step-through with a debugger.
    - [ ] **(b) `MappingBuilder` helper toolkit (recommended).** Extend `make_pair` / `make_flag` / `push_modifier_flags` / `collect_member_pairs` with a small builder API: `MappingBuilder::new(range, span).flag(name).pair(key, value).inline_body(body).build()`. Codifies the four recurring shapes (`flags + name + slots`, `condition + body`, `op + operands`, `transparent passthrough`) without macros. ~30% verbosity reduction. Each arm becomes a few-line builder pipeline; `project` stays as the dispatch.
    - [ ] **(c) Per-variant `SyntaxTree::project_to_data` methods.** Move each arm to where the variant is defined in `types.rs`. Pro: variant-local. Con: scatters the projection ruleset across `types.rs` (meant to be data-only), and breaks "one place to read all projection rules".

- [ ] [S5B] **All JSON shape decisions (`$inline`, `$skip`, `$type:"expression"`, plural-of-self collapse, marker-vs-leaf) live as uniform projection rules in `to_data`, not in `to_json`.**

- [x] [S5C] **`Tree::to_json` for the `Tree::SyntaxTree` arm calls `data_to_json(to_data(tree, source))`.**
  - Done 2026-05-08. Hybrid dispatch in `xpath/match_result.rs`: project via `lower_to_data_ir` first; if `has_unhandled` is false render via `data_to_json`, else fall back to legacy `tree_to_json`. After S5A-Z7 the fallback is unreachable. `Tree::DataTree` keeps direct `data_to_json`; `Tree::SqlTree` keeps `sql_to_json` until S5E.

- [ ] [S5D] **`tractor/src/tree/to_json.rs` does not exist.**
  - Depends on S5C (no caller).

- [ ] [S5E] **`SqlTree` JSON output flows through `DataTree` projection.**
  - All three tree families share one JSON projection algorithm with three entry points.

---

## S16 — Typed-tree migration: known shape regressions

**Goal.** Restore semantic-shape information that the typed-tree migration accidentally flattened. The typed catalogue (`SyntaxTree::{Name, String, Int, ...}`) is generic by design; per-language semantic wrappers and markers that the imperative pipeline minted didn't all carry over. Each item below is a concrete diff between current output and the committed snapshot (pre-typed-migration imperative output) — surfaced 2026-05-14 during a snapshot regen audit.

**Why now.** Alpha-stage so query compatibility doesn't matter, but the snapshots are the historical record of the project's curated tree shape and several losses look like genuine design regressions, not intentional simplifications. Each regression is small in isolation; collectively they erode the "principled semantic tree" property.

**Reversibility.** High. Each is a localized fix in one or two lowering files. After fixing, re-run `task test:snapshots:update` to clear the staged regression.

**Affected snapshots (currently failing `task test:snapshots`):** the 11 files reverted on 2026-05-14 — all 8 blueprint `*.snapshot.json` plus `php.php.snapshot.txt`, `ruby.rb.snapshot.txt`, `typescript.ts.snapshot.txt`. They will fail snapshot-check until the underlying regressions are addressed or the user explicitly opts to bake in the new shape.

### Tasks

- [ ] [S16-Z1] **TypeScript: restore `<path>` semantic node for import paths.** `import './barrel'` currently lowers the path string to `SyntaxTree::String`, which projects as `<string>"./barrel"</string>`. The pre-migration shape was `<path>./barrel</path>` (semantic role: "module path"). Either add a `SyntaxTree::Path { text }` leaf variant (cross-language, lines up with the `From/Path` segments structure that already exists) or wrap the import target in a typed slot whose projection emits `<path>` instead of `<string>`.

- [ ] [S16-Z2] **TypeScript: restore `import[sideeffect]` marker.** `import './barrel'` (no specifiers, side-effect-only) currently emits `<import>...</import>` indistinguishable from `import { x } from './barrel'`. The marker carried a real distinction. Add a `side_effect: bool` field to whatever variant lowers TS imports, project it as a `<sideeffect/>` marker.

- [ ] [S16-Z3] **PHP: restore `use[alias]` marker + `<path>` / `<aliased>` wrappers.** `use App\Foo as Log;` was `<use[alias]><path><name>App</name>...</path><aliased><name>Log</name></aliased></use>`; now flattened to `<use><name>App</name>...<name>Log</name></use>` — both the alias marker and the path/aliased distinction are lost. Restore the alias marker and slot wrappers in the PHP lowering.

- [ ] [S16-Z4] **PHP + Ruby: restore `<type>` wrapper on `extends` / `implements`.** `class Foo extends Base` was `<extends><type><name>Base</name></type></extends>`; now `<extends><name>Base</name></extends>`. `<type>` is the stable place to attach generic args / qualified names later. Restore via a typed slot on Class.

- [ ] [S16-Z5] **All 8 SyntaxTree languages: restore JSON projection's `$type` discriminator.** `data_to_json` no longer emits the `"$type": "program"` / `"$type": "file"` / etc. key on Module root. Useful for downstream consumers to disambiguate. Re-emit on the data-projection root.

- [ ] [S16-Z6] **All 8 SyntaxTree languages: restore curated top-level field grouping in JSON projection.** Pre-migration JSON had `imports: [...]`, `namespaces: [...]`, `consts: [...]`, `expressions: [...]` etc. — items grouped by kind. Current `data_to_json` collects everything under `nodes: [...]`. Either re-introduce per-language grouping rules in `to_data` or document that flat `nodes` is the new canonical shape.

- [ ] [S16-Z7] **All languages: restore `leading: bool` on comments.** Comment objects in JSON were `{leading: true, text: "..."}`; now flattened to `"..."`. The `leading` flag distinguished leading from trailing comments. Re-emit via `DataTree::Comment { leading, text }` or whatever the projection chain looks like for comments.

- [ ] [S16-Z8] **JSON `path` typed field flattening.** Java `path: {names: ["com.example"]}` is now split into `path: {names: ["com", "example"]}` (segment split — may be improvement). Python `path: {names: ["os"]}` is now `node: {name: "os"}` (lost the `path` semantic field). Audit each per-language: keep the segment-split if intentional, restore the `path` field name if dropped accidentally.

- [ ] [S16-Z9] **JSON typed-field flattening across primitives.** Ruby `int: "3"` → `node: 3`; PHP `int: "1"` → `node: 1`; Go `string: "\"fmt\""` → `node: "\"fmt\""`. The typed field name was the kind discriminator. Decide policy: either keep `int`/`string`/etc. as the field name in the JSON projection, or commit to generic `node` everywhere and document.

- [ ] [S16-Z10] **Re-regenerate the 11 reverted snapshots once Z1–Z9 land.** Run `task test:snapshots:update` after each fix and verify only the targeted shape changes apply.

---

## S12 — Drop the IR vocabulary entirely; per-domain tree types (`SyntaxTree` / `DataTree` / `SqlTree` / `DocumentTree`)

**Status: structural rename shipped 2026-05-12 (commits `0e08dffd` … `28dac866`); residual mop-up open per 2026-05-14 audit.** The eleven original Z-steps (type renames `Ir/DataIr/SqlIr → SyntaxTree/DataTree/SqlTree`, module `crate::ir → crate::tree`, `IrFamily → TreeKind`, spec dir `semantic-tree/ → tree/`, living-doc scrub, TODO.md scrub) all landed and are no longer enumerated here — see the git log for those commits. A subsequent audit (below) found that the rename sweep stopped at type names and missed a substantial tail of internal function names, ~150 local-variable bindings, 11 test files, and stale `src/ir/` path references in two design docs.

**Decisions still locked (2026-05-12):**
- Per-domain tree types `SyntaxTree` / `DataTree` / `SqlTree` / `DocumentTree`; `TreeKind` with variants `Syntax` / `Data` / `Sql` / `Document`.
- Historical doc `docs/design-transform-redesign-exploration.md` stays frozen with a terminology-note banner — IR mentions there are intentional.
- `docs/design-ir-and-renderings.md` §9 documents the rename decision itself; "IR" mentions in that section are intentional historical record.

**Open question:** [S12-Z5] flagged that `DocumentTree` extraction (Markdown moves out of `DataTree`) may not have actually happened despite being marked done. Verify before declaring the original eleven-step slice fully closed.

### Audit findings — 2026-05-14

Search: `\bIr[A-Z]|\bir_|_ir\b` across `tractor/src` and `tractor/tests`. 306 hits across 28 source files + 5 test files. Categorized below.

- [ ] [S12-Z12] **Public-API function rename: `tree::lower_to_data_ir → tree::lower_to_data`.** Defined at [`tree/to_data.rs:54`](tractor/src/tree/to_data.rs#L54), re-exported from [`tree/mod.rs:142`](tractor/src/tree/mod.rs#L142). Callers: [`xpath/match_result.rs:159`](tractor/src/xpath/match_result.rs#L159) and three test sites in `to_data.rs` (lines 1428, 1499, 1527). Single public-API surface still carrying "IR" in its name.

- [ ] [S12-Z13] **Internal-function renames** (low public surface, mechanical):
  - `tree::render::common::write_ir → write_tree` and `write_ir_with_source → write_tree_with_source` ([`tree/render/common.rs`](tractor/src/tree/render/common.rs)). Called by all 8 language `render_source.rs` files.
  - `mutation::xpath_upsert::update_existing_via_data_ir / update_existing_via_syntax_ir / insert_new_via_data_ir / render_data_ir_with_spans` ([`mutation/xpath_upsert.rs`](tractor/src/mutation/xpath_upsert.rs) lines 293 / 402 / 551 / 497). Drop the `_ir` / `_data_ir` suffix or replace with `_data_tree` / `_syntax_tree`.
  - `tree::coverage::collect_ir_ranges → collect_tree_ranges` ([`tree/coverage.rs:244`](tractor/src/tree/coverage.rs#L244)).
  - `languages::rust_lang::lower::clone_ir → clone_tree` ([`languages/rust_lang/lower.rs:1753`](tractor/src/languages/rust_lang/lower.rs#L1753)).

- [ ] [S12-Z14] **Local-variable cleanup in `lower.rs` files** (~150 occurrences, mechanical rename, no public surface). Patterns: `object_ir`, `name_ir`, `type_ir`, `value_ir`, `path_ir`, `next_ir`, `inner_ir`, `seg_ir`, `pattern_ir`, `index_ir`, `case_ir`, `ir_tree`, `ir_range`, `ir_root`, `ir_xot`, `ir_text`, `ir_xml`, `ir_render`, `ir_view`. Highest concentration: [`languages/csharp/lower.rs`](tractor/src/languages/csharp/lower.rs) (39 hits), [`languages/typescript/lower.rs`](tractor/src/languages/typescript/lower.rs) (43 hits), [`languages/java/lower.rs`](tractor/src/languages/java/lower.rs) (24 hits), [`tree/to_xot.rs`](tractor/src/tree/to_xot.rs) (18 hits, mostly `ir_range`). Also `parser/mod.rs:303,307,308,316,421,423,424,431` (`ir_tree` local). Suggested replacements: drop `_ir` suffix entirely (`object_ir → object`) or rename to `_tree` where ambiguity exists.

- [ ] [S12-Z15] **Test file renames in `tractor/tests/`.** Eleven files still prefixed `ir_*`: `ir_csharp_json_parity.rs`, `ir_csharp_parity.rs`, `ir_go_missing_kinds.rs`, `ir_java_missing_kinds.rs`, `ir_php_missing_kinds.rs`, `ir_python_blueprint.rs`, `ir_python_missing_kinds.rs`, `ir_python_parity.rs`, `ir_ruby_missing_kinds.rs`, `ir_rust_missing_kinds.rs`, `ir_typescript_missing_kinds.rs`. Five of them carry `ir_*` identifiers inside (function names, locals, doc comments). The `*_parity.rs` files compared the IR pipeline against the legacy walk_transform output — that comparison is moot now (S6 closed it). Consider whether to rename (`tree_<lang>_*`) or delete the parity tests outright.

- [ ] [S12-Z16] **Doc-comment / module-comment scrub** (no behavior change):
  - [`tree/types.rs:15`](tractor/src/tree/types.rs#L15) references `source[root_ir.range]` in the module docstring → `source[root_tree.range]` (or just rephrase).
  - [`docs/pipeline-architecture.md`](docs/pipeline-architecture.md) lines 3, 79, 123, 140 still reference `tractor/src/ir/` paths that no longer exist (`tractor/src/tree/` now).
  - [`docs/design-ir-and-renderings.md`](docs/design-ir-and-renderings.md) line 473 references `docs/pipeline-architecture.md` as needing scrub — recursive dangling pointer.

- [ ] [S12-Z17] **Decide: rename `docs/design-ir-and-renderings.md` itself?** The filename still carries the old vocabulary. The body deliberately documents the IR→tree rename (§9 — that section's IR mentions are intentional historical record), so renaming the file forces an update to every link that points to `design-ir-and-renderings.md`. Candidates: `design-trees-and-renderings.md` or leave as-is with a terminology banner in the header. Decision deferred — flag only.

- [ ] [S12-Z18] **`tractor/todo/41-ir-migration-status.md` — archive or delete.** Pre-S12 planning snapshot from 2026-05-07. Content is fully superseded by the shipped renames. Either move under a clearly-archived path or delete.

---

## Side cleanups (smaller, can interleave)

Each is one closeable invariant. No full slice header — too small to warrant one.

- [ ] [C1] **No `LanguageOps` entry on the tree path declares `field_wrappings` (read only by the legacy `XeeBuilder`).**
  - Depends on S2 routing tree languages away from `XeeBuilder`. Languages still on the legacy path retain their wrappings.

- [ ] [C2] **One source of truth for emitted element names: either `SyntaxTree` variants alone (per-language `TractorNode` enums deleted), or `TractorNode` generated from `SyntaxTree`.**
  - Decision point. The choice cascades into S10F and the shape-contract walks in S3E.

- [ ] [C3] **`transform::builder::XotBuilder` does not exist.**
  - Depends on S6 (WASM moved off it) and S2 (legacy languages routed through `XeeBuilder` only).

- [ ] [C4] **`output::xml_node_to_json` is reachable only by genuine XPath partial matches; the legacy tree/DataTree fallthrough is gone.**
  - Depends on S5 + S8.

- [ ] [C5] **Exactly one language registry exists. `tractor/src/languages/info.rs::LANGUAGES` either is gone or is a typed view onto `tractor/src/languages/mod.rs::LANGUAGES`.**
  - Surfaced during S2-Z3: two `LANGUAGES` arrays both claim SSoT in their docstrings. `info.rs` carries `aliases`, `has_transforms`, `grammar_file` (web/wasm path); `mod.rs` carries `extensions`, `grammar`, `tree_kind`, `transform`, `post_transform`, etc.
  - Two paths: (a) merge `info.rs`'s extra fields into `LanguageOps` and delete `info.rs`; or (b) rewrite `info.rs::LANGUAGES` as a derivation of `mod.rs::LANGUAGES`, leaving `Language` (enum) and `LanguageInfo` (struct) as a thin facade if external consumers need them.
  - Today `info.rs::LANGUAGES` still lists c/cpp/html/css/bash/scala/lua/haskell/ocaml/r/julia/xml as `has_transforms: false`, while S2-Z3 dropped them from the parser side. After consolidation, `language_info::get_language_info("cpp")` should be `None` (or `xml`-only for the passthrough case) so the two registries never disagree.
