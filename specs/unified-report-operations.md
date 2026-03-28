---
priority: 1
type: spec
supersedes: todo/19-unified-report-model.md
related:
  - specs/report-model.md
  - specs/cli-output-design.md
  - specs/codexpath/cli/output-options/group-option.md
  - docs/batch-execution-architecture.md
---

# Unified Report & Operations Model

Every tractor command is an operation. Every operation produces results.
A report is a flat collection of results with a summary. The CLI entry
point determines default view, format, and grouping — not the report
structure.

**Core invariant**: `tractor check ...` produces byte-identical output
to `tractor run config.yaml` (containing the same check as a single
operation), given the same `-v`, `-f`, `-g` flags.

---

## Current State (Problems)

1. **Two report shapes.** Single commands produce
   `Report { kind, matches, summary }`. Run wraps them:
   `Report { kind: Run, operations: [Report, ...], summary }`.
   Structured output (JSON) has different shapes depending on
   whether you used `tractor check` or `tractor run`.

2. **`kind` lives on Report, not on results.** When merging results
   from multiple operations, the command type is lost. The run report
   re-nests them as sub-reports instead of flattening.

3. **Separate renderers per command.** `render_check_report`,
   `render_query_report`, `render_set_report`, `render_run_report` —
   duplicated dispatch logic. `render_run_report` has a large match
   block to re-dispatch each sub-report by kind.

4. **Set summary hack.** Set reports reuse `errors`/`warnings` fields
   for `updated`/`unchanged` counts. JSON renderer special-cases this
   with `if matches!(report.kind, ReportKind::Set)`.

5. **Non-equivalent output.** `tractor check -f json` and
   `tractor run -f json` (one check op) produce structurally different
   JSON. The run output has `operations: [{kind: "check", ...}]`
   wrapping, which the check output does not.

---

## Design

### 1. ReportMatch gains `command`

Every match carries the command (operation type) that produced it:

```rust
pub struct ReportMatch {
    // Core identity
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub end_line: u32,
    pub end_column: u32,

    // Operation type — always populated
    pub command: String,  // "check", "query", "test", "set", "update"

    // Content fields (Option, populated by view)
    pub tree:     Option<XmlNode>,
    pub value:    Option<String>,
    pub source:   Option<String>,
    pub lines:    Option<Vec<String>>,
    pub reason:   Option<String>,
    pub severity: Option<Severity>,
    pub message:  Option<String>,
    pub rule_id:  Option<String>,
    pub status:   Option<String>,
    pub output:   Option<String>,
}
```

`command` is always populated. In structured output (JSON/YAML/XML)
it is always emitted. When grouped by command, it is hoisted to the
group level and omitted from individual results (see section 4).

### 2. Report becomes command-agnostic

```rust
pub struct Report {
    pub summary: Option<Summary>,
    pub results: Vec<ResultItem>,
}
```

`results` is a list that contains either individual matches or
groups (see section 4). This single field replaces the current
`matches`, `groups`, and `operations` fields.

Removed:
- **`kind: ReportKind`** — the command type is on each match, not
  the report. A report is a container, not typed.
- **`operations: Option<Vec<Report>>`** — no nested reports. All
  matches from all operations are flattened into one list.
- **Separate `matches` / `groups` fields** — unified into `results`.

The constructors `Report::check()`, `Report::query()`, etc. remain
as convenience functions. They set `command` on each match and compute
the summary. But they all produce the same `Report` type.

`Report::run()` changes from wrapping sub-reports to **merging** them:
it takes `Vec<Report>`, drains all matches into one flat list, and
computes an aggregate summary.

### 3. Summary: root aggregates + per-command breakdown

```rust
pub struct Summary {
    /// Did the batch as a whole succeed?
    pub passed: bool,
    /// Total result count across all commands.
    pub total: usize,
    /// Distinct files across all commands.
    pub files: usize,

    /// Per-command breakdown. Only commands that were actually
    /// executed appear here.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub check: Option<CheckSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub set: Option<SetSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test: Option<TestSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<QuerySummary>,
}

pub struct CheckSummary {
    pub total: usize,
    pub errors: usize,
    pub warnings: usize,
    pub files: usize,
}

pub struct SetSummary {
    pub total: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub files: usize,
}

pub struct TestSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
}

pub struct QuerySummary {
    pub total: usize,
    pub files: usize,
}
```

Key properties:
- **Root `total` and `files`** are always the aggregate across all
  commands. For a single `tractor check`, these equal
  `check.total` and `check.files`.
- **Per-command sub-summaries** carry command-specific fields with
  proper names (`updated`/`unchanged` instead of overloading
  `errors`/`warnings`). Only commands that produced results appear.
- **`passed`** is top-level only: false if any check errors, any
  test failures, or any set drift (verify mode). Queries don't
  affect `passed`.
- **Reconcilable**: `check.errors + check.warnings = check.total`,
  `set.updated + set.unchanged = set.total`, sub-totals sum to
  root `total`.

Single check example:
```json
{
  "summary": {
    "passed": false,
    "total": 3,
    "files": 2,
    "check": { "total": 3, "errors": 3, "warnings": 0, "files": 2 }
  }
}
```

Mixed batch example:
```json
{
  "summary": {
    "passed": false,
    "total": 6,
    "files": 4,
    "check": { "total": 3, "errors": 3, "warnings": 0, "files": 2 },
    "set": { "total": 3, "updated": 2, "unchanged": 1, "files": 2 }
  }
}
```

### 4. Recursive results with multi-level grouping

`results` is a list where each item is either an individual match
or a group. Groups are distinguished by the presence of a `group`
field that names the grouping dimension.

```rust
pub enum ResultItem {
    Match(ReportMatch),
    Group {
        /// Grouping dimension: "file", "command", "rule_id", etc.
        group: String,
        /// The hoisted field value. Serialized as the field itself,
        /// e.g. {group: "file", file: "src/Foo.cs"}.
        // (In Rust, stored as a (key, value) pair and serialized
        // dynamically. The key matches the group dimension.)
        /// Per-group summary (optional, for structured formats).
        summary: Option<Summary>,
        /// Nested results: either matches or sub-groups.
        results: Vec<ResultItem>,
    },
}
```

The hoisted field is serialized as-is — the same field name as on
a match. This keeps the data contract consistent: `file` means
the same thing whether it appears on a match or on a group.

**`-g file`** (single level):
```json
{
  "results": [
    {
      "group": "file",
      "file": "src/Foo.cs",
      "results": [
        { "command": "check", "line": 12, "reason": "TODO", "severity": "error" }
      ]
    }
  ]
}
```

**`-g command`** (single level):
```json
{
  "results": [
    {
      "group": "command",
      "command": "check",
      "results": [
        { "file": "src/Foo.cs", "line": 12, "reason": "TODO", "severity": "error" }
      ]
    }
  ]
}
```

**`-g command,file`** (multi-level):
```json
{
  "results": [
    {
      "group": "command",
      "command": "check",
      "results": [
        {
          "group": "file",
          "file": "src/Foo.cs",
          "results": [
            { "line": 12, "reason": "TODO", "severity": "error" },
            { "line": 47, "reason": "TODO", "severity": "error" }
          ]
        },
        {
          "group": "file",
          "file": "src/Bar.cs",
          "results": [
            { "line": 3, "reason": "TODO", "severity": "error" }
          ]
        }
      ]
    },
    {
      "group": "command",
      "command": "set",
      "results": [
        {
          "group": "file",
          "file": "config.json",
          "results": [
            { "line": 1, "status": "updated" }
          ]
        }
      ]
    }
  ]
}
```

**`-g none`** (flat):
```json
{
  "results": [
    { "command": "check", "file": "src/Foo.cs", "line": 12, "reason": "TODO", "severity": "error" },
    { "command": "set", "file": "config.json", "line": 1, "status": "updated" }
  ]
}
```

Each level hoists one field. Leaf-level results carry only the
fields not hoisted by any ancestor group. The structure is
self-describing: a consumer checks for the `group` key to
distinguish groups from matches.

XML follows the same pattern:
```xml
<group file="src/Foo.cs">
  <result line="12" command="check">...</result>
</group>
```

### 5. Format is a report-level decision

The output format (`-f`) is set at the CLI level and applies to the
entire report. Config files do not specify format — they declare
*what* to do (operations); the CLI decides *how to show it*.

```
tractor run config.yaml -f json     # everything as JSON
tractor run config.yaml -f gcc      # everything as gcc-style lines
tractor run config.yaml -f text     # everything as human-readable text
```

The same config file can be rendered in any format. This prevents
mixed-format output (XML matches interleaved with JSON matches)
which would be unparseable.

### 6. GCC renders all command types

GCC format uses the `note` severity level (standard in GCC/Clang)
for non-check results. Every result has a file and line, so the
`file:line:col: level: message` template works universally:

| Command | Severity in gcc | Message source                   |
|---------|-----------------|----------------------------------|
| check   | error/warning   | `reason` field                   |
| set     | note            | `status` field ("updated", etc.) |
| query   | note            | `value` field                    |
| test    | note/error      | assertion result                 |

Example mixed batch output:
```
src/Foo.cs:12:5: error[no-todos]: TODO found
src/Foo.cs:47:5: error[no-todos]: TODO found
src/Bar.cs:3:1: error[no-todos]: TODO found
config.json:1:1: note[set]: updated

3 errors in 2 files, 1 file updated
```

Tractor maps its severity levels to gcc as:
- `error` → `error`
- `warning` → `warning`
- (non-check results) → `note`

The `info` severity is a common log level but `note` is the
established term in GCC/Clang diagnostic output.

### 7. Default format and grouping per entry point

| CLI command | Default `-g`    | Default `-f` | Rationale                           |
|-------------|-----------------|--------------|-------------------------------------|
| `query`     | `none`          | `text`       | Exploring, want flat results        |
| `check`     | `file`          | `gcc`        | Lint output, file context needed    |
| `test`      | `none`          | `text`       | Pass/fail per assertion             |
| `set`       | `file`          | `text`       | File-level mutations                |
| `run`       | `file`          | `text`       | Universal format for mixed commands |

The `run` command defaults to `-f text` because text can render all
command types sensibly. GCC is also viable (see section 6) and can
be used explicitly with `-f gcc`.

This means `tractor check -f json -g file` and
`tractor run config.yaml -f json -g file` (one check op) produce
identical JSON. The data is the same; only the default flags differ
between entry points.

---

## Comprehensive Examples

### Single check (ad-hoc) — flat

```
tractor check "src/**/*.cs" -x "//comment[contains(.,'TODO')]" \
  --reason "TODO found" -f json -g none
```

```json
{
  "summary": {
    "passed": false,
    "total": 3,
    "files": 2,
    "check": { "total": 3, "errors": 3, "warnings": 0, "files": 2 }
  },
  "results": [
    { "command": "check", "file": "src/Foo.cs", "line": 12, "column": 5, "reason": "TODO found", "severity": "error" },
    { "command": "check", "file": "src/Foo.cs", "line": 47, "column": 5, "reason": "TODO found", "severity": "error" },
    { "command": "check", "file": "src/Bar.cs", "line": 3, "column": 1, "reason": "TODO found", "severity": "error" }
  ]
}
```

### Same check via `tractor run` — identical output

```yaml
# tractor.yaml
check:
  files: ["src/**/*.cs"]
  rules:
    - id: no-todos
      xpath: "//comment[contains(.,'TODO')]"
      reason: "TODO found"
```

```
tractor run tractor.yaml -f json -g none
```

```json
{
  "summary": {
    "passed": false,
    "total": 3,
    "files": 2,
    "check": { "total": 3, "errors": 3, "warnings": 0, "files": 2 }
  },
  "results": [
    { "command": "check", "file": "src/Foo.cs", "line": 12, "column": 5, "reason": "TODO found", "severity": "error", "rule_id": "no-todos" },
    { "command": "check", "file": "src/Foo.cs", "line": 47, "column": 5, "reason": "TODO found", "severity": "error", "rule_id": "no-todos" },
    { "command": "check", "file": "src/Bar.cs", "line": 3, "column": 1, "reason": "TODO found", "severity": "error", "rule_id": "no-todos" }
  ]
}
```

Same structure. Only difference: `rule_id` present (because the config
names the rule). The inline check has no `rule_id`.

### Single check — grouped by file (default)

```
tractor check "src/**/*.cs" -x "//comment[contains(.,'TODO')]" \
  --reason "TODO found" -f json
```

```json
{
  "summary": {
    "passed": false,
    "total": 3,
    "files": 2,
    "check": { "total": 3, "errors": 3, "warnings": 0, "files": 2 }
  },
  "results": [
    {
      "group": "file",
      "file": "src/Foo.cs",
      "results": [
        { "command": "check", "line": 12, "column": 5, "reason": "TODO found", "severity": "error" },
        { "command": "check", "line": 47, "column": 5, "reason": "TODO found", "severity": "error" }
      ]
    },
    {
      "group": "file",
      "file": "src/Bar.cs",
      "results": [
        { "command": "check", "line": 3, "column": 1, "reason": "TODO found", "severity": "error" }
      ]
    }
  ]
}
```

### Mixed batch — grouped by file (default for run)

```yaml
# tractor.yaml
operations:
  - check:
      files: ["src/**/*.cs"]
      rules:
        - id: no-todos
          xpath: "//comment[contains(.,'TODO')]"
          reason: "TODO found"
  - set:
      files: ["config.json"]
      mappings:
        - xpath: "//version"
          value: "2.0"
```

```
tractor run tractor.yaml -f json
```

```json
{
  "summary": {
    "passed": false,
    "total": 4,
    "files": 3,
    "check": { "total": 3, "errors": 3, "warnings": 0, "files": 2 },
    "set": { "total": 1, "updated": 1, "unchanged": 0, "files": 1 }
  },
  "results": [
    {
      "group": "file",
      "file": "src/Foo.cs",
      "results": [
        { "command": "check", "line": 12, "reason": "TODO found", "severity": "error", "rule_id": "no-todos" },
        { "command": "check", "line": 47, "reason": "TODO found", "severity": "error", "rule_id": "no-todos" }
      ]
    },
    {
      "group": "file",
      "file": "src/Bar.cs",
      "results": [
        { "command": "check", "line": 3, "reason": "TODO found", "severity": "error", "rule_id": "no-todos" }
      ]
    },
    {
      "group": "file",
      "file": "config.json",
      "results": [
        { "command": "set", "line": 1, "status": "updated" }
      ]
    }
  ]
}
```

### Mixed batch — grouped by command, then file

```
tractor run tractor.yaml -f json -g command,file
```

```json
{
  "summary": {
    "passed": false,
    "total": 4,
    "files": 3,
    "check": { "total": 3, "errors": 3, "warnings": 0, "files": 2 },
    "set": { "total": 1, "updated": 1, "unchanged": 0, "files": 1 }
  },
  "results": [
    {
      "group": "command",
      "command": "check",
      "results": [
        {
          "group": "file",
          "file": "src/Foo.cs",
          "results": [
            { "line": 12, "reason": "TODO found", "severity": "error", "rule_id": "no-todos" },
            { "line": 47, "reason": "TODO found", "severity": "error", "rule_id": "no-todos" }
          ]
        },
        {
          "group": "file",
          "file": "src/Bar.cs",
          "results": [
            { "line": 3, "reason": "TODO found", "severity": "error", "rule_id": "no-todos" }
          ]
        }
      ]
    },
    {
      "group": "command",
      "command": "set",
      "results": [
        {
          "group": "file",
          "file": "config.json",
          "results": [
            { "line": 1, "status": "updated" }
          ]
        }
      ]
    }
  ]
}
```

### gcc output — single check

```
tractor check "src/**/*.cs" -x "//comment[contains(.,'TODO')]" --reason "TODO found"
```

```
src/Foo.cs:12:5: error: TODO found
src/Foo.cs:47:5: error: TODO found
src/Bar.cs:3:1: error: TODO found

3 errors in 2 files
```

### gcc output — mixed batch

```
tractor run tractor.yaml -f gcc
```

```
src/Foo.cs:12:5: error[no-todos]: TODO found
src/Foo.cs:47:5: error[no-todos]: TODO found
src/Bar.cs:3:1: error[no-todos]: TODO found
config.json:1:1: note[set]: updated

3 errors in 2 files, 1 file updated
```

---

## Decisions

### Format is CLI-level only

Config files declare *what* to do (operations). The CLI decides
*how to show it* (`-f`, `-v`, `-g`). The same config can be rendered
with different formats per invocation. Config files must not specify
output format — this prevents mixed-format output within a single
report.

### GCC uses `note` for non-check results

GCC and Clang define three diagnostic levels: `error`, `warning`,
`note`. Tractor uses `note` for non-check results (set, query, test)
so that gcc format can render all command types in a consistent
template. This is the standard term in compiler diagnostics — `info`
is more common in logging but not in gcc-style output.

### Hoisted fields use the same name at group and match level

When grouping hoists a field, the group carries the field with its
original name — not a generic `value` property. `{group: "file",
file: "src/Foo.cs"}`, not `{group: "file", value: "src/Foo.cs"}`.
A consumer looking for `file` finds it the same way whether on a
match or on a group. No polymorphism, no indirection.

### Recursive `results`, not named containers

Each group has a `results` list (not `commands`, `files`, `operations`,
etc.). Named containers would require every group type to declare
child properties for every other group type. Since grouping is
arbitrarily composable (`-g file`, `-g command,file`, `-g file,command`),
uniform `results` is the only viable approach.

### Group-level summaries fit naturally

Each group can carry an optional `summary` scoped to its contents.
This enables per-file error counts, per-command totals, etc. Design
and implementation deferred — the structure supports it when needed.

---

## Migration Plan

### Step 1: Add `command` to ReportMatch

Add `command: String` to `ReportMatch`. Populate it in each
`execute_*()` function:
- `execute_query()` → `"query"`
- `execute_check()` → `"check"`
- `execute_test()` → `"test"`
- `execute_set()` → `"set"`
- `execute_update()` → `"update"`

Include `command` in JSON/YAML/XML serialization. No behavior change
for existing output formats — `command` is a new field.

### Step 2: Restructure Summary

Replace the flat `Summary` with root aggregates + per-command
sub-summaries. Add proper `CheckSummary`, `SetSummary`, etc.
Remove the `errors`/`warnings` overloading hack for set reports
and the `if matches!(report.kind, ReportKind::Set)` special case
in `json.rs`.

### Step 3: Unify `results` list

Replace the separate `matches`, `groups`, and `operations` fields
with a single `results: Vec<ResultItem>` where items are either
matches or groups. This is the core structural change.

### Step 4: Flatten Report::run()

Change `Report::run(reports: Vec<Report>)` to merge all matches
into one flat `results` list instead of wrapping as sub-reports.
Compute the aggregate summary from the merged results.

### Step 5: Remove ReportKind

With `command` on each match and no nested reports, `ReportKind` is
redundant. Remove it. Update constructors and any code that pattern-
matches on `kind`.

### Step 6: Multi-level grouping

Implement `-g command,file` syntax. Extend `with_groups()` to apply
grouping dimensions in order, producing nested group structures.
Each level hoists one field from the matches below it.

### Step 7: Unify renderers

Replace the per-command render functions (`render_check_report`,
`render_query_report`, `render_set_report`, `render_run_report`)
with a single `render_report()`. The unified renderer walks the
`results` tree and uses the match's `command` field to select
per-match templates. Command-specific defaults (format, view,
grouping) stay in the CLI layer.

---

## Open Questions

1. **Test assertions in mixed batches.** A test assertion has per-
   assertion `expected` and pass/fail. With multiple test assertions
   in a batch, we need per-assertion tracking. Leaning toward making
   `expected` a match-level field for test results, similar to how
   `status` is match-level for set. `TestSummary` tracks aggregate
   `passed`/`failed` counts.

2. **Operation IDs.** Config entries currently don't have IDs (only
   rules within check operations have `id`). Adding per-operation IDs
   would enable `-g operation` to group by specific operation instance
   rather than by command type. Deferred until there's a concrete
   use case.

3. **`command` field name.** Alternatives considered: `op`, `type`,
   `operation`, `kind`. `command` matches tractor's CLI terminology
   (subcommands are "commands") and is unambiguous. It also parallels
   `rule_id` — both are annotations about what produced the match.

4. **`-v` field selection DSL.** With summary sub-sections
   (`summary.check.errors`) and the recursive `results` structure,
   simple flat view names (`-v errors`) become ambiguous. Full XPath
   on the report (`-v summary/check/errors`) conflicts with relative
   syntax (`+source,-lines`). This is a presentation concern — the
   data model doesn't depend on it. Deferred; pragmatic short names
   for now.

5. **`run` default format.** Text is the safe default since it handles
   all command types. GCC is also viable since `note` level covers
   non-check results. Could revisit once real mixed-batch usage
   patterns emerge.
