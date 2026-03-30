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

### 1. The report is a group

A report is not a special top-level container — it is a group. The
root group, sub-groups, and leaf results all share the same recursive
structure. Every level can carry totals, a grouping declaration for
its children, and a results list.

```rust
pub struct Report {
    pub success: Option<bool>,
    pub totals: Option<Totals>,
    pub group: Option<String>,       // what `results` is grouped by
    pub results: Vec<ResultItem>,
}

pub enum ResultItem {
    Match(ReportMatch),
    Group(Report),  // same structure, recursively
}
```

A group that has `group: "file"` declares that its `results` are
grouped by file. Each child group carries the hoisted `file` field.
If `group` is absent, `results` contains leaf matches.

This replaces the current `Report`, `FileGroup`, and the nested
`operations: Vec<Report>`. One recursive type for everything.

### 2. ReportMatch gains `command`

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
group level and omitted from individual results.

### 3. Group key lives on the parent

The grouping dimension is declared on the parent, not repeated on
each child. The parent says "my results are grouped by X" and each
child carries only the X value.

**Not this** (redundant):
```json
{
  "results": [
    { "group": "file", "file": "src/Foo.cs", "results": [...] },
    { "group": "file", "file": "src/Bar.cs", "results": [...] }
  ]
}
```

**This** (group key on parent):
```json
{
  "group": "file",
  "results": [
    { "file": "src/Foo.cs", "results": [...] },
    { "file": "src/Bar.cs", "results": [...] }
  ]
}
```

For multi-level grouping, each level declares its own sub-grouping:

```json
{
  "group": "command",
  "results": [
    {
      "command": "check",
      "group": "file",
      "results": [
        { "file": "src/Foo.cs", "results": [...] },
        { "file": "src/Bar.cs", "results": [...] }
      ]
    }
  ]
}
```

### 4. Totals

The `totals` object carries all numeric aggregates. It appears on
the root group and optionally on any sub-group. The structure is
the same at every level.

```rust
pub struct Totals {
    /// Number of results at or below this group.
    pub results: usize,
    /// Distinct files at or below this group.
    pub files: usize,
    /// Check-specific counts (present when check results exist).
    #[serde(skip_serializing_if = "is_zero")]
    pub errors: usize,
    #[serde(skip_serializing_if = "is_zero")]
    pub warnings: usize,
    /// Set-specific counts (present when set results exist).
    #[serde(skip_serializing_if = "is_zero")]
    pub updated: usize,
    #[serde(skip_serializing_if = "is_zero")]
    pub unchanged: usize,
}
```

Key properties:
- **No per-command sub-objects in totals.** If you want per-command
  breakdown, group by command — each command group carries its own
  `totals`. This avoids duplication: root `totals.check.errors`
  would just duplicate the check group's `totals.errors`.
- **Zero-valued fields are omitted.** A pure check report's totals
  won't have `updated`/`unchanged`.
- **Reconcilable**: `errors + warnings` = total check results,
  `updated + unchanged` = total set results, group totals sum to
  parent totals.

### 5. `success`

`success` is a top-level boolean on the root group. It is the
aggregate verdict:
- false if any check errors exist
- false if any test assertions failed
- false if any set drift detected (verify mode)
- queries don't affect `success`

`success` lives alongside `totals`, not inside it — it's a verdict,
not a count.

### 6. Format is a report-level decision

The output format (`-f`) is set at the CLI level and applies to the
entire report. Config files do not specify format — they declare
*what* to do (operations); the CLI decides *how to show it*.

```
tractor run config.yaml -f json     # everything as JSON
tractor run config.yaml -f gcc      # everything as gcc-style lines
tractor run config.yaml -f text     # everything as human-readable text
```

The same config file can be rendered in any format. This prevents
mixed-format output (XML results interleaved with JSON results)
which would be unparseable.

### 7. GCC renders all command types

GCC format uses the `note` severity level (standard in GCC/Clang)
for non-check results. Every result has a file and line, so the
`file:line:col: level: message` template works universally:

| Command | Severity in gcc | Message source                   |
|---------|-----------------|----------------------------------|
| check   | error/warning   | `reason` field                   |
| set     | note            | `status` field ("updated", etc.) |
| query   | note            | `value` field                    |
| test    | note/error      | assertion result                 |

Tractor maps its severity levels to gcc as:
- `error` → `error`
- `warning` → `warning`
- (non-check results) → `note`

The `info` severity is a common log level but `note` is the
established term in GCC/Clang diagnostic output.

### 8. Rule validation in the report

When `tractor check` runs rules that have valid/invalid examples,
the validation results are included in the main report as
`command: "test"` results. They are not separated into stderr or
a side channel.

Rationale:
- Tractor always respects the chosen output format. Structured output
  is essential for downstream processing and CI/CD pipelines.
- Validation failures need GitHub annotations pointing to the rule
  file — this requires file/line/reason in the standard report
  structure.
- Composability: all orthogonal settings (`-f`, `-v`, `-g`) apply
  uniformly, including to error results.
- Validation errors are structurally identical to regular match
  results — file (the rule file), line, reason, severity.

With `-g file` as default, validation results naturally separate
from check results because they reference different files (rule
files vs source files):

```
rules/tractor.yaml:3:1: note[test]: valid example passed (no-todos)
rules/tractor.yaml:5:1: error[test]: invalid example failed (no-todos)
src/Foo.cs:12:5: error[no-todos]: TODO found
src/Bar.cs:3:1: error[no-todos]: TODO found

1 validation error, 2 check errors in 3 files
```

In JSON with `-g file`, the rule file gets its own file group:
```json
{
  "group": "file",
  "results": [
    {
      "file": "rules/tractor.yaml",
      "results": [
        { "command": "test", "line": 3, "severity": "note", ... },
        { "command": "test", "line": 5, "severity": "error", ... }
      ]
    },
    {
      "file": "src/Foo.cs",
      "results": [
        { "command": "check", "line": 12, ... }
      ]
    }
  ]
}
```

No extra nesting level. No dynamic grouping. File-based grouping
naturally separates concerns.

### 9. Default format and grouping per entry point

| CLI command | Default `-g`    | Default `-f` | Rationale                           |
|-------------|-----------------|--------------|-------------------------------------|
| `query`     | `none`          | `text`       | Exploring, want flat results        |
| `check`     | `file`          | `gcc`        | Lint output, file context needed    |
| `test`      | `none`          | `text`       | Pass/fail per assertion             |
| `set`       | `file`          | `text`       | File-level mutations                |
| `run`       | `command,file`  | `text`       | Mixed commands, separated naturally |

Single-command entry points use single-level grouping. `run` uses
`command,file` to separate different operation types, with file
sub-grouping within each.

With the same explicit flags, `tractor check -f json -g file` and
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
  "success": false,
  "totals": { "results": 3, "files": 2, "errors": 3 },
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
  "success": false,
  "totals": { "results": 3, "files": 2, "errors": 3 },
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
  "success": false,
  "totals": { "results": 3, "files": 2, "errors": 3 },
  "group": "file",
  "results": [
    {
      "file": "src/Foo.cs",
      "totals": { "results": 2, "errors": 2 },
      "results": [
        { "command": "check", "line": 12, "column": 5, "reason": "TODO found", "severity": "error" },
        { "command": "check", "line": 47, "column": 5, "reason": "TODO found", "severity": "error" }
      ]
    },
    {
      "file": "src/Bar.cs",
      "totals": { "results": 1, "errors": 1 },
      "results": [
        { "command": "check", "line": 3, "column": 1, "reason": "TODO found", "severity": "error" }
      ]
    }
  ]
}
```

### Mixed batch — grouped by file

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
tractor run tractor.yaml -f json -g file
```

```json
{
  "success": false,
  "totals": { "results": 4, "files": 3, "errors": 3, "updated": 1 },
  "group": "file",
  "results": [
    {
      "file": "src/Foo.cs",
      "totals": { "results": 2, "errors": 2 },
      "results": [
        { "command": "check", "line": 12, "reason": "TODO found", "severity": "error", "rule_id": "no-todos" },
        { "command": "check", "line": 47, "reason": "TODO found", "severity": "error", "rule_id": "no-todos" }
      ]
    },
    {
      "file": "src/Bar.cs",
      "totals": { "results": 1, "errors": 1 },
      "results": [
        { "command": "check", "line": 3, "reason": "TODO found", "severity": "error", "rule_id": "no-todos" }
      ]
    },
    {
      "file": "config.json",
      "totals": { "results": 1, "updated": 1 },
      "results": [
        { "command": "set", "line": 1, "status": "updated" }
      ]
    }
  ]
}
```

### Mixed batch — grouped by command, then file (default for run)

```
tractor run tractor.yaml -f json
```

```json
{
  "success": false,
  "totals": { "results": 4, "files": 3, "errors": 3, "updated": 1 },
  "group": "command",
  "results": [
    {
      "command": "check",
      "totals": { "results": 3, "files": 2, "errors": 3 },
      "group": "file",
      "results": [
        {
          "file": "src/Foo.cs",
          "totals": { "results": 2, "errors": 2 },
          "results": [
            { "line": 12, "reason": "TODO found", "severity": "error", "rule_id": "no-todos" },
            { "line": 47, "reason": "TODO found", "severity": "error", "rule_id": "no-todos" }
          ]
        },
        {
          "file": "src/Bar.cs",
          "totals": { "results": 1, "errors": 1 },
          "results": [
            { "line": 3, "reason": "TODO found", "severity": "error", "rule_id": "no-todos" }
          ]
        }
      ]
    },
    {
      "command": "set",
      "totals": { "results": 1, "files": 1, "updated": 1 },
      "group": "file",
      "results": [
        {
          "file": "config.json",
          "totals": { "results": 1, "updated": 1 },
          "results": [
            { "line": 1, "status": "updated" }
          ]
        }
      ]
    }
  ]
}
```

### Check with rule validation — grouped by file (default)

```
tractor check --rules rules/tractor.yaml "src/**/*.cs" -f json
```

```json
{
  "success": false,
  "totals": { "results": 5, "files": 3, "errors": 4 },
  "group": "file",
  "results": [
    {
      "file": "rules/tractor.yaml",
      "totals": { "results": 2, "errors": 1 },
      "results": [
        { "command": "test", "line": 3, "severity": "note", "reason": "valid example passed", "rule_id": "no-todos" },
        { "command": "test", "line": 5, "severity": "error", "reason": "invalid example failed: expected some, got 0", "rule_id": "no-todos" }
      ]
    },
    {
      "file": "src/Foo.cs",
      "totals": { "results": 2, "errors": 2 },
      "results": [
        { "command": "check", "line": 12, "reason": "TODO found", "severity": "error", "rule_id": "no-todos" },
        { "command": "check", "line": 47, "reason": "TODO found", "severity": "error", "rule_id": "no-todos" }
      ]
    },
    {
      "file": "src/Bar.cs",
      "totals": { "results": 1, "errors": 1 },
      "results": [
        { "command": "check", "line": 3, "reason": "TODO found", "severity": "error", "rule_id": "no-todos" }
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

### gcc output — check with validation failure

```
tractor check --rules rules/tractor.yaml "src/**/*.cs"
```

```
rules/tractor.yaml:5:1: error[test]: invalid example failed: expected some, got 0 (no-todos)
src/Foo.cs:12:5: error[no-todos]: TODO found
src/Foo.cs:47:5: error[no-todos]: TODO found
src/Bar.cs:3:1: error[no-todos]: TODO found

1 validation error, 3 check errors in 3 files
```

---

## Decisions

### The report is a group

The root report and sub-groups share the same structure: `success`,
`totals`, `group` (declaring what children are grouped by), and
`results`. There is no separate `Summary` wrapper — `success` and
`totals` are direct fields on the group. This makes the report
self-similar at every level.

### Group key lives on the parent

The grouping dimension is declared on the parent group, not repeated
on each child. The parent says `group: "file"` and each child
carries only `file: "src/Foo.cs"`. A consumer reads the parent's
`group` field to know how to interpret the children. No redundancy.

### Hoisted fields use the same name at group and match level

When grouping hoists a field, the group carries the field with its
original name — not a generic `value` property. `{file: "src/Foo.cs"}`
on a group, same as on a match. A consumer looking for `file` finds
it the same way at any level. No polymorphism, no indirection.

### No per-command breakdown in totals

Per-command stats (check errors, set updates) are not nested inside
`totals` as sub-objects. If you want per-command breakdown, group by
command — each command group carries its own `totals` with the
relevant fields. This avoids duplication: a root-level
`totals.check.errors` would just duplicate the check group's
`totals.errors`.

### Totals use command-specific field names

`totals` has flat fields: `results`, `files`, `errors`, `warnings`,
`updated`, `unchanged`. Zero-valued fields are omitted. A pure check
report shows `errors` and `warnings`; a pure set report shows
`updated` and `unchanged`. Mixed reports show all relevant fields.

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

### Recursive `results`, not named containers

Each group has a `results` list (not `commands`, `files`, `operations`,
etc.). Named containers would require every group type to declare
child properties for every other group type. Since grouping is
arbitrarily composable (`-g file`, `-g command,file`, `-g file,command`),
uniform `results` is the only viable approach.

### Rule validation results go in the report

When `tractor check` validates rule examples, the validation results
are included in the main report as `command: "test"` results pointing
to the rule file with line numbers. This preserves structured output,
enables CI/CD annotations, and maintains composability. With file-based
grouping, validation results naturally separate from check results
because they reference different files (rule files vs source files).

### Group-level totals

Each group can carry `totals` scoped to its contents. This enables
per-file error counts, per-command totals, etc. The structure is
the same `Totals` type at every level.

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

### Step 2: Introduce `Totals` and `success`

Replace the flat `Summary` with `Totals` (counts only) and a
top-level `success` boolean. `Totals` has `results`, `files`, plus
command-specific fields (`errors`, `warnings`, `updated`,
`unchanged`). Remove the `errors`/`warnings` overloading hack for
set reports and the `if matches!(report.kind, ReportKind::Set)`
special case in `json.rs`.

### Step 3: Unify into recursive `results`

Replace the separate `matches`, `groups`, and `operations` fields
with a single `results: Vec<ResultItem>` where items are either
matches or groups. Groups have the same structure as the root
report. This is the core structural change.

### Step 4: Group key on parent

Move the `group` field from each child to the parent. The parent
declares `group: "file"` and children carry only `file: "..."`.

### Step 5: Flatten Report::run()

Change `Report::run(reports: Vec<Report>)` to merge all matches
into one flat `results` list instead of wrapping as sub-reports.
Compute the aggregate `totals` and `success` from the merged results.

### Step 6: Remove ReportKind

With `command` on each match and no nested reports, `ReportKind` is
redundant. Remove it. Update constructors and any code that pattern-
matches on `kind`.

### Step 7: Multi-level grouping

Implement `-g command,file` syntax. Extend grouping to apply
dimensions in order, producing nested group structures. Each level
hoists one field from the results below it. Each group carries its
own `totals`.

### Step 8: Unify renderers

Replace the per-command render functions (`render_check_report`,
`render_query_report`, `render_set_report`, `render_run_report`)
with a single `render_report()`. The unified renderer walks the
`results` tree recursively and uses the match's `command` field to
select per-match templates. Command-specific defaults (format, view,
grouping) stay in the CLI layer.

---

## Open Questions

1. **Test assertions in mixed batches.** A test assertion has per-
   assertion `expected` and pass/fail. With multiple test assertions
   in a batch, we need per-assertion tracking. Leaning toward making
   `expected` a match-level field for test results, similar to how
   `status` is match-level for set.

2. **Operation IDs.** Config entries currently don't have IDs (only
   rules within check operations have `id`). Adding per-operation IDs
   would enable `-g operation` to group by specific operation instance
   rather than by command type. Deferred until there's a concrete
   use case.

3. **`command` field name.** Alternatives considered: `op`, `type`,
   `operation`, `kind`. `command` matches tractor's CLI terminology
   (subcommands are "commands") and is unambiguous. It also parallels
   `rule_id` — both are annotations about what produced the match.

4. **`-v` field selection DSL.** With `totals` as a nested object
   and the recursive `results` structure, the `-v` parameter needs
   to address nested fields. `-v totals` selects the totals object.
   Full XPath-style paths (`totals/errors`) vs flat short names
   (`errors`) vs relative syntax (`+source,-lines`) — these are
   presentation concerns. The data model doesn't depend on it.
   Deferred; pragmatic short names for now.

5. **`run` default format.** Text is the safe default since it handles
   all command types. GCC is also viable since `note` level covers
   non-check results. Could revisit once real mixed-batch usage
   patterns emerge.
