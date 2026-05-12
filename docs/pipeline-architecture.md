# Pipeline architecture

This document describes the actual data-processing pipeline of the tractor CLI as it exists today: parallelism, branching points, file paths. For the tree-side architecture (CST→tree lowering, `to_xot`, `to_json`, source renderers), see the doc-comments in `tractor/src/ir/`. For active reorganization work, see `TODO.md`.

## High-level diagram

```
CLI args  (clap, tractor/src/main.rs + tractor/src/cli/run.rs)
   │
   ▼
RunContext::build()  (tractor/src/cli/context.rs)
   │   resolves OutputFormat, ViewSet, color, verbosity, base_dir
   │
   ▼
Operation planning  (tractor/src/cli/*.rs per command, or tractor/src/cli/run.rs for `tractor run config.yaml`)
   │   per command: build OperationPlan(s) with Sources, Filters, XPaths
   │   input resolution: input::resolve_operation_inputs
   │     (glob expand, CLI intersection, diff filter, language detect,
   │      inline stdin / -s, virtual paths)
   │
   ▼
executor::execute(plans, ExecCtx, ReportBuilder)
   │   tractor/src/executor/mod.rs — thin dispatcher
   │   ┌────────┬────────┬───────┬──────┬────────┐
   │   ▼        ▼        ▼       ▼      ▼        ▼
   │  query   check    test    set   update
   │   │ (executor/query.rs etc — one fn per op type)
   │   │
   │   │ All read-only ops route through query_files_multi
   │   │ (executor/mod.rs:132):
   │   │   sources.par_iter()  ← rayon parallelism
   │   │     source.parse(lang, tree_mode, ...)
   │   │       → tractor::parser::parse  ── tree path or legacy
   │   │     result.query(xpath)
   │   │       → XPath eval (xee)
   │   │   collect Vec<Match>, sort by (file, line, col), apply limit
   │   │
   │   │ set / update mutate files; their executors apply
   │   │ replacements after the query collects target sites.
   │
   ▼
ReportBuilder (tractor/src/report.rs)
   │   add_all(Vec<ReportMatch>), grouping (check), summary
   │
   ▼
matcher::project_report(report, view)
   │   tractor/src/matcher.rs:304 — drop fields not in ViewSet
   │   (Count / Schema fields short-circuit here)
   │
   ▼
matcher::prepare_report_for_output(report, ctx)
   │   tractor/src/matcher.rs:357 — final shape + apply -m template
   │
   ▼
format::render(report, ctx)
   │   tractor/src/format/mod.rs — dispatch by OutputFormat
   │   text / json / yaml / xml / gcc / github / claude-code
   │
   ▼
stdout (matches/report)  +  stderr (summary, diagnostics)
```

## Parse path (where the tree pipeline lives)

`source.parse(...)` calls into `tractor::parser::parse` (`tractor/src/parser/mod.rs:1106`). Three tree-family branches plus a legacy branch are gated by `use_ir_pipeline(lang, mode)` (`parser/mod.rs:380`) until `TODO.md`'s S2-Z4 collapses the gate into a registry lookup. Branch-by-branch:

| Path | Languages | Where |
|---|---|---|
| **tree (programming)** — `SyntaxTree` | csharp, python, java, ts/js/tsx/jsx, rust, go, ruby, php | `parse_with_ir_pipeline_to_xee` |
| **tree (data)** — `DataTree` | json, yaml, toml, ini, env, markdown (Structure mode) | same fn, data branch |
| **tree (sql)** — `SqlTree` | tsql | same fn, sql branch |
| **Legacy imperative** — `XeeBuilder::build_with_options` + `walk_transform` | Raw mode for everything; c, cpp, html, css, bash, scala, lua, haskell, ocaml, r, julia; json/yaml in Data mode | `parser/mod.rs:933` |
| **WASM** — `XotBuilder` + `walk_transform` | All web-app parses | `tractor/src/wasm/mod.rs` |

The tree path renders to xot, **serialises the xot to a string, and re-parses it into xee `Documents`** (acknowledged "v1 stepping stone" at `parser/mod.rs:641`; TODO.md S7 closes it). After that it runs each language's `post_transform` for residual shape work plus the `list="X"` attribute pass.

### Reverse path (render / set / update value-rewrite)

`tractor render` and `tractor set / update`'s value-rewrite use a separate reverse pipeline: `tractor::render::parse_xml` / `parse_json` → `XmlNode` → `render::render(node, lang, TreeMode::Data, opts)` (`tractor/src/render/mod.rs`). It speaks `XmlNode`, not tree, and supports csharp / json / yaml only. The tree-side reverse renderer (`tractor/src/ir/source/*.rs`) is implemented for 9 languages but not yet wired into production. TODO.md S4 closes the gap.

## Parallelism

| Stage | Parallel? | Notes |
|---|---|---|
| Input resolve | No | Glob expand + lang filter are sequential |
| Parse + XPath query | **Yes** | `rayon par_iter()` over `sources` in `executor::query_files_multi` |
| Sort, truncate | No | After flatten, single-threaded |
| Per-op dispatch | No | `executor::execute` iterates `OperationPlan`s sequentially |
| Report build / projection | No | Single-pass |
| Rendering | No | Single string builder per format |
| File writes (`set` / `update`) | No | Sequential — preserves predictable error ordering |

The rayon worker pool uses a 16 MiB stack (`cli/context.rs`) because xee's XPath evaluator has deeply recursive AST walks. This was previously load-bearing for `render_to_xot` too; that recursion was decomposed in iter 38 so the stack-size hack now exists only for xee.

## Branching points

| # | Where | What it switches on |
|---|---|---|
| 1 | `main.rs` | Subcommand (`check` / `query` / `test` / `set` / `update` / `run` / `render` / `init` / `languages` / `help`) |
| 2 | `cli/run.rs` vs `cli/<cmd>.rs` | Single CLI op vs config-file batch |
| 3 | `cli/context.rs::RunContext::build` | `OutputFormat`, `ViewSet`, color |
| 4 | `input::Source::parse` (per source) | Disk file vs virtual / inline |
| 5 | `parser::parse` | tree (programming / data / sql) vs legacy imperative |
| 6 | `executor::execute` | `OperationPlan` variant — dispatches to `execute_query` / `execute_check` / `execute_test` / `execute_set` / `execute_update` |
| 7 | `matcher::project_report` | `Count` / `Schema` view → short-circuit; otherwise project per `ViewSet` |
| 8 | `format::render` | One of seven `OutputFormat`s |

## Key files

| File | Role |
|---|---|
| `tractor/src/main.rs` | Entry point, subcommand routing |
| `tractor/src/cli/mod.rs` | clap definitions |
| `tractor/src/cli/run.rs` | `tractor run config.yaml` driver |
| `tractor/src/cli/{check,query,test,set,update,render,init,languages,help,config}.rs` | per-subcommand CLI handlers |
| `tractor/src/cli/context.rs` | `RunContext::build()`, `ExecCtx`, format + view normalisation |
| `tractor/src/input/source.rs` | `Source` (disk + virtual, with `.parse()`) |
| `tractor/src/input/file_resolver.rs` | glob expansion, language filter |
| `tractor/src/input/filter.rs` | per-result filtering (severity, tag, …) |
| `tractor/src/input/git.rs` | diff-files intersection |
| `tractor/src/input/plan.rs` | `resolve_operation_inputs` — turns CLI/config args into `Vec<Source>` + `Filters` |
| `tractor/src/parser/mod.rs` | `parse()` (unified) + tree + legacy parse fns |
| `tractor/src/ir/` | typed tree + `to_xot` / `to_json` / `to_data` / `source` (reverse render, unwired) |
| `tractor/src/executor/mod.rs` | `execute()` dispatcher, `query_files_multi` (rayon) |
| `tractor/src/executor/{query,check,test,set,update}.rs` | per-op executors + their `OperationPlan` types |
| `tractor/src/matcher.rs` | `run_rules`, `project_report`, `prepare_report_for_output`, `apply_message_template` |
| `tractor/src/report.rs` *(in library: `tractor/src/model/report.rs`)* | `Report`, `ReportBuilder`, `ReportMatch`, `FileGroup`, `Summary` |
| `tractor/src/format/mod.rs` | render dispatch by `OutputFormat` |
| `tractor/src/format/{text,json,yaml,xml,gcc,github,claude_code}.rs` | per-format renderers |
| `tractor/src/format/options.rs` | `OutputFormat`, `ViewSet`, `ViewField` enum |
| `tractor/src/format/projection.rs` | view-field application onto `ReportMatch` |
| `tractor/src/render/mod.rs` | reverse renderer (XmlNode → source, csharp/json/yaml only — to be retired by TODO.md S4) |
| `tractor/src/wasm/mod.rs` | WASM bindings (still on `XotBuilder` + `walk_transform`; TODO.md S6) |

## Standardisation gaps still open

| Gap | Description | Tracked |
|---|---|---|
| Two parse APIs | `parse(ParseInput, ParseOptions)` (unified) + legacy `parse_string_to_xot` / `parse_file_to_xee` / `parse_string_to_xee` still exposed in `lib.rs` | TODO.md S9 |
| Two reverse renderers | XmlNode-based `render/*.rs` (3 languages, wired) and tree-based `ir/source/*.rs` (9 languages, unwired) | TODO.md S4 |
| Two JSON projections | `xml_to_json` (legacy), `tree_to_json` (1087 LOC of heuristics), `data_to_json` (principled). Three concurrent paths. | TODO.md S5 |
| Two language registries | `tractor/src/languages/mod.rs::LANGUAGES` + `tractor/src/languages/info.rs::LANGUAGES`, both claiming SSoT | TODO.md C5 |
| WASM vs CLI divergence | WASM bypasses `crate::tree`; produces different semantic XML for migrated languages | TODO.md S6 |
| Post-pass paradigm mix | tree pipeline renders typed → mutates rendered xot with `post_transform` shared helpers | TODO.md S3 |
| `xot.to_string()` → xee reparse | tree pipeline serialises XML and re-parses into xee Documents | TODO.md S7 |
