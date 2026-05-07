# TS → IR → {xot,xml,json,yaml,source} — architectural cleanup

Slices ordered to minimise rework. Each task line is one imperative action with a clear
outcome; nested bullets are context, not work to do.

### Working with this list

- `[ ]` open · `[/]` Claude: start work · `[x]` done (Claude appends a one-line "Done:" note).
- Append `??` to a line you want clarified. On the next turn Claude rewrites that single line — no merging, splitting, or reorganising — and removes the `??`.
- Append `++` to a line you want decomposed. On the next turn Claude adds nested sub-task checkboxes underneath, leaves the parent line as the tracker, and removes the `++`.
- To merge or reorder tasks, ask explicitly; Claude will not do it on its own.
- Edit the prose under a task freely; Claude treats nested bullets as context, not as actions.

## Findings

### Pipeline as it actually runs

Library entry point: `parser::parse(ParseInput, ParseOptions)` (`parser/mod.rs:1106`).
Three downstream branches, gated by `use_ir_pipeline(lang, mode)` (`parser/mod.rs:380`):

| Path | Languages | Where |
|---|---|---|
| **IR (programming)** — `Ir` | csharp, python, java, ts/js/tsx/jsx, rust, go, ruby, php | `parse_with_ir_pipeline_to_xee` |
| **IR (data)** — `DataIr` | json, yaml, toml, ini, env, markdown (Structure mode) | same fn, data branch |
| **IR (sql)** — `SqlIr` | tsql | same fn, sql branch |
| **Legacy imperative** — `XeeBuilder::build_with_options` + `walk_transform` | Raw mode for everything; c, cpp, html, css, bash, scala, lua, haskell, ocaml, r, julia; json/yaml in Data mode | `parser/mod.rs:933` |
| **WASM** — `XotBuilder` + `walk_transform` | All web-app parses | `wasm/mod.rs` |

The IR path renders to xot, **serializes the xot to a string, and re-parses it into xee `Documents`** (acknowledged "v1 stepping stone" at `parser/mod.rs:641`). After that it runs the legacy `post_transform` for the language, which for migrated languages still means `chain_inversion::invert_chains_in_tree` (1826 LOC of imperative xot mutation) plus per-language list-tagging.

`tractor render` and `tractor set/update`'s value-rewrite use a *different*, parallel reverse pipeline: `render::parse_xml`/`parse_json` → `XmlNode` → `render::render(node, lang, TreeMode::Data, opts)` (`render/mod.rs`). It speaks XmlNode, not IR, and supports csharp/json/yaml only.

### Weakest points

- **W1 — Three parallel IR shapes, three parallel everything.** `Ir` (1703 LOC), `DataIr` (226), `SqlIr` (741). Each has its own `lower_*`, `to_xot`, `to_json`, `source/*`, dispatch arm, `Tree::*` variant, `Match::to_json` branch. Three concurrent JSON projection strategies coexist (legacy `xml_node_to_json`, heuristic `ir_to_json`, principled `data_to_json`). → addressed by **S5** (single projection) plus **S2** (single dispatch).
- **W2 — Module documentation lies about production status.** `ir/mod.rs` § Status said "experimental, parity-target sketch on a Python fragment"; in reality IR is the production path for ~80% of languages and `walk_transform` is unreachable for them. → addressed by **S1**.
- **W3 — The reverse path is on the wrong substrate.** Two reverse renderers exist: wired `render/{csharp,json,yaml}.rs` (XmlNode, TreeMode::Data, 3 languages) and unwired `ir/source/*.rs` (IR, anchored or canonical, 9 languages). `tractor render` / `set` / `update` are stuck at 3 languages because the wired path can't see the IR. → addressed by **S4**.
- **W4 — Cross-cutting transforms are still imperative xot mutation.** `ir/mod.rs` claims chain inversion is pure `Ir → Ir`; actually `transform::chain_inversion` (1826 LOC) is invoked from each language's `post_transform` and runs on the rendered xot output of the IR pipeline. → addressed by **S3**.
- **W5 — Web (WASM) and CLI produce different output for migrated languages.** `wasm/mod.rs` uses `XotBuilder` + `walk_transform` and never touches `crate::ir`. The web playground (`web/src/tractor.ts`) calls into it. Where the IR migration changed the shape, the two diverge silently. → addressed by **S6**.
- **W6 — Six places know the language list, none authoritative.** `SUPPORTED_LANGUAGES`, `detect_language`, `get_tree_sitter_language`, `LANGUAGES` (LanguageOps.ids), `use_ir_pipeline`, and the per-IR-family match arms in `parse_with_ir_pipeline*` (×2 for the xot and xee variants). The aliasing of `csharp`/`cs`, `python`/`py`, etc. is independently encoded in five of those. → addressed by **S2**.
- **W7 — `XmlNode` mixes XML markup with XPath atomic data.** `xpath/match_result.rs:20` defines `XmlNode` with `Element/Text/Comment/PI` *and* `Map/Array/Number/Boolean/Null`. Two unrelated abstractions in one enum; renderers branch on it. → addressed by **S8**.
- **W8 — Cardinality plumbed twice.** IR's typed slots (`Box<Ir>` / `Vec<Ir>`) carry cardinality natively, but `parse_with_ir_pipeline*` still runs the legacy `list="X"` attribute pass (`parser/mod.rs:602–609`). For JSON the typed renderer reads slots; for XML downstream readers consult `list=`. Two encodings of the same fact. → addressed by **S3** (S3D specifically).
- **W9 — Field-wrappings and per-language `TractorNode` enums are dead-data for migrated languages.** Every `LanguageOps` declares `field_wrappings`, only consumed by the legacy XeeBuilder; for IR languages they're effectively no-ops. Each language also declares a strum `TractorNode` enum that parallels what `Ir` already encodes. → addressed by **C1** + **C2**.
- **W10 — Old API still exposed alongside the new one.** `lib.rs:87–99` re-exports `parse_string_to_xot`, `parse_file_to_xee`, etc. alongside `parse(ParseInput, ParseOptions)`. The "one principled parse entry point" comment is intent, not fact. → addressed by **S9**.

### Out-of-scope guardrails

- The IR design itself (typed slots, byte-range anchoring, `Inline`/`Unknown` escape hatches, coverage audit) is sound. Don't refactor it as part of this work.
- Three IR families staying separate is fine *as types*. The cost only goes away if dispatch + projection + rendering stop being copy-pasted three ways — that's S2 + S5, not a re-merge of the types.

---

## S1 — Truth-up the IR module docs

**Problem (W2):** `ir/mod.rs` was labelled "experimental, parity-target sketch on a Python fragment" while in reality it had become the production path for ~80% of languages. New readers misjudged the system and assumed the legacy `crate::transform` was the production path.

Done.

## S2 — One language registry (kills W6)

**Problem — single source of truth for language identity is missing.** The answer to "what languages does tractor support, and what does the alias `cs` mean?" is split across six independent tables that each encode part of it:

1. `tractor/src/parser/mod.rs:18` — `SUPPORTED_LANGUAGES` (name → extensions).
2. `tractor/src/parser/mod.rs:118` — `detect_language` (extension → canonical name).
3. `tractor/src/parser/mod.rs:155` — `get_tree_sitter_language` (name + alias → grammar).
4. `tractor/src/languages/mod.rs:141` — `LANGUAGES` registry (`LanguageOps.ids`: full alias list per language).
5. `tractor/src/parser/mod.rs:380` — `use_ir_pipeline` (name + alias → IR-or-not).
6. `tractor/src/parser/mod.rs:520, :574, :660, :741` — four parallel match arms inside `parse_with_ir_pipeline` and `parse_with_ir_pipeline_to_xee` (alias → IR-family lower function).

Aliasing (`csharp`/`cs`, `python`/`py`, `typescript`/`ts`/`tsx`/`jsx`, …) is independently re-listed in five of these.

**Concrete failure mode:** when one table is updated and another isn't, the file still parses but routes to the wrong pipeline *silently*. Add `transact-sql` to `LanguageOps.ids` (4) and `detect_language` (2) won't recognise the extension; add a new TS-family alias to `use_ir_pipeline` (5) and forget the dispatch arm in (6) and the language hits the runtime error `"IR pipeline not yet wired for language …"`. Dispatch and detection drift apart silently because nothing forces them to agree.

**Principle:** single source of truth / knowledge ownership. DRY is the surface name for the same principle — the duplication is the symptom, the silent inconsistency on partial updates is the failure mode. (Not primitive obsession: the problem isn't using `&str` for language IDs, it's that no single owner answers "given this string, what is this language?")
- [/] Single declaration of all languages and their string ids we have, merge all language metadata and configuration into that.
  - Scope note: this subsumes the existing S2A–S2E sub-tasks below. As each child here completes, the matching S2A–E item should be marked `[x]` (or removed). Don't trigger S2A–E independently while this consolidation is in progress.
  - Currently scattered across (per the Problem above): `SUPPORTED_LANGUAGES`, `detect_language`, `get_tree_sitter_language` (parser/mod.rs); `use_ir_pipeline` + four dispatch match arms (parser/mod.rs); `LANGUAGES` (languages/mod.rs).
  - Target end state: `LANGUAGES` holds one row per supported language with all metadata (canonical name + aliases, extensions, tree-sitter grammar, IR family + lower fn, transforms, post-transform, syntax category, field wrappings, node spec). Every other table either reads from it or is deleted.
  - [x] [S2-Z1] Extend `LanguageOps` (`tractor/src/languages/mod.rs:119`) with three new fields and populate every existing row.
    - New fields: `extensions: &'static [&'static str]`, `grammar: fn() -> tree_sitter::Language`, `ir_family: IrFamily` where `IrFamily = None | Programming(LowerFn) | Data(LowerFn) | Sql` (new enum to declare alongside).
    - Existing rows to populate: typescript, csharp, python, go, rust, java, ruby, php, tsql, json, yaml, toml, ini, env, markdown — 15 entries.
    - Per-language grammar shim functions next to each row (`fn ts_csharp() -> tree_sitter::Language { tree_sitter_c_sharp::LANGUAGE.into() }`).
    - Outcome: every existing row carries extensions + grammar + ir_family. Secondary tables still exist as duplicates and are still consulted; build passes.
    - Done: added `GrammarFn`, `LowerToIr`/`LowerToDataIr`/`LowerToSqlIr`, and `IrFamily { None | Programming(_) | Data(_) | Sql(_) }` to `languages/mod.rs`; added 17 grammar shim fns; added 3 `#[cfg(feature = "native")]` fields to `LanguageOps`; populated the registry. **Note:** the TS/TSX/JS row split into 3 rows because grammars differ (TS, TSX, JS) — `["typescript","ts"]`, `["tsx"]`, `["javascript","js","jsx"]`. Each carries the same transforms/post-transform/vocabulary; only `grammar`, `extensions`, `ids` differ. Both `cargo check --features native` and `cargo check --no-default-features --features wasm --target wasm32-unknown-unknown` build cleanly.
  - **(dropped — user direction 2026-05-08)** ~~S2-Z2: Add rows for c/cpp/html/css/bash/scala/lua/haskell/ocaml/r/julia/xml.~~ Those languages aren't really supported (no blueprints, no transforms); they were just listed in `parser/mod.rs`. Don't promote them to `LANGUAGES`. *(Term used above — "secondary tables" — meant `SUPPORTED_LANGUAGES`, `detect_language`, `get_tree_sitter_language` in `parser/mod.rs`, i.e. the lookups that today live outside `LANGUAGES`.)*
  - [x] [S2-Z3] Make `LANGUAGES` the canonical list: rewrite the parser-side tables as derivations and drop the now-orphan languages from them.
    - `SUPPORTED_LANGUAGES` (`parser/mod.rs:18`) becomes a `Lazy<Vec<(&str, &[&str])>>` over `LANGUAGES.iter().map(|l| (l.ids[0], l.extensions))`.
    - `detect_language(path)` (`parser/mod.rs:118`) iterates `LANGUAGES`, returns the canonical name (`ids[0]`) of the entry whose `extensions` matches the path.
    - `get_tree_sitter_language(name)` (`parser/mod.rs:155`) returns `(get_language(name)?.grammar)()`.
    - The c/cpp/html/css/bash/scala/lua/haskell/ocaml/r/julia/xml entries that today exist only in those parser-side tables drop out — their files will fail with a clear "unsupported language" error rather than silently parsing into a transformless blob.
    - Outcome: `LANGUAGES` is the single source of truth for what languages exist; everything else reads from it.
    - Done: `SUPPORTED_LANGUAGES`, `detect_language`, `get_tree_sitter_language`, and `get_language_abi_versions` in `parser/mod.rs` all derive from `crate::languages::LANGUAGES` now (XML kept as a one-line passthrough special case in `detect_language` since it has no tree-sitter grammar). `cargo check` (native + wasm) clean; `cargo test --lib` 373/373 pass. **Remaining collision:** `tractor/src/languages/info.rs` still owns a separate `LANGUAGES` array of `LanguageInfo` plus a `Language` enum, with overlapping responsibilities (extensions, aliases, has_transforms, web grammar_file). Listed below as **C5** to consolidate next.
  - [/] [S2-Z4] Replace `parser::use_ir_pipeline` (`parser/mod.rs:380`) and the four dispatch match arms in `parse_with_ir_pipeline` and `parse_with_ir_pipeline_to_xee` (`:520, :574, :660, :741`) with registry lookups.
    - `use_ir_pipeline` deleted; callers consult `get_language(lang)?.ir_family`.
    - Each `match lang { "csharp" => lower_csharp_root, ... }` collapses to `match get_language(lang)?.ir_family { IrFamily::Programming(f) => f(node, source), ... }`.
    - Outcome: dispatch reads from the registry. Adding/removing a language no longer edits `parser/mod.rs`.
    - Status: half done. `use_ir_pipeline` is gone (handled by S2C); the four match arms in `parse_with_ir_pipeline*` still pattern-match on language id and need to collapse to a single `match l.ir_family` lookup.
  - [x] [S2-Z5] Run `cargo build --release` and `cargo test`; resolve any fallout.
    - Outcome: green build, all tests pass; the language-registry consolidation lands as a single behaviour-preserving refactor.
    - Done: `cargo check --features native` clean (only pre-existing warnings); `cargo check --no-default-features --features wasm --target wasm32-unknown-unknown` clean; `cargo test --lib` 373/373 pass after S2-Z1, S2C, S2-Z3.

- [x] [DGV7Q] [S2A] Declare a public `IrFamily` enum in `tractor/src/languages/mod.rs` ready for `LanguageOps` to consume in S2B.
  - Done: subsumed by S2-Z1. `IrFamily { None | Programming(_) | Data(_) | Sql(_) }` + the `LowerTo*` type aliases live in `languages/mod.rs`.
- [x] [RY3I3] [S2B] Add an `ir_family: IrFamily` field to `LanguageOps` and populate every entry in `LANGUAGES`.
  - Done: subsumed by S2-Z1. Field added and populated for every existing row (TS family split into 3 grammar-specific rows).
- [x] [UKNUK] [S2C] Delete `parser::use_ir_pipeline` (`parser/mod.rs:380`) and replace its callsites with `LanguageOps::ir_family`.
  - Tracked by S2-Z4 above.
  - Done: function deleted from `parser/mod.rs`; both callers (lines 441 and 918) now do `crate::languages::get_language(lang).map(|l| l.uses_ir(resolved)).unwrap_or(false)`. The mode-aware decision lives on a new `LanguageOps::uses_ir(TreeMode)` method that reads `ir_family` from the registry.
- [/] [51D4W9] [S2D] Collapse the duplicated dispatch arms in `parse_with_ir_pipeline` and `parse_with_ir_pipeline_to_xee` into one match driven by `ir_family`.
  - Tracked by S2-Z4 above.
- [/] [U7O68] [S2E] Audit and remove every other place that knows alias mappings (csharp/cs, ts/tsx/jsx, …); route them through `get_language(lang)`.
  - Tracked by S2-Z3 above (rewriting the parser-side tables as derivations of `LANGUAGES` removes the alternative alias-knowing sites).

## S3 — Eliminate the chain-inversion post-step; lower chains directly into `Ir::Access` (kills W4, partially W8)

**Problem:** `transform::chain_inversion::invert_chains_in_tree` (1826 LOC) is invoked from each language's `post_transform` and mutates the *rendered xot tree* after the IR has been produced. This is not just "imperative leftover" — it's redundant work: tree-sitter parses chains right-deep (operator precedence), the lowering function for the language already has all the structural information needed to produce a left-deep `Ir::Access`, and `Ir::Access { receiver, segments: Vec<AccessSegment> }` already exists for exactly this. The xot-walk re-derives the same inverted shape after the fact, against a less-typed substrate. Cardinality is also encoded twice — once in IR slots (`Box<Ir>` / `Vec<Ir>`) and again as `list="X"` attributes the same post-walk attaches.

**Target state (user direction 2026-05-08):** there is **no** chain-inversion transform step at all. Every `lower_<lang>_root` constructs `Ir::Access` directly when it encounters chained member/index/call expressions. The `transform::chain_inversion` module is deleted, not relocated; nothing replaces it.

- [ ] [WGWS0] [S3A] Make every language's CST→IR lowering construct `Ir::Access` natively for chained access; delete `transform::chain_inversion`.
  - Audit each `lower_<lang>_root` (`ir/{csharp,python,java,typescript,rust_lang,go_lang,ruby,php}.rs`) for any chain shape that currently relies on the post-walk to invert. Rewrite the lowering to produce a left-deep `Ir::Access { receiver, segments }` directly.
  - Languages that already do this (csharp, parts of typescript): verify completeness against the current corpus; their `post_transform` chain-inversion call becomes a no-op and is removed.
  - Languages that currently lean on the post-walk (per existing call sites): `languages/{csharp,go,rust_lang,ruby,typescript,php}/post_transform.rs` each invokes `chain_inversion::invert_chains_in_tree`. Replace each call site with the native lowering, then delete the call.
  - Once no caller remains, delete `tractor/src/transform/chain_inversion.rs` (1826 LOC) and remove the module from `transform/mod.rs`.
  - Outcome: chain inversion exists nowhere as a step. Right-deep CST → left-deep IR is the lowering contract for every language.
- [ ] [XZ64] [S3B] Eliminate every remaining per-language `post_transform` pass; encode the same shapes natively in the IR variants and the lowering, the same way S3A does for `Ir::Access`.
  - The remaining xot post-passes today: flat conditional shape (`<if>...<else_if/>...<else/>`), `<member>`/`<call>` slot wrapping (PHP), singleton closure body re-tagging (Go), TSQL binary-operand wrapping, identifier-role marking, etc. Each is currently a walk over rendered xot.
  - For each pass, choose the natural home in the IR rather than carry it as a separate step. Examples: flat conditionals — `Ir::If { condition, body, else_branch: Option<Box<Ir::ElseIf | Ir::Else>> }` already encodes the chain; either flatten it during `lower_<lang>_root` or during `to_xot`, never as a post-walk on xot. Identifier-role marking — encode declaration vs reference on the `Ir::Name` variant directly so the lowering decides it once. Slot wrappers — produce the right `Ir::*` shape from the lowering, no rewrap.
  - When every pass has its natural home, `languages/{csharp,go,rust_lang,ruby,typescript,php,java,python,tsql}/post_transform.rs` end up with no actual work; delete the files and their `post_transform: Some(...)` entries in `LanguageOps`.
  - Outcome: there is no `post_transform` step. The IR you get from `lower_<lang>_root` is already the canonical shape; `to_xot` is mechanical.
  - [ ] [S3B-Z1] C# — fold `csharp_normalize_conditional_access`, `unify_file_scoped_namespace`, `attach_where_clause_constraints`, `append_constraint_to_generic` from `languages/csharp/post_transform.rs` into `lower_csharp_root` / `lower_csharp_node` (and any new `Ir::*` variants required, e.g. for `where`-clause attachment). Then `LanguageOps` for csharp gets `post_transform: None` and the file is deleted.
  - [ ] [S3B-Z2] Rust — fold `rust_normalize_field_expression`, `rust_normalize_lifetime_names`, `rust_restructure_use` from `languages/rust_lang/post_transform.rs` into the lowering. `post_transform: None`; delete the file.
  - [ ] [S3B-Z3] TypeScript / JS / TSX — fold `typescript_unwrap_callee` and `typescript_restructure_import` from `languages/typescript/post_transform.rs` into `lower_typescript_root` / `lower_typescript_node`. All three TS-family rows in `LANGUAGES` go to `post_transform: None`; delete the file.
  - [ ] [S3B-Z4] Python — fold `python_tag_from_imports_uniform`, `python_restructure_imports`, `python_alias_pairs`, `python_flatten_dotted_name` from `languages/python/post_transform.rs` into the lowering. `post_transform: None`; delete the file.
  - [ ] [S3B-Z5] Java — fold `java_unwrap_type_in_path` from `languages/java/post_transform.rs` into the lowering. `post_transform: None`; delete the file.
  - [ ] [S3B-Z6] Go — fold `go_retag_singleton_closure_body` from `languages/go/post_transform.rs` into the lowering. `post_transform: None`; delete the file.
  - [ ] [S3B-Z7] T-SQL — fold `tsql_wrap_binary_operands` and `tsql_tag_select_columns` from `languages/tsql/post_transform.rs` into `lower_sql_root` / `SqlIr` variants. `post_transform: None`; delete the file.
  - [ ] [S3B-Z8] PHP — fold `php_wrap_member_call_slots` and `php_restructure_use` from `languages/php/post_transform.rs` into `lower_php_root` / `lower_php_node`. `post_transform: None`; delete the file.
  - [ ] [S3B-Z9] Ruby — fold `ruby_tag_case_when_lists`, `ruby_retag_singleton_block_body`, `ruby_collapse_lambda_body`, `ruby_extract_pair_keys` from `languages/ruby/post_transform.rs` into the lowering. `post_transform: None`; delete the file.
  - [ ] [S3B-Z10] Cross-cutting `collapse_conditionals` (`languages/mod.rs:443`): pick a single home — either flatten the `<if><else_if/><else/>` chain inside `lower_<lang>_root` for each language that exercises it, or do it once in `to_xot` driven by the typed `Ir::If { else_branch: Option<Box<Ir>> }` shape. Whichever home wins, the standalone xot-walk in `languages/mod.rs` retires.
  - [ ] [S3B-Z11] Once every `post_transform.rs` is gone (Z1–Z9 plus S3A's chain-inversion deletions), drop the `post_transform: Option<PostTransformFn>` field from `LanguageOps` entirely; remove `get_post_transform` and the `post_transform` invocation block in `parse_with_ir_pipeline*` (the latter is also tracked by S3C).
- [ ] [3C72S] [S3C] Delete the post-transform invocation on rendered xot in `parse_with_ir_pipeline_to_xee` (`parser/mod.rs:602–609`) and the matching block in `parse_with_ir_pipeline`.
  - Depends on S3A + S3B.
  - Outcome: the IR pipeline no longer mixes paradigms.
- [ ] [3IIU] [S3D] Drop the `list="X"` attribute pass for IR languages and have `to_xot` emit cardinality from typed slots.
  - `Box<Ir>` → singleton; `Vec<Ir>` → list.
  - Outcome: cardinality is encoded once (in IR slots), not twice (slots + attribute).
- [ ] [DZRA9N] [S3E] Move `transform::shape_contracts` assertions from xot-walks to compile-time/`cargo check` constraints on `Ir`.
  - Most rules become unrepresentable at the type level (per `ir/types.rs:31`); keep runtime-only ones (e.g. `op-marker-matches-text`) as thin walks on `Ir`.
  - Outcome: shape bugs become type errors where possible.

## S4 — Wire IR-based reverse rendering (kills W3)

**Problem:** Forward parsing speaks IR; reverse rendering (back to source) speaks `XmlNode` + `TreeMode::Data` and supports only csharp/json/yaml. The IR-aware reverse renderer (`ir/source/*`) is implemented for 9 languages but not wired in. As a result, `tractor render`, `tractor set`, and `tractor update` are stuck at 3 of 28 languages — gated by what the legacy XmlNode renderer happens to support.

- [ ] [J60X] [S4A] Make `tractor render` accept source + lang and emit re-rendered source via `ir::source::render(ir, lang, anchor)`.
  - Edit `cli/render.rs` to read source (stdin/`--string`/file), call `parse()`, then `ir::source::render`.
  - Outcome: `tractor render` works for every language with an IR.
- [ ] [21DT] [S4B] Switch the value-rewrite path in `mutation/xpath_upsert.rs:252,390` from `render_with_spans(xml_node, lang, TreeMode::Data, …)` to anchored IR re-render with span tracking.
  - Outcome: `tractor set/update` lifts from 3 supported languages to all IR-supported languages.
- [ ] [KOLKFS] [S4C] Delete `render::parse_xml` and `render::parse_json` from `render/mod.rs` (lines 87, 172) once nothing reads XmlNode-from-text.
  - Depends on S4A + S4B.
  - Outcome: no more re-parsing tractor's own output to feed the reverse path.
- [ ] [3WO0Y] [S4D] Delete `render/{csharp,json,yaml}.rs` (~1800 LOC) once their callers are gone.
  - Optional: keep behind an explicit fallback flag for the languages still without IR, with a sunset comment.
  - Outcome: one reverse renderer, on the same substrate as the forward one.

## S5 — Single JSON projection: Ir → DataIr → JSON (kills part of W1)

**Problem:** Three concurrent JSON projection strategies coexist: legacy `xml_node_to_json` (XmlNode-based fallback), heuristic `ir_to_json` (1087 LOC of `$inline`/`$skip`/`$type`/plural-collapse rules papering over IR↔JSON impedance), and the principled `data_to_json` reading a typed `DataIr`. Each new edge case lands as another heuristic in `ir_to_json`; the in-progress `to_data` projection covers only `Ir::Class` so far.

- [ ] [RYLH] [S5A] Extend `ir::to_data::lower_to_data_ir` to cover every `Ir` variant.
  - Today it covers `Ir::Class` + scalar leaves (`ir/to_data.rs:37`). Slice plan in `docs/design-projection-pipeline.md`.
  - Outcome: every `Ir` projects to a `DataIr` deterministically.
- [ ] [FJYNZN] [S5B] Move the heuristic blocks in `ir/to_json.rs` (1087 LOC; `$inline`, `$skip`, `$type:"expression"`, plural-of-self collapse, marker-vs-leaf) into uniform projection rules in `to_data`.
  - Outcome: JSON shape decisions are made once, in one place, on a typed shape.
- [ ] [AP9K0B] [S5C] Switch `Tree::to_json` (`xpath/match_result.rs:139`) to call `data_to_json(to_data(ir, source))` for the `Tree::Ir` arm.
  - Keep `data_to_json` direct for `Tree::DataIr` and `sql_to_json` for `Tree::Sql` for now.
  - Outcome: programming-language JSON output flows through the principled pipeline.
- [ ] [UCMC] [S5D] Delete `tractor/src/ir/to_json.rs` once no caller depends on it.
  - Outcome: ~1000 LOC of heuristics removed.
- [ ] [X8UX] [S5E] Project `SqlIr` through `DataIr` for JSON output (so all three IR families share one projection algorithm).
  - Outcome: one JSON projection algorithm, three entry points.

## S6 — WASM uses the unified parse (kills W5)

**Problem:** WASM (`wasm/mod.rs`) imports `XotBuilder` + `walk_transform` + `get_transform` and never touches `crate::ir`. The web playground (`web/src/tractor.ts`) calls into it. Since the IR migration changed the semantic XML shape for migrated languages, the web playground silently produces *different* output than the CLI for the same source — queries that work in one fail in the other.

- [ ] [6B4STY] [S6A] Capture before/after fixtures showing where the WASM playground diverges from the CLI for migrated languages.
  - WASM uses `XotBuilder` + `walk_transform`; CLI uses IR. Concrete diffs prove the gap.
  - Outcome: a small fixture set the rest of the slice can validate against.
- [ ] [RE5EF5] [S6B] Replace the `XotBuilder` call in `wasm::parse_to_xml` with the IR pipeline.
  - Either build a `tree_sitter::Tree`-shaped input from `SerializedNode` on the Rust side, or extract a WASM-friendly variant of `parse_with_ir_pipeline_to_xee`.
  - Outcome: the web playground runs the same parse function as the CLI.
- [ ] [ZA8RL] [S6C] Verify the web playground produces identical output to `tractor -x` on the same source for every migrated language.
  - Run the S6A fixtures through both paths.
  - Outcome: web ↔ CLI output divergence is zero.

## S7 — Drop xot serialize/reparse (kills the "v1 stepping stone" tax)

**Problem:** The IR pipeline renders the IR to a fresh `xot::Xot`, calls `xot.to_string(...)` to serialise it to XML text, then feeds that string to `documents.add_string(...)` so xee re-parses it into a queryable `Documents`. Acknowledged "v1 stepping stone" in code at `parser/mod.rs:641`, never replaced. Pays a serialise + parse cost on every parse, and means xot's typed in-memory tree is thrown away just to be reconstructed via XML text.

- [ ] [EPJZN] [S7A] Determine whether xee-xpath's `Documents` can ingest an existing `xot::Xot` + node handle.
  - `parser/mod.rs:641` calls this out as a v1 limitation.
  - Outcome: a clear yes/no on whether xee needs an upstream change.
- [ ] [55W1IP] [S7B] Implement direct IR → xee Documents construction (extend xee, or build into the Documents' xot in place).
  - Replaces the `xot::Xot::new()` → render → `to_string()` → `documents.add_string()` chain.
  - Outcome: the IR pipeline no longer round-trips through serialized XML.
- [ ] [ZX824I] [S7C] Delete the `xot.to_string(...) → documents.add_string(...)` block at `parser/mod.rs:776` (and the matching one in the data/SQL branches).
  - Depends on S7B.
  - Outcome: no more serialize/reparse step.

## S8 — Split `XmlNode` from XPath atomic data (kills W7)

**Problem:** `xpath::XmlNode` glues two unrelated abstractions into one enum: XML markup (`Element` / `Text` / `Comment` / `ProcessingInstruction`) and XPath atomic / structured values (`Map` / `Array` / `Number` / `Boolean` / `Null`). Every renderer that walks a match has to branch on whether the variant is markup or a value, and the type signature lies about what a function actually accepts.

- [ ] [VO169] [S8A] Define separate `XmlMarkup` (Element/Text/Comment/PI) and `XpathValue` (Map/Array/Number/Boolean/Null) types in `xpath/match_result.rs`.
  - Outcome: the markup and atomic-value abstractions stop sharing one enum.
- [ ] [2KKHE] [S8B] Refactor `Tree` to `Tree::Xml(XmlMarkup) | Tree::Atom(XpathValue) | Tree::Ir{..} | Tree::DataIr{..} | Tree::Sql{..}`.
  - Outcome: every tree variant carries one well-defined shape.
- [ ] [X7WKK] [S8C] Update every renderer that branches on `XmlNode::{Map, Array, Number, Boolean, Null}` to consume `Tree::Atom` instead.
  - Likely sites: `format/json.rs`, `format/xml.rs`, `output/*`.
  - Outcome: renderers no longer pattern-match across two abstractions in one enum.

## S9 — Retire the old parse API surface (kills W10)

**Problem:** `parser/mod.rs` now exposes a unified `parse(ParseInput, ParseOptions)` *and* the legacy multi-function API (`parse_string_to_xot`, `parse_file_to_xot`, `parse_string_to_xee`, `parse_file_to_xee`, `load_xml_string_to_documents`, `load_xml_file_to_documents`, plus the matching `*_with_options` variants). All of them are re-exported from `lib.rs`, and tests still call the legacy ones. The "one principled parse entry point" comment at `parser/mod.rs:1091` is intent, not fact.

- [ ] [X2N5] [S9A] Migrate every test from `parse_string_to_xot` / `parse_file_to_xee` to `parse(ParseInput, ParseOptions)`.
  - Files: `tests/ir_csharp_parity.rs`, `tests/ir_python_parity.rs`, `tests/ir_python_blueprint.rs`, `tests/ir_*_missing_kinds.rs`, `tests/coverage_report.rs`.
  - Outcome: no caller in the repo uses the legacy parse functions.
- [ ] [QWGMKD] [S9B] Remove the back-compat re-exports in `lib.rs:87–99`.
  - Targets: `parse_string_to_xot`, `parse_file_to_xot`, `parse_string_to_xee`, `parse_file_to_xee`, `load_xml_string_to_documents`, `load_xml_file_to_documents`.
  - Depends on S9A.
  - Outcome: only `parse()` is exposed publicly.
- [ ] [TR47] [S9C] Delete the `parse_*_with_options` functions from `parser/mod.rs`, keeping only the unified `parse()`.
  - Depends on S9B.
  - Outcome: one parse function in the library, end to end.

## Side cleanups (smaller, can interleave)

**Problem:** The IR migration leaves residue scattered across the codebase: data declared by every `LanguageOps` entry that only the legacy XeeBuilder reads (W9), per-language `TractorNode` strum enums that parallel what `Ir` already encodes (W9), a redundant tree builder (`XotBuilder`) that survives only as long as WASM and the legacy path need it, and an XML→JSON projection that shrinks but doesn't disappear. Each of these is a small, low-risk delete that becomes safe once a specific upstream slice (S2, S5, S6) lands.

- [ ] [HS78] [C1] Remove `field_wrappings` from `LanguageOps` entries that are fully on the IR path.
  - Field-wrappings are read only by `XeeBuilder::build_with_options`; once S2 routes IR languages away from it, they're dead data.
  - Outcome: each language declares wrappings only if it actually needs them.
- [ ] [A2WV] [C2] Decide the future of per-language `TractorNode` strum enums in `languages/{lang}/output.rs`.
  - Choice: (a) drive shape contracts off `Ir` variants alone and delete `TractorNode`, or (b) keep `TractorNode` as the human-facing vocabulary and generate it from `Ir`.
  - Outcome: one source of truth for emitted element names.
- [ ] [76SDP] [C3] Delete `transform::builder::XotBuilder` if nothing still uses it.
  - After S6 (WASM moves to IR) and S2 (legacy languages routed through XeeBuilder only), `XotBuilder` may be fully redundant.
  - Outcome: one tree-builder, not two.
- [ ] [C4] Audit and simplify `output::xml_node_to_json` once `Tree::Xml` is rare.
  - After S5 + S8, it's only reached by XPath partial matches.
  - Outcome: the legacy XML→JSON projection shrinks to the cases that genuinely need it.
- [ ] [C5] Reconcile `tractor/src/languages/info.rs::LANGUAGES` (the `LanguageInfo` array + `Language` enum) with `tractor/src/languages/mod.rs::LANGUAGES` (the `LanguageOps` array).
  - Surfaced during S2-Z3: there are *two* `LANGUAGES` arrays, both claiming SSoT in their docstrings. `info.rs` carries `aliases`, `has_transforms`, `grammar_file` (web-wasm path); `mod.rs` carries `extensions`, `grammar`, `ir_family`, `transform`, `post_transform`, etc.
  - Two paths: (a) merge `info.rs`'s extra fields into `LanguageOps` and delete `info.rs`; or (b) rewrite `info.rs::LANGUAGES` as a derivation of `mod.rs::LANGUAGES`, leaving `Language` (enum) and `LanguageInfo` (struct) as a thin facade if external consumers need them.
  - Also: `info.rs::LANGUAGES` still lists c/cpp/html/css/bash/scala/lua/haskell/ocaml/r/julia/xml as `has_transforms: false`, while S2-Z3 dropped them from the parser side. After consolidation, `language_info::get_language_info("cpp")` should be `None` (or `xml`-only for the passthrough case) so the two registries never disagree.
  - Outcome: there is exactly one language registry; `LanguageInfo` either disappears or is a typed view onto `LanguageOps`.

_(Out-of-scope guardrails moved to the Findings section at the top.)_
