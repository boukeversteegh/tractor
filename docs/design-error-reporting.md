# Generalized Error Reporting Through the Report Model

## Context

Tractor currently handles errors (invalid CLI args, invalid XPath, missing tools like git) via `eprintln!("error: ...")` and early exit. This bypasses the report model entirely, so errors get no structured output, no source highlighting, no JSON/YAML/GitHub annotation format, and no grouping.

The goal is to route errors through the same `Report`/`ReportMatch`/formatter pipeline as regular results, so they benefit from all existing output formats and rendering (including source highlighting with caret marks).

## Implemented

- **Severity**: 4 levels: `Fatal`, `Error`, `Warning`, `Info`.
  - `Fatal` = tractor broke (invalid XPath, missing git, unparseable config). Always causes `success: false`, even in NoVerdict mode (query).
  - `Error` = user-defined rule violation at error level. From check rules.
  - `Warning` = user-defined rule violation at warning level. From check rules.
  - `Info` = helpful tractor feedback. Doesn't affect success.
  - Users can only set `--severity error|warning` on their rules. `Fatal` and `Info` are reserved for tractor.
- **DiagnosticOrigin**: Enum on `ReportMatch` for non-file sources (Xpath, Cli, Config, Input). Renderers display this in place of the file path when `file` is empty.
- **No Diagnostic builder in tractor-core** — diagnostic `ReportMatch` items are constructed directly at call sites.
- **Errors absorbed into Report** — executor catches expected failures (e.g. invalid XPath) and adds them as fatal `ReportMatch` entries to the `ReportBuilder`. Unexpected errors that reach `main()` are wrapped in a minimal fatal report.
- **Validation consolidated at executor level** — XPath validation happens once in `execute_query`/`execute_check`.
- **ReportBuilder collector** — a single `ReportBuilder` accumulates matches across all operations. Totals and success are derived from match data on `build()`.
- **Diagnostic field preservation** — `project_report` skips view-field stripping for Fatal matches, so diagnostic fields (reason, severity, lines, etc.) are always available to renderers.
- **Shared render_fields_for_match** — all renderers use a shared function to determine which fields to render: view-requested fields first, then diagnostic extras.
- **XPath error rendering** — invalid XPath shows severity, reason, origin, and source with caret highlighting across all formats.

## Renderer Behavior

- **Text**: Renders `severity(origin): reason` inline. Shows source with caret for diagnostics. Skips location prefix for file-less diagnostics.
- **GCC**: Maps `Fatal → error`, `Info → note`. Uses `origin.as_str()` as prefix when file is empty. Shows source with caret.
- **GitHub**: Maps `Fatal → error`, `Info → notice`. Includes source expression and column position in message.
- **JSON/YAML/XML**: Include diagnostic extras (severity, reason, origin, lines) on Fatal matches regardless of view.

## Future Work

| Feature | Notes |
|---|---|
| Hint field on ReportMatch | Suggested fix text (e.g. "did you mean: //function_item"). Rendering scaffolding removed — re-add when a producer exists. |
| XPath warnings (e.g. text()) | Warn when valid XPath uses patterns that likely don't work as expected in tractor. |
| Stderr interleaving | Render diagnostic fields to stderr interleaved with stdout (see TODO-23). |
| Config parse errors | Config file as source, serde error location as caret. |
| 0-match typo suggestions | Info-level match suggesting similar XPath when query returns nothing. |
| Missing `-x` in check | Fatal match with hint. |
| `--diff-files` but no git | Fatal with suggestion. |
