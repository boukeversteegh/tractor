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

- **W1.** Three parallel tree shapes, three parallel everything (`SyntaxTree`, `DataTree`, `SqlTree` each with their own `lower_*`, `to_xot`, `to_json`, dispatch arm, projection). → S2 + S5.
- **W2.** Module documentation lied about production status (`tree/mod.rs` claimed "experimental sketch on Python"; it's the production path). → S1 (closed).
- **W3.** Reverse path is on the wrong substrate (`render/*.rs` speaks `XmlNode`, supports 3 languages; `tree/source/*.rs` speaks `SyntaxTree`, supports 9, but is unwired). → S4.
- **W4.** Cross-cutting transforms still mutate rendered xot. `chain_inversion` was removed (S3A). What remains in per-language `post_transform.rs` is a mix of language-specific rewrites and shared cross-language helpers parameterised by per-language data. → S3.
- **W5.** WASM and CLI produce different output for migrated languages (WASM still uses `XotBuilder` + `walk_transform`). → S6.
- **W6.** Six places know the language list, none authoritative; alias-mapping (`csharp`/`cs`, `python`/`py`, …) duplicated five ways. → S2.
- **W7.** `XmlNode` mixes XML markup (Element/Text/Comment/PI) with XPath atomic data (Map/Array/Number/Boolean/Null) in one enum. → S8.
- **W8.** Cardinality plumbed twice — typed `Vec<SyntaxTree>` slots *and* `list="X"` attributes on rendered xot. → S3D.
- **W9.** `field_wrappings` and per-language `TractorNode` strum enums are dead-data for migrated languages. → C1, C2.
- **W10.** Old parse API surface (`parse_string_to_xot`, `parse_file_to_xee`, …) still exposed alongside the unified `parse()`. → S9.

The tree design itself (typed slots, byte-range anchoring, `Inline`/`Unknown` escape hatches, coverage audit) is sound — none of these slices refactor it. The three tree families staying separate as types is also fine; the cost only goes away if dispatch + projection + rendering stop being copy-pasted three ways (S2 + S5).

---

## S1 — tree module documentation reflects production reality

**Closed.** `tree/mod.rs` § Status no longer claims "experimental sketch"; it states the production fact.

---

## S2 — One language registry (kills W6)

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

**Why now.** Two reverse renderers exist and the wired one (`render/{csharp,json,yaml}.rs`) speaks `XmlNode` + `TreeMode::Data` and supports only csharp/json/yaml. The tree-aware renderer (`tree/source/*.rs`) is implemented for 9 languages but never reached from production. `tractor render` / `set` / `update` are stuck at 3 of 28 supported languages because the wired path can't see the tree.

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
    - [ ] [S4B-Z4] **csharp upsert via `SyntaxTree`.** Same wiring for the C# code path (`render::render_with_spans(xml_node, "csharp", …)`) — uses `SyntaxTree` instead of `DataTree`. Reuses `tree::source::render` for anchored rendering, adds span tracking.
      - **Deferred 2026-05-08.** No driving tests; legacy path was already broken (empty `SpanMap`). Needs `SyntaxTree` mutation primitives + `SyntaxTree`-direct span-tracking renderer (both L-sized). Reopen when a workflow requires it.
    - [x] [S4B-Z5] **Retire transitional shims in `render::json`/`render::yaml`.** Done — strict `field=` checks restored; `render::yaml::tests::sequence` rewritten to legacy shape. (Modules deleted entirely at S4D shortly after.)

- [x] [S4C] **`render::parse_xml` and `render::parse_json` do not exist.** Done — removed alongside S4D.

- [x] [S4D] **`render/{csharp,json,yaml}.rs` do not exist (~1800 LOC deleted).** Done — whole `tractor/src/render/` directory retired (~2200 LOC); `xpath_upsert.rs` slimmed in lockstep; new `lang_supports_upsert` allowlist (json/yaml/yml).

---

## S5 — Single JSON projection: `SyntaxTree → DataTree → JSON` (kills part of W1)

**Goal.** One JSON projection algorithm for all three tree families. The principled `data_to_json` (typed `DataTree` reader) is the only path; the heuristic blocks in `tree_to_json` are gone.

**Why now.** Three concurrent JSON projection strategies coexist: legacy `xml_node_to_json` (XmlNode-based fallback), heuristic `tree_to_json` (1087 LOC of `$inline`/`$skip`/`$type`/plural-collapse rules papering over tree↔JSON impedance), and the principled `data_to_json` reading a typed `DataTree`. Each new edge case lands as another heuristic in `tree_to_json`; the in-progress `to_data` projection covers only `SyntaxTree::Class` so far.

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

- [/] [S5A] **`tree::to_data::lower_to_data_ir` covers every `SyntaxTree` variant deterministically.**
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
  - **OPTIONS: Boilerplate reduction in `to_data.rs`** (2026-05-08, raised by user). The `project` function is ~500 LOC of mechanically-similar match arms — declaration variants do `push_modifier_flags + named slots`, control-flow variants do `condition + body-flatten + else?`, op-bearing expressions do `op + operands`, etc. Three plausible reduction approaches; pick one (mark `[x]`) to commit.
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

## S6 — WASM uses the unified parse (kills W5)

**Goal.** The web playground and the CLI produce *byte-identical* semantic XML for the same source on every migrated language. WASM runs `parse_with_ir_pipeline_to_xee` (or its WASM-friendly variant), not `XotBuilder` + `walk_transform`.

**Why now.** `wasm/mod.rs` imports `XotBuilder` + `walk_transform` + `get_transform` and never touches `crate::tree`. The web playground (`web/src/tractor.ts`) calls into it. Since the tree migration changed the semantic XML shape for migrated languages, the playground silently produces *different* output than the CLI for the same source — queries that work in one fail in the other.

**Depends on.** S2 (registry-driven dispatch makes the WASM-side fan-out one place).
**Unblocks.** WASM playground accuracy — currently a quiet correctness bug for users.
**Independent of.** S3, S4, S5, S7, S8, S9.

**Size.** M. Either build a `tree_sitter::Tree`-shaped input from `SerializedNode` on the Rust side, or extract a WASM-friendly variant of `parse_with_ir_pipeline_to_xee`.
**Reversibility.** High. WASM is its own crate boundary; revert is one file.

**Invariants when closed:**
- `wasm::parse_to_xml` does not import `XotBuilder` or `walk_transform`.
- The web playground produces identical output to `tractor -x` on the same source for every migrated language (verified by S6A fixtures).

### Tasks

- [ ] [S6A] **A fixture set under `tests/wasm_parity/` captures concrete WASM↔CLI divergences for migrated languages.**
  - Small input file per language; expected = CLI output; observed = WASM output. Used by S6C as the parity oracle.

- [ ] [S6B] **`wasm::parse_to_xml` runs the tree pipeline.**
  - Either build a `tree_sitter::Tree`-shaped input from `SerializedNode` on the Rust side, or extract a WASM-friendly variant of `parse_with_ir_pipeline_to_xee`.

- [ ] [S6C] **The S6A fixtures pass: web ↔ CLI divergence is zero for every migrated language.**

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

- [/] [S10B] **No `tree/render/<lang>.rs` per-language emitter exists; `languages/<lang>/render_source.rs` exists in its place.** (File-move portion done; the registry-field `render_canonical: Option<RenderCanonicalFn>` on `LanguageOps` was deferred — `tree/render/mod.rs::render` still uses a match dispatch into the new per-language locations. Pure file-move-and-import-update; the registry-plumbing refactor is a separate concern.)
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

## S12 — Drop the tree vocabulary entirely; per-domain tree types (`SyntaxTree` / `DataTree` / `SqlTree` / `DocumentTree`)

**Goal.** "tree" disappears from the codebase, comments, and docs. Rust types rename to per-domain trees; `crate::tree → crate::tree`. Markdown extracts from `DataTree` into a new `DocumentTree`. Vocabulary in docs: "tree" / "tree node", not "tree" / "tree node". Per the decisions recorded in `docs/design-tree-and-renderings.md` §9 (2026-05-11 / 2026-05-12).

**Why now.** Design-doc rewrite (S11 + the tree-and-renderings doc) commits to "tree" as the user-facing vocabulary. Keeping the Rust types named `SyntaxTree` while docs say "tree" creates a permanent translation tax for new readers. "Intermediate representation" leaks an implementation framing into the API.

**Depends on.** None structural — pure mechanical rename + Markdown extraction. Coordinate with S11 (in-flight): if both are happening, do S12 first so S11's node work uses the new names.
**Unblocks.** Cleaner spec docs (no need for a "tree means `SyntaxTree` in code" note); fresh-reader onboarding.
**Independent of.** S1–S10, S11's structural changes (S12 is naming + Markdown extraction; S11 is shape).

**Size.** L (~50 files touched, plus Markdown extraction; mostly mechanical). Per-Z-step commits.
**Reversibility.** High — git revert restores the old names. Public API churn for anyone using `tractor::tree::*` externally.

**Decisions locked (2026-05-12):**
- Type renames: `Ir → SyntaxTree`, `DataIr → DataTree`, `SqlIr → SqlTree`. New `DocumentTree` extracted from Markdown lowering.
- Module: `crate::tree → crate::tree`.
- `IrFamily → TreeKind` (variants `Syntax` / `Data` / `Sql` / `Document`).
- Spec dir: `specs/tractor-parse/semantic-tree/ → specs/tractor-parse/tree/`.
- Historical doc `docs/design-transform-redesign-exploration.md`: leave body frozen, add terminology-note banner.

**Invariants when closed:**
- `crate::tree::SyntaxTree`, `crate::tree::DataTree`, `crate::tree::SqlTree`, `crate::tree::DocumentTree` exist; `SyntaxTree` / `DataTree` / `SqlTree` do not.
- `crate::tree` module path replaces `crate::tree`.
- `TreeKind` (with `Syntax` / `Data` / `Sql` / `Document` variants) replaces `TreeKind`.
- Markdown lowers to `DocumentTree`, not `DataTree`.
- `specs/tractor-parse/tree/` exists; `specs/tractor-parse/semantic-tree/` does not.
- All living docs scrubbed of "tree" / "tree" vocabulary. Historical doc carries a terminology-note banner.
- TODO.md vocabulary scrubbed.
- All tests pass under the new names.

### Tasks

- [ ] [S12-Z1] **`SyntaxTree` enum renamed to `SyntaxTree`; `SyntaxTree::*` constructors renamed to `SyntaxTree::*`.**
  - Find-replace across `tractor/src/` and `tractor/tests/`. ~50 files; bulk of the type-rename work.
  - Module path stays `crate::tree::*` for this step (it's just the type name).
  - Verify: `cargo check` clean; `cargo test` green.

- [ ] [S12-Z2] **`crate::tree` module renamed to `crate::tree`; `pub mod tree;` → `pub mod tree;`.**
  - `git mv tractor/src/tree tractor/src/tree`; update `tractor/src/lib.rs`.
  - Update every `use crate::tree::*` to `use crate::tree::*`.
  - Verify: `cargo check`, `cargo test`.

- [ ] [S12-Z3] **`DataTree` renamed to `DataTree` (Markdown still inside for now).**
  - Mechanical type rename. Markdown extraction is Z5 (kept separate to minimize blast radius per commit).
  - Verify: `cargo check`, `cargo test`.

- [ ] [S12-Z4] **`SqlTree` renamed to `SqlTree`.** Same mechanical pattern as Z3.

- [ ] [S12-Z5] **`DocumentTree` created; Markdown lowering extracted from `DataTree` into `DocumentTree`.**
  - New file `tractor/src/tree/document.rs` (or `tree/document/types.rs`) with `enum DocumentTree`. Variants drawn from the current Markdown `DataTree::Document` + `DataTree::Element { name, markers, ... }` shapes, but typed: `DocumentTree::Document`, `DocumentTree::Heading { level: HeadingLevel, ... }`, `DocumentTree::List { ordered: ListOrdering, items }`, `DocumentTree::CodeBlock { language, code }`, `DocumentTree::BlockQuote`, `DocumentTree::ThematicBreak`, etc.
  - Move `tractor/src/tree/markdown_data.rs` → `tractor/src/tree/document/lower.rs` and rewrite to lower into `DocumentTree`.
  - Add `to_xot` / `to_json` for `DocumentTree`. Output shape unchanged from today's `DataTree::Element` projections (keeps existing XPath queries / blueprint tests working).
  - Update language registry: Markdown's `tree_kind` switches from `Data` to `Document`.
  - Verify: existing Markdown blueprint snapshots unchanged; `cargo test` green.

- [ ] [S12-Z6] **`TreeKind → TreeKind`; variants `Syntax` / `Data` / `Sql` / `Document`.**
  - Rename the enum on `LanguageOps`. Update every callsite.
  - Per-language values updated (most languages get `TreeKind::Syntax`; data languages `Data`; T-SQL `Sql`; Markdown `Document` per Z5).
  - Verify: `cargo check`, `cargo test`.

- [ ] [S12-Z7] **Module-level renames flow through.**
  - `to_xot::render_tree_* → render_tree_*`, `data_tree → data_tree`, etc.
  - Audit `crate::tree::*` re-exports and supporting type aliases (now under `crate::tree::*`).
  - Doc-comment scan: "tree" → "tree" or specific tree type; "tree node" → "tree node".

- [ ] [S12-Z8] **`specs/tractor-parse/semantic-tree/` renamed to `specs/tractor-parse/tree/`.**
  - `git mv` the directory.
  - Update cross-links in `specs/tractor-parse/*.md`, `docs/*.md`, CLAUDE.md, README files.
  - Subspec contents scrubbed for tree vocabulary; future passes may split into `syntax-tree.md` / `data-tree.md` / `sql-tree.md` / `document-tree.md` subdivisions.

- [ ] [S12-Z9] **Living `docs/*.md` files scrubbed.**
  - `docs/pipeline-architecture.md` (15 mentions), `docs/transform-validation-architecture.md` (4 mentions), `docs/design-projection-pipeline.md` (105 mentions). Rewrite tree references to the new tree names.
  - `docs/design-transform-redesign-exploration.md` — body unchanged; add terminology-note banner at top mapping `Ir → SyntaxTree`, `DataIr → DataTree`, `SqlIr → SqlTree`.
  - `docs/design-tree-and-renderings.md` itself — rename to `docs/design-tree-and-renderings.md` (or move to `specs/tractor-parse/tree/renderings.md` when ready per §6 of the design doc).

- [x] [S12-Z10] **TODO.md scrubbed.**
  - "IR variant" → "tree node", "IR shape" → "tree structure", `Ir::*` → `SyntaxTree::*`, `DataIr → DataTree`, `SqlIr → SqlTree`, `IrFamily → TreeKind`, `crate::ir → crate::tree`, and the various `_ir_` / `_ir`-suffixed identifiers across slice descriptions and invariants.
  - Code-pattern references like `Vec<Ir>` / `Box<Ir>` in S11 task descriptions renamed to `Vec<SyntaxTree>` / `Box<SyntaxTree>`.
  - File-path references like `ir/source/*.rs` updated to `tree/source/*.rs` since the directory was renamed in Z2.
  - Historical references in done items (`[x]` tasks describing what S5C / S5A did when the code was named `Ir`) are kept as-is for accuracy.

- [ ] [S12-Z11] **`specs/tractor-parse/dual-view/`, `specs/codexpath/cli/output-options/json-format/`, `specs/cli-output-design.md` scrubbed.**
  - Find-replace tree vocabulary; align with the §5 bucketing from the design doc as the principles get re-categorized.

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
