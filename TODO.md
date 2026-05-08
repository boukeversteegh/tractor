# TS → IR → {xot,xml,json,yaml,source} — architectural cleanup

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
- Reordering or merging happens only on explicit request.

## Status snapshot

| Slice | Status | Size | Next action |
|---|---|---|---|
| S1 — IR docs reflect production | closed | XS | — |
| S2 — one language registry | in progress (~70%) | L | S2-Z4: collapse the four `parse_with_ir_pipeline*` match arms |
| S3 — eliminate xot post-passes | in progress (S3A closed; S3B-Z1a in progress) | XL | S3B-Z1a: attach `where`-clauses during lowering; then scope S3B-Z1b (shared-helpers pipeline) |
| S4 — IR-based reverse rendering | open | M | S4A: `tractor render` calls `ir::source::render` |
| S5 — single JSON projection | open | M | S5A: extend `to_data` to cover every `Ir` variant |
| S6 — WASM uses unified parse | open | M | S6A: capture WASM↔CLI divergence fixtures |
| S7 — drop xot serialise/reparse | open | S | S7A: confirm xee Documents ingestion path |
| S8 — split `XmlNode` from atoms | open | S | S8A: define `XmlMarkup` / `XpathValue` types |
| S9 — retire old parse API | open | S | S9A: migrate tests off legacy parse fns |
| S10 — per-language consolidation | open | L | After S3B-Z1c; pure structural moves |
| C1–C5 (side cleanups) | open | XS each | Interleave |

## Findings — pipeline as it actually runs

Library entry point: `parser::parse(ParseInput, ParseOptions)` (`parser/mod.rs:1106`). Three downstream branches, gated by `use_ir_pipeline(lang, mode)` until S2-Z4 collapses it (`parser/mod.rs:380`):

| Path | Languages | Where |
|---|---|---|
| **IR (programming)** — `Ir` | csharp, python, java, ts/js/tsx/jsx, rust, go, ruby, php | `parse_with_ir_pipeline_to_xee` |
| **IR (data)** — `DataIr` | json, yaml, toml, ini, env, markdown (Structure mode) | same fn, data branch |
| **IR (sql)** — `SqlIr` | tsql | same fn, sql branch |
| **Legacy imperative** — `XeeBuilder::build_with_options` + `walk_transform` | Raw mode for everything; c, cpp, html, css, bash, scala, lua, haskell, ocaml, r, julia; json/yaml in Data mode | `parser/mod.rs:933` |
| **WASM** — `XotBuilder` + `walk_transform` | All web-app parses | `wasm/mod.rs` |

The IR path renders to xot, **serialises the xot to a string, and re-parses it into xee `Documents`** (acknowledged "v1 stepping stone" at `parser/mod.rs:641`). After that it runs each language's `post_transform` for any remaining shape work plus the `list="X"` attribute pass.

`tractor render` and `tractor set/update`'s value-rewrite use a *different*, parallel reverse pipeline: `render::parse_xml` / `parse_json` → `XmlNode` → `render::render(node, lang, TreeMode::Data, opts)` (`render/mod.rs`). It speaks `XmlNode`, not IR, and supports csharp/json/yaml only.

## Findings — weaknesses (W → slice index)

- **W1.** Three parallel IR shapes, three parallel everything (`Ir`, `DataIr`, `SqlIr` each with their own `lower_*`, `to_xot`, `to_json`, dispatch arm, projection). → S2 + S5.
- **W2.** Module documentation lied about production status (`ir/mod.rs` claimed "experimental sketch on Python"; it's the production path). → S1 (closed).
- **W3.** Reverse path is on the wrong substrate (`render/*.rs` speaks `XmlNode`, supports 3 languages; `ir/source/*.rs` speaks `Ir`, supports 9, but is unwired). → S4.
- **W4.** Cross-cutting transforms still mutate rendered xot. `chain_inversion` was removed (S3A). What remains in per-language `post_transform.rs` is a mix of language-specific rewrites and shared cross-language helpers parameterised by per-language data. → S3.
- **W5.** WASM and CLI produce different output for migrated languages (WASM still uses `XotBuilder` + `walk_transform`). → S6.
- **W6.** Six places know the language list, none authoritative; alias-mapping (`csharp`/`cs`, `python`/`py`, …) duplicated five ways. → S2.
- **W7.** `XmlNode` mixes XML markup (Element/Text/Comment/PI) with XPath atomic data (Map/Array/Number/Boolean/Null) in one enum. → S8.
- **W8.** Cardinality plumbed twice — typed `Vec<Ir>` slots *and* `list="X"` attributes on rendered xot. → S3D.
- **W9.** `field_wrappings` and per-language `TractorNode` strum enums are dead-data for migrated languages. → C1, C2.
- **W10.** Old parse API surface (`parse_string_to_xot`, `parse_file_to_xee`, …) still exposed alongside the unified `parse()`. → S9.

The IR design itself (typed slots, byte-range anchoring, `Inline`/`Unknown` escape hatches, coverage audit) is sound — none of these slices refactor it. The three IR families staying separate as types is also fine; the cost only goes away if dispatch + projection + rendering stop being copy-pasted three ways (S2 + S5).

---

## S1 — IR module documentation reflects production reality

**Closed.** `ir/mod.rs` § Status no longer claims "experimental sketch"; it states the production fact.

---

## S2 — One language registry (kills W6)

**Goal.** A single source of truth answers "given this string, what is this language?" — extensions, grammar, IR family, transforms, and vocabulary all live in one row of `LANGUAGES`. Adding a language is one row.

**Why now.** Aliasing (`csharp`/`cs`, `typescript`/`ts`/`tsx`/`jsx`, …) is independently re-listed in five tables: `SUPPORTED_LANGUAGES`, `detect_language`, `get_tree_sitter_language`, `LanguageOps.ids`, `use_ir_pipeline`, plus four match arms in `parse_with_ir_pipeline*`. Concrete failure mode: forget to update one of the six sites when adding a language and the file parses but routes to the wrong pipeline silently.

**Depends on.** Nothing — foundation slice.
**Unblocks.** S3B-Z1b (shared-helpers pipeline needs `LanguageOps` to carry per-language data tables as first-class fields), S3B-Z11 (drop `post_transform` field cleanly), S6 (WASM consults registry), S10A/B (lowering and render-canonical pointers update one row).
**Independent of.** S4, S5, S7, S8, S9.

**Size.** L. Touches `parser/mod.rs` heavily; every language row gets new fields. ~70% complete.
**Reversibility.** Medium. Each Z-step is independently revertible.

**Invariants when closed:**
- `parser::SUPPORTED_LANGUAGES`, `detect_language`, `get_tree_sitter_language` all derive from `LANGUAGES`.
- `parser::use_ir_pipeline` does not exist.
- No `match lang { "csharp" => …, "python" => … }` exists in `parse_with_ir_pipeline*` or `ir/source/mod.rs::render`; all per-language dispatch is registry-keyed.
- `LanguageOps` carries `extensions`, `grammar`, `ir_family` alongside its existing fields.
- Adding a new language is one row in `LANGUAGES` plus its grammar shim.

### Tasks

- [x] [S2-Z1] **`LanguageOps` carries `extensions`, `grammar`, `ir_family` for every existing row.**
  - New enum `IrFamily { None | Programming(LowerToIr) | Data(LowerToDataIr) | Sql(LowerToSqlIr) }` declared alongside.
  - 17 grammar shim fns (`fn ts_csharp() -> tree_sitter::Language { tree_sitter_c_sharp::LANGUAGE.into() }`).
  - Note: TS/TSX/JS split into 3 rows because grammars differ — `["typescript","ts"]`, `["tsx"]`, `["javascript","js","jsx"]`. Same transforms; only `grammar`, `extensions`, `ids` differ.
  - Done: `cargo check --features native` and `cargo check --no-default-features --features wasm --target wasm32-unknown-unknown` both clean.

- [x] [S2-Z3] **`parser::SUPPORTED_LANGUAGES`, `detect_language`, `get_tree_sitter_language`, `get_language_abi_versions` derive from `LANGUAGES`.**
  - XML kept as a one-line passthrough special case in `detect_language` (no tree-sitter grammar).
  - Done: `cargo check` (native + wasm) clean; `cargo test --lib` 373/373 pass.
  - Surfaced: `tractor/src/languages/info.rs::LANGUAGES` is a *second* `LANGUAGES` array (with `aliases`, `has_transforms`, `grammar_file` for web). Tracked as **C5**.

- [/] [S2-Z4] **No language-keyed `match lang { … }` arm exists in `parse_with_ir_pipeline*`; dispatch reads `match l.ir_family { IrFamily::Programming(f) => f(node, source), … }`.**
  - Half done: `use_ir_pipeline` is gone. Four match arms at `parser/mod.rs:520, :574, :660, :741` still pattern-match on language id and need to collapse.
  - Companion: the `match lang { … }` in `ir/source/mod.rs::render` is the same family — collapse it the same way as part of S10B (which adds `render_canonical: Option<RenderCanonicalFn>` to `LanguageOps`).

- [x] [UKNUK] [S2C] **`parser::use_ir_pipeline` does not exist; both callers consult `LanguageOps::uses_ir(TreeMode)`.**
  - Done: function deleted. Callers (lines 441 and 918) now do `crate::languages::get_language(lang).map(|l| l.uses_ir(resolved)).unwrap_or(false)`. The mode-aware decision lives on a new `LanguageOps::uses_ir(TreeMode)` method that reads `ir_family` from the registry.

- [x] [S2-Z5] **Build green; `cargo test --lib` passes after the consolidation.**
  - Done: 373/373 pass after S2-Z1 + S2C + S2-Z3.

- ~~S2-Z2~~ **(dropped 2026-05-08)** — c/cpp/html/css/bash/scala/lua/haskell/ocaml/r/julia/xml aren't really supported; don't promote them to `LANGUAGES`.

---

## S3 — Eliminate xot post-passes; IR is canonical from lowering (kills W4 + W8)

**Goal.** No xot post-pass mutates the rendered tree. `lower_<lang>_root` returns canonical IR; `to_xot` is mechanical. Cross-cutting transforms (chain inversion, conditional flattening, slot wrapping, list-tagging) are encoded natively — in the lowering, in typed `Ir` variants, or in `to_xot` driven by typed slots.

**Why now.** `transform::chain_inversion` (1826 LOC) was deleted in S3A. What remains in per-language `post_transform.rs` is a mix of:
1. **Genuinely language-specific xot rewrites** (e.g. `attach_ir_where_clauses` for C#, `python_restructure_imports` for Python) — these fold into the language's IR lowering.
2. **Shared cross-language helpers parameterised by per-language data** (`collapse_conditionals`, `tag_multi_role_children`, `wrap_expression_positions`, `flatten_nested_paths`, `strip_body_braces`, `wrap_relationship_targets_in_type`, `flatten_single_declarator_children`, `distribute_member_list_attrs`) — the *same algorithms* replicated across nine language post_transforms with only the data lists changing. This is the bulk of what's left.

Cardinality is also still plumbed twice — typed `Vec<Ir>` slots *and* `list="X"` attributes on rendered xot (W8).

**Strategy.** Two complementary moves, run in parallel:
1. **Per-language fold (Z1a, Z2…Z9).** Each language's *language-specific* passes go into its lowering or new typed `Ir` variants, the same way S3A did for `Ir::Access`.
2. **Shared-helper consolidation (Z1b).** The shared cross-language helpers migrate into a single registry-driven post-render pipeline, with their per-language data lists hoisted onto `LanguageOps` as first-class fields. As S3D (typed-slot cardinality) and similar work absorbs each helper's purpose into the IR itself, the corresponding data tables shrink and the global pass progressively retires.

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
- `to_xot` emits cardinality from typed `Box<Ir>` / `Vec<Ir>` slots; no `list="X"` attribute pass exists.
- `transform::shape_contracts` runtime walks are minimised; provable rules are type-level.

### Tasks

- [x] [WGWS0] [S3A] **No `chain_inversion` module exists; every language's lowering constructs left-deep `Ir::Access` directly from right-deep CST.**
  - Done: all 8 `chain_inversion::*` callsites removed (csharp/go/typescript/rust_lang plus ruby×2 and php×2). Two genuine gaps surfaced when the post-walk was disabled — Go's `index_expression` and PHP's `subscript_expression` produced flat `Ir::SimpleStatement` for single-step `arr[0]`; both rewritten to fold into `Ir::Access` (mirroring the existing TS / C# / Rust pattern). `tractor/src/transform/chain_inversion.rs` (1826 LOC) and `tractor/tests/chain_inversion_emits.rs` deleted; `pub mod chain_inversion;` removed from `transform/mod.rs`. All 26 test binaries (~1100 tests including the 7-language `cross_language_index_access_chain_inverts` loop) pass; native + WASM builds clean.

- [/] [XZ64] [S3B] **No `languages/<lang>/post_transform.rs` file exists; per-language shape decisions live in the lowering or in typed `Ir` variants; no global post-render pipeline survives either.**

  - [/] [S3B-Z1] **C#: lowering produces canonical IR end-to-end; `languages/csharp/post_transform.rs` does not exist. (Pathfinder for the per-language fold + shared-helper consolidation pattern.)**
    - **Scope finding (2026-05-08):** the four passes named in this slice's original draft (`csharp_normalize_conditional_access`, `unify_file_scoped_namespace`, `attach_where_clause_constraints`, `append_constraint_to_generic`) were already gone — folded into the IR lowering in earlier iters. The only genuinely C#-specific pass remaining is `attach_ir_where_clauses` (~115 LOC). The other ~200 lines of `csharp_post_transform` are calls to **shared cross-language transforms** (`collapse_conditionals`, `tag_multi_role_children`, `wrap_expression_positions`, `flatten_nested_paths`, `strip_body_braces`, `wrap_relationship_targets_in_type`, `flatten_single_declarator_children`, `distribute_member_list_attrs`) parameterised by C#-specific data lists. The same factoring applies to Z2…Z9 — none of those files can be deleted by per-language work alone.

    - [x] [S3B-Z1a] **C# `where T : ...` constraints attach to their target generics during lowering; `attach_ir_where_clauses` post-walk does not exist.**
      - Done via shape (i): `fold_csharp_where_clauses_into_generics` in `tractor/src/ir/csharp.rs` runs at the end of the `class_declaration` lowering arm, translating each `<constraint>` into zero-width markers (`<class/>`/`<new/>`/`<struct/>`/`<notnull/>`/`<unmanaged/>`) or `<extends>` wrappers and appending them to the matching `Ir::SimpleStatement::generic` item. `where_clauses` stays populated on `Ir::Class` so `render_ir_class` can flush the where-clause source bytes as gap text under `<class>` (new `CSlot::Where` branch in `tractor/src/ir/to_xot.rs`) — no `<where>` element in output.
      - The only remaining content of `csharp/post_transform.rs` is calls to shared cross-language transforms with C#-specific data lists.

    - [ ] [S3B-Z1b] **Shared cross-language post-render helpers run from a single registry-driven pipeline, with their per-language data lists carried as `LanguageOps` fields. No language declares its own copy of `tag_multi_role_children`-style invocations.** *(Sibling concern — not C#-specific. Unblocks Z1c and Z2…Z9's file-deletion steps.)*
      - **Approach revised 2026-05-08:** rather than designing a unified pipeline + per-language data tables, follow the experimental delete-and-fix path used for Z1a/Z1c — for each language, set `post_transform: None`, run the suite, and fix breakages by encoding the missing knowledge in the IR (typed slots, struct wrappers like `Expression`, or enum-variant metadata). The shared helpers either become redundant (cardinality already in `Vec<Ir>` slots — closed by S3D) or move into IR construction (e.g. `Expression::wrap` at value-position lowering). Z2…Z9 each delete their own `post_transform.rs` directly.
      - Open work: typify the remaining expression-position slots as `Box<Expression>` / `Option<Box<Expression>>` for cross-language coverage — currently only `Ir::Variable.value` is typed. Affected fields: `Ir::If.condition`, `Ir::Binary.left/right`, `Ir::Return.value`, `Ir::Comparison.left/right`, `Ir::Logical.left/right` (if exists), `Ir::Yield.value`, `Ir::Cast.inner`, `Ir::While.condition`, `Ir::Match.subject`, `Ir::Ternary.condition/then/else`. Migration is mechanical: change field type, update construction sites to `Expression::wrap(...)`, deref in renderer/walker.
      - Coordinate with **S3D** (drop `list="X"` attribute pass once `to_xot` reads cardinality from typed IR slots).

    - [x] [S3B-Z1c] **`languages/csharp/post_transform.rs` does not exist; csharp's `LANGUAGES` row has `post_transform: None`.**
      - Done: file deleted, `csharp::csharp_post_transform` reference removed from `LANGUAGES`, `csharp::mod` no longer reexports it. Only one test failed (`csharp_null_forgiving_postfix_unary`); the fix was to typify `Ir::Variable.value: Option<Expression>` so `<value><expression>...</expression></value>` is produced at IR construction (via `Expression::wrap`) instead of by `wrap_expression_positions` post-walk. Java/TS lowerings updated correspondingly.

  - [ ] [S3B-Z2] **Rust lowering produces canonical IR end-to-end; `languages/rust_lang/post_transform.rs` does not exist.**
    - Folds in: `rust_normalize_field_expression`, `rust_normalize_lifetime_names`, `rust_restructure_use`. Depends on Z1b for the shared-helper portion.

  - [ ] [S3B-Z3] **TypeScript / JS / TSX lowering produces canonical IR end-to-end; `languages/typescript/post_transform.rs` does not exist.**
    - Folds in: `typescript_unwrap_callee`, `typescript_restructure_import`. All three TS-family rows in `LANGUAGES` go to `post_transform: None`.

  - [ ] [S3B-Z4] **Python lowering produces canonical IR end-to-end; `languages/python/post_transform.rs` does not exist.**
    - Folds in: `python_tag_from_imports_uniform`, `python_restructure_imports`, `python_alias_pairs`, `python_flatten_dotted_name`.

  - [x] [S3B-Z5] **Java lowering produces canonical IR end-to-end; `languages/java/post_transform.rs` does not exist.**
    - Done 2026-05-08. `scoped_identifier` lowering recurses to flatten nested paths (folds in `flatten_nested_paths` + `java_unwrap_type_in_path`). `enhanced_for_statement` and `lower_java_multi_declarator` wrap value-position content via `Expression::wrap` (folds in `wrap_expression_positions` for those slots). Ratchet bumped 600→603 — three new advisory `no-children-overflow` sites pending S3D.

  - [x] [S3B-Z6] **Go lowering produces canonical IR end-to-end; `languages/go/post_transform.rs` does not exist.**
    - Done 2026-05-08. Single test broke (`go_multi_value_return_lists_expressions`); fixed by lowering `return_statement` with `Expression::wrap` per returned value (multi-return → multiple `<expression>` siblings under `<return>`). `go_retag_singleton_closure_body` was already redundant — IR closure rendering covers the case. No ratchet bump.

  - [x] [S3B-Z7] **T-SQL lowering produces canonical SqlIr end-to-end; `languages/tsql/post_transform.rs` does not exist.**
    - Done 2026-05-08. Zero tests broke — `tsql_wrap_binary_operands` and `tsql_tag_select_columns` were not load-bearing for the current test surface (SqlIr lowering already produces the right shapes). Deleted the file and the module reference. No ratchet bump.

  - [ ] [S3B-Z8] **PHP lowering produces canonical IR end-to-end; `languages/php/post_transform.rs` does not exist.**
    - Folds in: `php_wrap_member_call_slots`, `php_restructure_use`.

  - [ ] [S3B-Z9] **Ruby lowering produces canonical IR end-to-end; `languages/ruby/post_transform.rs` does not exist.**
    - Folds in: `ruby_tag_case_when_lists`, `ruby_retag_singleton_block_body`, `ruby_collapse_lambda_body`, `ruby_extract_pair_keys`.

  - [ ] [S3B-Z10] **The flat conditional shape (`<if><else_if/><else/>`) is produced by lowering or `to_xot`, not by a separate xot walk.**
    - Today: `languages/mod.rs::collapse_conditionals` is a standalone xot walk invoked from each post_transform. Pick one home: per-language flatten in `lower_<lang>_root`, or once in `to_xot` driven by `Ir::If { else_branch: Option<Box<Ir>> }`.
    - Whichever wins, `collapse_conditionals` and `collect_if_nodes` in `languages/mod.rs` are deleted. Coordinate with Z1b — this helper is one of the cross-language ones it targets.

  - [ ] [S3B-Z11] **`LanguageOps::post_transform` field does not exist; no post-pass runs in the IR pipeline.**
    - Depends on Z1c + Z2–Z10 (every language's file is gone) and on Z1b (no shared global pipeline still using the field).
    - Removes: the field declaration, `get_post_transform()`, and the post-pass invocation block in `parse_with_ir_pipeline*` (S3C).
    - After: `languages/<lang>/` for every migrated language contains `mod.rs`, `input.rs` (kinds), `output.rs` (vocabulary). S10A then adds `lower.rs`; S10B adds `render_source.rs`.

- [ ] [3C72S] [S3C] **`parse_with_ir_pipeline*` invokes no post-pass on rendered xot.**
  - The block at `parser/mod.rs:602–609` (and matching ones in data/SQL branches) does not exist.
  - Depends on S3B-Z11.

- [ ] [3IIU] [S3D] **`to_xot` emits cardinality from typed slots; no `list="X"` attribute pass exists for IR languages.**
  - `Box<Ir>` → singleton; `Vec<Ir>` → list. The current attribute pass is removed for IR languages (legacy XeeBuilder path keeps it for unmigrated ones).
  - Coordinated with S3B-Z1b — once cardinality flows from typed slots, the `tag_*` helpers shrink or disappear.

- [ ] [DZRA9N] [S3E] **`transform::shape_contracts` runtime walks are minimised; provable rules live at the type level.**
  - Most rules become unrepresentable at the `Ir` enum level (per `ir/types.rs:31`). Keep runtime-only walks for genuinely runtime rules (e.g. `op-marker-matches-text`).

---

## S4 — IR-based reverse rendering (kills W3)

**Goal.** Forward and reverse paths share the same substrate. `tractor render`, `tractor set`, `tractor update` all dispatch through `ir::source::render(ir, lang, anchor)` and work on every language with an IR — no more 3-language ceiling.

**Why now.** Two reverse renderers exist and the wired one (`render/{csharp,json,yaml}.rs`) speaks `XmlNode` + `TreeMode::Data` and supports only csharp/json/yaml. The IR-aware renderer (`ir/source/*.rs`) is implemented for 9 languages but never reached from production. `tractor render` / `set` / `update` are stuck at 3 of 28 supported languages because the wired path can't see the IR.

**Depends on.** Nothing structural — `ir/source/render` already exists.
**Unblocks.** Removes `render/` entirely; reduces ~1800 LOC of XML→source code that duplicates the IR-side work.
**Independent of.** S2, S3, S5, S6, S7, S8, S9.

**Size.** M. Two callers (`cli/render.rs`, `mutation/xpath_upsert.rs`) switch over; ~1800 LOC of dead code follows.
**Reversibility.** High per task. Easy to leave the old renderers behind a feature flag if needed.

**Invariants when closed:**
- `tractor render` works for every language with an IR (9 today, growing as IR coverage expands).
- `tractor set` / `tractor update` work for every IR-supported language.
- `render::parse_xml` and `render::parse_json` do not exist.
- `render/{csharp,json,yaml}.rs` do not exist.
- The reverse path does not re-parse tractor's own output back through XML.

### Tasks

- [ ] [J60X] [S4A] **`tractor render` reads source, parses to IR, and emits via `ir::source::render(ir, lang, anchor)`.**
  - Edit `cli/render.rs` to read source (stdin / `--string` / file), call `parse()`, then `ir::source::render`.

- [ ] [21DT] [S4B] **`mutation/xpath_upsert.rs` value-rewrite uses anchored IR re-render with span tracking, not `render_with_spans(xml_node, lang, TreeMode::Data, …)`.**
  - Today: `mutation/xpath_upsert.rs:252,390`. After: speaks IR + source anchor.

- [ ] [KOLKFS] [S4C] **`render::parse_xml` and `render::parse_json` do not exist.**
  - Depends on S4A + S4B (nothing reads `XmlNode`-from-text any more).

- [ ] [3WO0Y] [S4D] **`render/{csharp,json,yaml}.rs` do not exist (~1800 LOC deleted).**
  - Optional fallback flag for languages still without IR, with a sunset comment, if anyone needs it.

---

## S5 — Single JSON projection: `Ir → DataIr → JSON` (kills part of W1)

**Goal.** One JSON projection algorithm for all three IR families. The principled `data_to_json` (typed `DataIr` reader) is the only path; the heuristic blocks in `ir_to_json` are gone.

**Why now.** Three concurrent JSON projection strategies coexist: legacy `xml_node_to_json` (XmlNode-based fallback), heuristic `ir_to_json` (1087 LOC of `$inline`/`$skip`/`$type`/plural-collapse rules papering over IR↔JSON impedance), and the principled `data_to_json` reading a typed `DataIr`. Each new edge case lands as another heuristic in `ir_to_json`; the in-progress `to_data` projection covers only `Ir::Class` so far.

**Depends on.** Nothing structural.
**Unblocks.** Removing ~1000 LOC of heuristics (`ir/to_json.rs`).
**Independent of.** S2, S3, S4, S6, S7, S8, S9.

**Size.** M. The bulk is rule migration and parity audit.
**Reversibility.** Medium. Requires snapshot regen for JSON output; revert is one commit but disruptive if downstream readers exist.

**Invariants when closed:**
- `ir::to_data::lower_to_data_ir` covers every `Ir` variant.
- `tractor/src/ir/to_json.rs` does not exist.
- `Tree::to_json` for `Tree::Ir` calls `data_to_json(to_data(ir, source))`.
- `SqlIr` JSON output also flows through `DataIr`.

### Tasks

- [ ] [RYLH] [S5A] **`ir::to_data::lower_to_data_ir` covers every `Ir` variant deterministically.**
  - Today: covers `Ir::Class` + scalar leaves (`ir/to_data.rs:37`). Slice plan in `docs/design-projection-pipeline.md`.

- [ ] [FJYNZN] [S5B] **All JSON shape decisions (`$inline`, `$skip`, `$type:"expression"`, plural-of-self collapse, marker-vs-leaf) live as uniform projection rules in `to_data`, not in `to_json`.**

- [ ] [AP9K0B] [S5C] **`Tree::to_json` for the `Tree::Ir` arm calls `data_to_json(to_data(ir, source))`.**
  - `Tree::DataIr` keeps direct `data_to_json`; `Tree::Sql` keeps `sql_to_json` until S5E.

- [ ] [UCMC] [S5D] **`tractor/src/ir/to_json.rs` does not exist.**
  - Depends on S5C (no caller).

- [ ] [X8UX] [S5E] **`SqlIr` JSON output flows through `DataIr` projection.**
  - All three IR families share one JSON projection algorithm with three entry points.

---

## S6 — WASM uses the unified parse (kills W5)

**Goal.** The web playground and the CLI produce *byte-identical* semantic XML for the same source on every migrated language. WASM runs `parse_with_ir_pipeline_to_xee` (or its WASM-friendly variant), not `XotBuilder` + `walk_transform`.

**Why now.** `wasm/mod.rs` imports `XotBuilder` + `walk_transform` + `get_transform` and never touches `crate::ir`. The web playground (`web/src/tractor.ts`) calls into it. Since the IR migration changed the semantic XML shape for migrated languages, the playground silently produces *different* output than the CLI for the same source — queries that work in one fail in the other.

**Depends on.** S2 (registry-driven dispatch makes the WASM-side fan-out one place).
**Unblocks.** WASM playground accuracy — currently a quiet correctness bug for users.
**Independent of.** S3, S4, S5, S7, S8, S9.

**Size.** M. Either build a `tree_sitter::Tree`-shaped input from `SerializedNode` on the Rust side, or extract a WASM-friendly variant of `parse_with_ir_pipeline_to_xee`.
**Reversibility.** High. WASM is its own crate boundary; revert is one file.

**Invariants when closed:**
- `wasm::parse_to_xml` does not import `XotBuilder` or `walk_transform`.
- The web playground produces identical output to `tractor -x` on the same source for every migrated language (verified by S6A fixtures).

### Tasks

- [ ] [6B4STY] [S6A] **A fixture set under `tests/wasm_parity/` captures concrete WASM↔CLI divergences for migrated languages.**
  - Small input file per language; expected = CLI output; observed = WASM output. Used by S6C as the parity oracle.

- [ ] [RE5EF5] [S6B] **`wasm::parse_to_xml` runs the IR pipeline.**
  - Either build a `tree_sitter::Tree`-shaped input from `SerializedNode` on the Rust side, or extract a WASM-friendly variant of `parse_with_ir_pipeline_to_xee`.

- [ ] [ZA8RL] [S6C] **The S6A fixtures pass: web ↔ CLI divergence is zero for every migrated language.**

---

## S7 — Drop xot serialise/reparse (kills the "v1 stepping stone" tax)

**Goal.** The IR pipeline constructs xee `Documents` directly from xot, with no intermediate XML serialisation + reparse step.

**Why now.** The IR pipeline renders the IR to a fresh `xot::Xot`, calls `xot.to_string(...)` to serialise it to XML text, then feeds that string to `documents.add_string(...)` so xee re-parses it into a queryable `Documents`. Acknowledged "v1 stepping stone" in code at `parser/mod.rs:641`. Pays a serialise + parse cost on every parse and discards the typed in-memory tree just to reconstruct it.

**Depends on.** Nothing structural; may need an upstream change to xee.
**Unblocks.** Per-parse latency (no serialise + parse), simpler memory profile.
**Independent of.** Other slices.

**Size.** S in code; potentially L if xee needs an upstream change.
**Reversibility.** High. Self-contained in `parser/mod.rs`.

**Invariants when closed:**
- The `xot.to_string(...) → documents.add_string(...)` block at `parser/mod.rs:776` (and the matching ones in data/SQL branches) does not exist.
- xee `Documents` can ingest an existing `xot::Xot` + node handle directly.

### Tasks

- [ ] [EPJZN] [S7A] **A clear yes/no answer exists on whether xee `Documents` can ingest an existing `xot::Xot` + node handle today.**
  - Documented at the top of S7B's PR. May require xee upstream change.

- [ ] [55W1IP] [S7B] **Direct IR → xee Documents construction is implemented.**
  - Replaces the `xot::Xot::new()` → render → `to_string()` → `documents.add_string()` chain.

- [ ] [ZX824I] [S7C] **The `xot.to_string(...) → documents.add_string(...)` block at `parser/mod.rs:776` does not exist.**
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
- `Tree` variant set is `Tree::Xml(XmlMarkup) | Tree::Atom(XpathValue) | Tree::Ir{..} | Tree::DataIr{..} | Tree::Sql{..}`.
- No renderer pattern-matches across markup-and-atom in one expression.

### Tasks

- [ ] [VO169] [S8A] **`XmlMarkup` and `XpathValue` types exist; `XmlNode` is `XmlMarkup` only.**

- [ ] [2KKHE] [S8B] **`Tree` is `Tree::Xml(XmlMarkup) | Tree::Atom(XpathValue) | Tree::Ir{..} | Tree::DataIr{..} | Tree::Sql{..}`.**

- [ ] [X7WKK] [S8C] **No renderer branches on `XmlNode::{Map, Array, Number, Boolean, Null}`.**
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

- [ ] [X2N5] [S9A] **No test calls `parse_string_to_xot` / `parse_file_to_xee` / their variants.**
  - Files: `tests/ir_csharp_parity.rs`, `tests/ir_python_parity.rs`, `tests/ir_python_blueprint.rs`, `tests/ir_*_missing_kinds.rs`, `tests/coverage_report.rs`.

- [ ] [QWGMKD] [S9B] **`lib.rs` does not re-export the legacy parse functions.**
  - Targets at `lib.rs:87–99`. Depends on S9A.

- [ ] [TR47] [S9C] **`parse_*_with_options` functions do not exist in `parser/mod.rs`; only `parse()` remains.**
  - Depends on S9B.

---

## S10 — Per-language directory consolidation

**Goal.** All language-specific code for one language lives in one directory: `languages/<lang>/` carries the kinds, vocabulary, lowering, and canonical-source emitter. The shared IR machinery is small and clearly demarcated under `ir/`.

**Why now.** Today, language-specific code is split across two top-level directories — `languages/<lang>/` for kinds + vocabulary + post-transform, and `ir/<lang>.rs` (1.5–3K LOC each) + `ir/source/<lang>.rs` for the lowering and canonical emitter. The split reflects the IR migration history (the new pipeline grew sideways under `ir/`), not current ownership. The principle "languages decide what transformations they use, nothing forced top-down" survives if and only if the per-language code physically clusters.

The shared IR machinery — `ir/types.rs` (the unified `Ir` enum), `ir/to_xot.rs`, `ir/to_json.rs`, `ir/to_data.rs`, `ir/coverage.rs`, `ir/lower_helpers.rs`, `ir/source/common.rs` (the `Syntax` + `write_ir` engine) — is language-agnostic by design and stays in `ir/`. The cross-language unification at the IR layer is the architectural commitment of the design and isn't undone by this reorg. What changes is *where the per-language code physically lives*.

**Depends on.** S2-Z4 (registry pointers in place — the moves update one row each), and ideally S3B-Z1c (so the moved directories don't carry `post_transform.rs` files about to be deleted).
**Unblocks.** A reader can answer "show me all csharp-specific code" with one `ls`.
**Independent of.** S4, S5, S6, S7, S8, S9 — they don't read these paths.

**Size.** L. ~14 KLOC of file moves across 8 languages × 2 file types, plus directory regrouping for data-IR (9 files) and sql-IR (5 files).
**Reversibility.** High. Pure file moves with import updates; one-commit revert.

**Invariants when closed:**
- `ir/<lang>.rs` does not exist for any programming language; `languages/<lang>/lower.rs` exists instead.
- `ir/source/<lang>.rs` does not exist for any programming language; `languages/<lang>/render_source.rs` exists instead.
- `ir/data/` is a directory containing all `DataIr` types, lowering, and renderers.
- `ir/sql/` is a directory containing all `SqlIr` types and renderers (T-SQL lowering is at `languages/tsql/lower.rs`, consistent with S10A).
- Per-language `input.rs` is `kinds.rs`; per-language `output.rs` is `vocabulary.rs` (or fold into `mod.rs`).
- `ir/source/` (post-rename: `ir/render/`) holds only `common.rs` + `mod.rs`.

### Tasks

- [ ] [LZ8K] [S10A] **No `ir/<lang>.rs` lowering file exists for any programming language; `languages/<lang>/lower.rs` exists in its place.**
  - Each move: `git mv ir/<lang>.rs languages/<lang>/lower.rs`; add `pub mod lower;` to `languages/<lang>/mod.rs`; drop `pub mod <lang>;` from `ir/mod.rs`; update the `ir_family: Programming(<lang>::lower::lower_<lang>_root)` pointer in the `LANGUAGES` registry.
  - Per-language sub-tasks:
    - [ ] [S10A-Z1] csharp — `ir/csharp.rs` (2886 LOC) → `languages/csharp/lower.rs`.
    - [ ] [S10A-Z2] python — `ir/python.rs` (2238 LOC) → `languages/python/lower.rs`.
    - [ ] [S10A-Z3] java — `ir/java.rs` (2053 LOC) → `languages/java/lower.rs`.
    - [ ] [S10A-Z4] typescript — `ir/typescript.rs` (2079 LOC) → `languages/typescript/lower.rs` (covers ts/tsx/js/jsx).
    - [ ] [S10A-Z5] rust_lang — `ir/rust_lang.rs` (1967 LOC) → `languages/rust_lang/lower.rs`.
    - [ ] [S10A-Z6] go — `ir/go_lang.rs` (1507 LOC) → `languages/go/lower.rs`.
    - [ ] [S10A-Z7] ruby — `ir/ruby.rs` (688 LOC) → `languages/ruby/lower.rs`.
    - [ ] [S10A-Z8] php — `ir/php.rs` (1563 LOC) → `languages/php/lower.rs`.

- [ ] [M3VR] [S10B] **No `ir/source/<lang>.rs` per-language emitter exists; `languages/<lang>/render_source.rs` exists in its place. `LanguageOps` carries a `render_canonical: Option<RenderCanonicalFn>` field; `ir/source/mod.rs::render`'s match dispatch is gone.**
  - Each per-language emitter is small (~26–31 LOC: `Syntax` struct + `render` fn calling `super::common::write_ir`). Shared `write_ir` engine in `ir/source/common.rs` stays put.
  - Per-language sub-tasks: [S10B-Z1..Z8] csharp / java / python / typescript / rust_lang / go / ruby / php.
  - SQL is handled by S10D.

- [ ] [N7TH] [S10C] **`ir/data/` is a directory; the nine flat `data*.rs` / `*_data.rs` files at `ir/` root do not exist.**
  - Today's flat layout: `data.rs` (226), `data_to_xot.rs` (501), `data_to_json.rs` (222), `to_data.rs` (465 — `Ir → DataIr` projection, stays at IR root), `json_data.rs` (187), `yaml_data.rs` (265), `toml_data.rs` (367), `ini_data.rs` (152), `markdown_data.rs` (324).
  - Target: `ir/data/{types.rs, to_xot.rs, to_json.rs, lower_json.rs, lower_yaml.rs, lower_toml.rs, lower_ini.rs, lower_markdown.rs}`.

- [ ] [P2QX] [S10D] **`ir/sql/` is a directory; the five flat `sql*.rs` files at `ir/` root and `ir/source/sql.rs` do not exist. T-SQL lowering lives at `languages/tsql/lower.rs`.**
  - Today's flat layout: `sql.rs` (741), `sql_lower.rs` (2243), `sql_to_xot.rs` (908), `sql_to_json.rs` (684), `ir/source/sql.rs` (147).
  - Target: `ir/sql/{types.rs, to_xot.rs, to_json.rs, render_source.rs}` + `languages/tsql/lower.rs`.

- [ ] [Q4WB] [S10E] **No `languages/<lang>/input.rs` file exists; `kinds.rs` exists in its place.**
  - Files contain only the generated `CsKind` / `PyKind` / `JavaKind` / etc. enum (CST-kind catalogue), used by `tests/kind_catalogue.rs` and the `Ir::Unknown` audit. The "input" name dates from the retired imperative pipeline.
  - Update `task gen:kinds` codegen to write `kinds.rs`. Update test imports.

- [ ] [R5DM] [S10F] **No `languages/<lang>/output.rs` file exists; `vocabulary.rs` exists in its place (or contents are folded into `mod.rs`).**
  - Cross-reference C2: if C2 chooses (a) "drive shape contracts off `Ir` variants alone and delete `TractorNode`", S10F is moot — delete the files instead.

- [ ] [W9KS] [S10G] **`ir/source/` does not exist; `ir/render/` exists in its place (or `ir/source/mod.rs::render` is gone entirely if S10B's registry field replaces it).**

- [ ] [V3QM] [S10H] **Either `transform/` has a clear sole-purpose role (with a name that matches), or it is gone.**
  - After S3A + S3B + S3D + S3E + C3, the survivors are: `walk_transform`, `apply_field_wrappings`, possibly `singletons.rs`, possibly `builder.rs`. These serve only the legacy `XeeBuilder` path (data languages JSON/YAML's syntax branch + Raw mode + WASM until S6).
  - Decide: rename `transform/` to `legacy_xot/` (or similar) to mark it non-IR, or delete what's actually dead.
  - Depends on S3, S6, C3.

---

## Side cleanups (smaller, can interleave)

Each is one closeable invariant. No full slice header — too small to warrant one.

- [ ] [HS78] [C1] **No `LanguageOps` entry on the IR path declares `field_wrappings` (read only by the legacy `XeeBuilder`).**
  - Depends on S2 routing IR languages away from `XeeBuilder`. Languages still on the legacy path retain their wrappings.

- [ ] [A2WV] [C2] **One source of truth for emitted element names: either `Ir` variants alone (per-language `TractorNode` enums deleted), or `TractorNode` generated from `Ir`.**
  - Decision point. The choice cascades into S10F and the shape-contract walks in S3E.

- [ ] [76SDP] [C3] **`transform::builder::XotBuilder` does not exist.**
  - Depends on S6 (WASM moved off it) and S2 (legacy languages routed through `XeeBuilder` only).

- [ ] [C4] **`output::xml_node_to_json` is reachable only by genuine XPath partial matches; the legacy IR/DataIr fallthrough is gone.**
  - Depends on S5 + S8.

- [ ] [C5] **Exactly one language registry exists. `tractor/src/languages/info.rs::LANGUAGES` either is gone or is a typed view onto `tractor/src/languages/mod.rs::LANGUAGES`.**
  - Surfaced during S2-Z3: two `LANGUAGES` arrays both claim SSoT in their docstrings. `info.rs` carries `aliases`, `has_transforms`, `grammar_file` (web/wasm path); `mod.rs` carries `extensions`, `grammar`, `ir_family`, `transform`, `post_transform`, etc.
  - Two paths: (a) merge `info.rs`'s extra fields into `LanguageOps` and delete `info.rs`; or (b) rewrite `info.rs::LANGUAGES` as a derivation of `mod.rs::LANGUAGES`, leaving `Language` (enum) and `LanguageInfo` (struct) as a thin facade if external consumers need them.
  - Today `info.rs::LANGUAGES` still lists c/cpp/html/css/bash/scala/lua/haskell/ocaml/r/julia/xml as `has_transforms: false`, while S2-Z3 dropped them from the parser side. After consolidation, `language_info::get_language_info("cpp")` should be `None` (or `xml`-only for the passthrough case) so the two registries never disagree.
