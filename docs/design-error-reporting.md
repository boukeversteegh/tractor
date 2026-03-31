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
- **CLI source**: Context-dependent — full argv for missing args, just the bad value for invalid arg values. The builder supports both.
- **Keep `ReportMatch`** as the struct name — it's polymorphic enough to hold findings, errors, and info.

## Implementation

### Phase 1: Core Model Extensions

#### 1a. Extend `Severity` enum

**File: `tractor-core/src/report.rs`**

```rust
pub enum Severity {
    Fatal,    // tractor broke (invalid input, missing tool)
    Error,    // user-defined finding at error level
    Warning,  // user-defined finding at warning level
    Info,     // tractor helpful feedback (0 matches, suggestions)
}
```

- Update `as_str()`: `Fatal → "fatal"`, `Info → "info"`
- Update `Serialize` (already uses `serde(rename_all = "lowercase")` — just add variants)

#### 1b. Add `hint` field to `ReportMatch`

**File: `tractor-core/src/report.rs`**

- Add `pub hint: Option<String>` to `ReportMatch` (after `message`)
- Update `Serialize` impl to emit `hint` when present (same pattern as `reason`, `message`)
- Update `make_report_match()` test helper with `hint: None`
- Update all test `ReportMatch` constructions with `hint: None`

#### 1c. Update `Totals` struct

**File: `tractor-core/src/report.rs`**

Add `fatals: usize` and `infos: usize` fields (both `skip_serializing_if = "is_zero"`).

#### 1d. Add `Diagnostic` builder

**File: `tractor-core/src/diagnostic.rs`** (new)

Builder struct for constructing error/info `ReportMatch` items ergonomically:

```rust
pub struct Diagnostic {
    severity: Severity,
    reason: String,
    hint: Option<String>,
    command: String,
    file: String,
    line: u32,
    column: u32,
    end_line: u32,
    end_column: u32,
    source: Option<String>,
    lines: Option<Vec<String>>,
}

impl Diagnostic {
    // Constructors
    pub fn fatal(reason: &str) -> Self;  // severity=Fatal
    pub fn info(reason: &str) -> Self;   // severity=Info

    // Chaining methods
    pub fn command(self, cmd: &str) -> Self;
    pub fn hint(self, hint: &str) -> Self;
    pub fn file(self, file: &str) -> Self;
    pub fn location(self, line: u32, col: u32, end_line: u32, end_col: u32) -> Self;
    pub fn source(self, source: &str) -> Self;
    pub fn source_lines(self, lines: Vec<String>) -> Self;

    // CLI convenience
    pub fn cli_source(self) -> Self;                  // file="<cli>", source=joined argv
    pub fn cli_highlight(self, needle: &str) -> Self; // find needle in argv, set column span

    // Terminal methods
    pub fn build(self) -> ReportMatch;
    pub fn into_report(self) -> Report;
}
```

Utility functions:
- `find_span(haystack, needle) -> Option<(u32, u32)>` — find substring, return 1-based (col, end_col)
- `offset_to_location(content, byte_offset) -> (u32, u32)` — byte offset to (line, col) in multi-line string
- `cli_invocation() -> String` — `std::env::args().join(" ")`

#### 1e. Add `Report::from_diagnostics()` constructor

**File: `tractor-core/src/report.rs`**

```rust
pub fn from_diagnostics(matches: Vec<ReportMatch>) -> Self
```

Computes totals from the matches (fatals, errors, warnings, infos counts). Sets `success: Some(false)` if any fatals or errors exist.

#### 1f. Export new types

**File: `tractor-core/src/lib.rs`**

Add `pub mod diagnostic;` and re-export `Diagnostic`.

### Phase 2: Renderer Updates

All renderers need to handle the new severity levels and the `hint` field.

#### 2a. Text renderer

**File: `tractor/src/pipeline/format/text.rs`**

In `append_match()`, after rendering reason, render `hint` if present:
```
invalid XPath syntax near position 3
fatal
  hint: did you mean: //function
```
Show hint whenever it's `Some`, regardless of view fields.

#### 2b. GCC renderer

**File: `tractor/src/pipeline/format/gcc.rs`**

- The severity label in `render_gcc_match()` already comes from `rm.severity.as_str()` — so `fatal` and `info` render automatically.
- After the main line, if `rm.hint` is `Some`, append: `  note: <hint text>`

```
<cli>:1:18: fatal: invalid XPath syntax
  note: did you mean: //function
src/main.rs:10:5: error: TODO comment found
```

#### 2c. GCC summary

**File: `tractor/src/pipeline/format/mod.rs`** (`print_gcc_summary`)

Update to include fatals in the summary: `1 fatal, 2 errors in 2 files`

#### 2d. Text summary

**File: `tractor/src/pipeline/format/text.rs`** (`format_summary`)

Update to handle fatal counts in summary output.

#### 2e. JSON renderer

**File: `tractor/src/pipeline/format/json.rs`**

Add `"hint"` key to the match object when present. Severity already serializes via serde.

#### 2f. YAML renderer

**File: `tractor/src/pipeline/format/yaml.rs`**

Same as JSON — add `hint` when present.

#### 2g. XML renderer

**File: `tractor/src/pipeline/format/xml.rs`**

Add `hint` as an attribute or child element on the match XML node.

#### 2h. GitHub renderer

**File: `tractor/src/pipeline/format/github.rs`**

Append hint to the annotation message when present.

### Phase 3: Error Propagation Integration

#### 3a. Update all `ReportMatch` construction sites

**File: `tractor/src/executor.rs`**

Add `hint: None` to `match_to_report_match()` and every other place `ReportMatch` is constructed.

#### 3b. Format-aware fallback in `main()`

**File: `tractor/src/main.rs`**

Every `*Args` struct has a `format: String` with a clap default value. Extract it before the mode dispatch:

```rust
// Before the mode match:
let format_str = match &cli.command {
    Some(Command::Check(a)) => &a.format,
    Some(Command::Query(a)) => &a.format,
    Some(Command::Test(a))  => &a.format,
    Some(Command::Set(a))   => &a.format,
    Some(Command::Run(a))   => &a.format,
    Some(Command::Update(_)) | Some(Command::Render(_)) => "text",
    None => &cli.query.format,
};
let fallback_format = OutputFormat::from_str(format_str).unwrap_or(OutputFormat::Text);
let use_color = should_use_color(...);

// In error handling:
if let Err(e) = result {
    if e.downcast_ref::<SilentExit>().is_some() {
        return ExitCode::FAILURE;
    }
    let report = Diagnostic::fatal(&e.to_string()).into_report();
    render_error_report(&report, fallback_format, use_color);
    return ExitCode::FAILURE;
}
```

Add `render_error_report(report, format, use_color)` — a small helper that renders with a sensible default view (reason, severity, hint, source/lines).

### Phase 4: Convert Specific Error Sites (incremental, each independent)

Each can be done as follow-up work:

| Error site | File | What to do |
|---|---|---|
| Missing `-x` in check | `modes/check.rs:32` | `Diagnostic::fatal("...").command("check").hint("add -x '//xpath'").cli_source()` |
| Invalid severity value | `modes/check.rs:21` | Highlight the bad value |
| Invalid XPath compilation | `executor.rs` | Catch XPath engine error, extract position, highlight in XPath string |
| Invalid `--format`/`--view`/`--group` | `pipeline/context.rs` | Highlight the bad value |
| `--diff-files` but no git | `pipeline/git.rs` | `Diagnostic::fatal("...").hint("install git or remove --diff-files")` |
| Config parse error | `tractor_config.rs` | Config file as source, serde error location |
| Config invalid XPath | `tractor_config.rs` | Config file as source, highlight XPath value |
| `--string` without `--lang` | `pipeline/input.rs` | `Diagnostic::fatal("...").hint("add -l <language>")` |
| Query with 0 matches (typo?) | `executor.rs` or mode | `Diagnostic::info("...").hint("did you mean: //function")` |

### Key Files Summary

| File | Change type |
|---|---|
| `tractor-core/src/report.rs` | Extend `Severity`, add `hint` field, update `Totals`, add `Report::from_diagnostics()` |
| `tractor-core/src/diagnostic.rs` | **New** — `Diagnostic` builder + utility functions |
| `tractor-core/src/lib.rs` | Export `diagnostic` module |
| `tractor/src/executor.rs` | Add `hint: None` to `ReportMatch` constructions |
| `tractor/src/main.rs` | Extract format before dispatch, format-aware error fallback |
| `tractor/src/pipeline/format/text.rs` | Render `hint`, update summary for fatals |
| `tractor/src/pipeline/format/gcc.rs` | Render `hint` as `note:` line |
| `tractor/src/pipeline/format/mod.rs` | Update gcc summary for fatals |
| `tractor/src/pipeline/format/json.rs` | Include `hint` in output |
| `tractor/src/pipeline/format/yaml.rs` | Include `hint` in output |
| `tractor/src/pipeline/format/xml.rs` | Include `hint` in output |
| `tractor/src/pipeline/format/github.rs` | Append `hint` to annotation |

### Existing Utilities to Reuse

- `render_source_precomputed()` in `tractor-core/src/output/formatter.rs` — renders source snippet with caret highlighting at given line/col range. The `Diagnostic` builder populates `source` + `line`/`column` so this works automatically.
- `render_lines()` in same file — multi-line source with gutter and highlighting
- `normalize_path()` in `tractor-core/src/output/mod.rs` — path normalization for `file` field
- `Report::check()`, `Report::query()` constructors — pattern for `Report::from_diagnostics()`
- `SilentExit` in `tractor/src/main.rs` — existing error type, continues to work

### Verification

1. **Unit tests**: Test `Diagnostic` builder produces correct `ReportMatch` fields. Test `Report::from_diagnostics()` computes correct totals (including fatals/infos).
2. **Serialization tests**: Verify `hint` appears in JSON when present, absent when None. Verify `fatal`/`info` severity serializes correctly.
3. **Manual testing**: Run `tractor check "*.rs"` (no `-x`) and verify the error renders through the report pipeline in text, gcc, and json formats.
4. **Existing tests**: Run `cargo test` — all existing tests pass (only added `hint: None` and new severity variants).
5. **Integration tests**: Check that `tests/integration/formats/` snapshot tests still pass.
