# Generalized Error Reporting Through the Report Model

## Context

Tractor currently handles errors (invalid CLI args, invalid XPath, missing tools like git) via `eprintln!("error: ...")` and early exit. This bypasses the report model entirely, so errors get no structured output, no source highlighting, no JSON/YAML/GitHub annotation format, and no grouping.

The goal is to route errors through the same `Report`/`ReportMatch`/formatter pipeline as regular results, so they benefit from all existing output formats and rendering (including source highlighting with caret marks). This also lays groundwork for linter-style suggested fixes.

## Design Decisions

- **Severity**: Extend from 2 levels (`Error`, `Warning`) to 4 levels: `Fatal`, `Error`, `Warning`, `Info`.
  - `Fatal` = tractor broke (invalid XPath, missing git, unparseable config). Always causes `success: false`.
  - `Error` = user-defined rule violation at error level. From check rules.
  - `Warning` = user-defined rule violation at warning level. From check rules.
  - `Info` = helpful tractor feedback (0 matches, typo suggestions). Doesn't affect success.
  - Users can only set `--severity error|warning` on their rules. `Fatal` and `Info` are reserved for tractor.
- **Command field**: Error/info matches use the **intended command** (e.g. `"check"`, `"query"`). They appear inline with real results.
- **Hint field**: A simple `hint: Option<String>` text field on `ReportMatch`. Human-readable suggestion like "did you mean: //function_item".
- **DiagnosticOrigin**: Enum on `ReportMatch` for non-file sources (Xpath, Cli, Config, Input). Renderers display this in place of the file path when `file` is empty — avoids fake file strings like `<cli>` or `<stdin>`.
- **Keep `ReportMatch`** as the struct name — it's polymorphic enough to hold findings, errors, and info.
- **No Diagnostic builder in tractor-core** — `tractor-core` compiles to WASM and must not depend on CLI concepts (`std::env::args`, etc). Diagnostic `ReportMatch` items are constructed directly at call sites.
- **Errors absorbed into Report** — execution functions (executor) catch expected failures (e.g. invalid XPath) and add them as fatal `ReportMatch` entries to the `ReportBuilder`. No special error type needed for propagation. Unexpected errors that reach `main()` are wrapped in a minimal fatal report for format-aware rendering.
- **Validation consolidated at executor level** — XPath validation happens once in `execute_query`/`execute_check`, not scattered across lower-level query/rule functions.
- **ReportBuilder collector** — a single `ReportBuilder` accumulates matches across all operations. Totals and success are derived from match data on `build()`. Executors are pure match producers.

## Error Flow

1. Executor validates inputs (XPath expressions, etc.) before running queries.
2. On validation failure: add `ReportMatch` with `Severity::Fatal` to the builder and return early.
3. On unexpected errors that reach `main()`: wrap in a fatal `ReportMatch` via `ReportBuilder` and render via `render_error_report()`.
4. Machine-consumed formats (JSON, YAML, XML, GitHub) render to stdout; human formats (text, gcc) render to stderr.

## Renderer Behavior

- **GitHub**: Maps `Fatal → error`, `Info → notice` (GitHub Actions only supports `error`, `warning`, `notice`).
- **GCC**: Maps `Fatal → error`, `Info → note` (standard gcc severity labels). Uses `origin.as_str()` as prefix when file is empty (like gcc's `cc1: error: ...`). Renders `hint` as `note:` line.
- **Text**: Renders `hint` as `  hint: ...` after match content. Shows `origin` when file is empty.
- **JSON/YAML/XML**: Include `hint`, `origin`, `fatals`, `infos` when present.

## Future Error Sites (incremental, each independent)

| Error site | File | What to do |
|---|---|---|
| Missing `-x` in check | `modes/check.rs` | Fatal match with hint "add -x '//xpath'" |
| Invalid severity value | `modes/check.rs` | Highlight the bad value |
| Invalid `--format`/`--view`/`--group` | `pipeline/context.rs` | Highlight the bad value |
| `--diff-files` but no git | `pipeline/git.rs` | Fatal with hint "install git or remove --diff-files" |
| Config parse error | `tractor_config.rs` | Config file as source, serde error location |
| Config invalid XPath | `tractor_config.rs` | Config file as source, highlight XPath value |
| `--string` without `--lang` | `pipeline/input.rs` | Fatal with hint "add -l \<language\>" |
| Query with 0 matches (typo?) | `executor.rs` | Info with hint "did you mean: //function" |
