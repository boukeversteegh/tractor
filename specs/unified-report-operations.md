---
priority: 1
type: spec
supersedes: todo/19-unified-report-model.md
---

# Unified Report & Operations Model

Every tractor command is an operation. Every operation produces matches.
A report is a flat collection of matches with a summary. The CLI entry
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

2. **`kind` lives on Report, not on matches.** When merging results
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

    // Operation type
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

`command` is always populated. Serialization behavior:

- **JSON/YAML/XML**: always emitted (part of the data contract).
- **gcc/github/text**: omitted when every match shares the same
  command (redundant). Included when mixed.

### 2. Report becomes command-agnostic

```rust
pub struct Report {
    pub matches: Vec<ReportMatch>,
    pub summary: Option<Summary>,
    pub groups: Option<Vec<Group>>,
}
```

Removed:
- **`kind: ReportKind`** — the command type is on each match, not
  the report. A report is a container, not typed.
- **`operations: Option<Vec<Report>>`** — no nested reports. All
  matches from all operations are flattened into one list.

The constructors `Report::check()`, `Report::query()`, etc. remain
as convenience functions. They set `command` on each match and compute
the summary. But they all produce the same `Report` type.

`Report::run()` changes from wrapping sub-reports to **merging** them:
it takes `Vec<Report>`, drains all matches into one flat list, and
computes an aggregate summary.

### 3. Summary becomes honest

```rust
pub struct Summary {
    pub passed: bool,
    pub total: usize,
    pub files: usize,

    // Check-specific
    #[serde(skip_serializing_if = "is_zero")]
    pub errors: usize,
    #[serde(skip_serializing_if = "is_zero")]
    pub warnings: usize,

    // Set-specific (no longer overloading errors/warnings)
    #[serde(skip_serializing_if = "is_zero")]
    pub updated: usize,
    #[serde(skip_serializing_if = "is_zero")]
    pub unchanged: usize,

    // Test-specific
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,

    // Verbose
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
}
```

Key changes:
- **`updated`/`unchanged`** are proper fields, not aliases for
  `errors`/`warnings`. The set-report special-case in JSON rendering
  goes away.
- Zero-valued command-specific fields are omitted from output. A
  pure check report won't have `updated`/`unchanged` in its JSON.
- `passed` semantics: false if any check errors OR any test failures
  OR any set drift (verify mode). Queries don't affect `passed`.

### 4. Group generalizes to multiple dimensions

```rust
pub struct Group {
    /// What this group represents.
    pub key: GroupKey,
    /// Matches within this group.
    pub matches: Vec<ReportMatch>,
    /// Full modified file content (set stdout mode).
    pub output: Option<String>,
}

pub enum GroupKey {
    File(String),
    Command(String),
}
```

The `-g` / `--group` flag gains a new value:

| Value     | Groups by       | Hoisted field  | Match omits       |
|-----------|-----------------|----------------|--------------------|
| `file`    | source file     | `file`         | `file`             |
| `command` | operation type  | `command`      | `command`          |
| `none`    | (flat list)     | —              | —                  |

Serialization uses the group key as the attribute:

**`-g file`:**
```json
{
  "groups": [
    {
      "file": "src/Foo.cs",
      "matches": [
        { "command": "check", "line": 12, "reason": "TODO found", "severity": "error" }
      ]
    }
  ]
}
```

**`-g command`:**
```json
{
  "groups": [
    {
      "command": "check",
      "matches": [
        { "file": "src/Foo.cs", "line": 12, "reason": "TODO found", "severity": "error" }
      ]
    }
  ]
}
```

XML follows the same pattern:
```xml
<group file="src/Foo.cs">...</group>
<group command="check">...</group>
```

Single-level grouping only (consistent with cli-output-design.md).

### 5. Default grouping per entry point

| CLI command | Default `-g` | Default `-f` | Rationale                        |
|-------------|--------------|--------------|----------------------------------|
| `query`     | `none`       | `text`       | Exploring, want flat results     |
| `check`     | `file`       | `gcc`        | Lint output, file context needed |
| `test`      | `none`       | `text`       | Pass/fail per assertion          |
| `set`       | `file`       | `text`       | File-level mutations             |
| `run`       | `file`       | `gcc`        | Batch = multi-rule lint by default |

The `run` command defaults to `-g file` because the most common batch
use case is multi-rule checking, where grouping by file (like eslint)
is the natural output. Users can switch to `-g command` to see
operations separated, or `-g none` for flat.

This means `tractor check -f json -g file` and
`tractor run config.yaml -f json -g file` (one check op) produce
identical JSON. The data is the same; only the default flags differ
between entry points.

---

## Examples

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
    "errors": 3
  },
  "matches": [
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
    "errors": 3
  },
  "matches": [
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
    "errors": 3
  },
  "groups": [
    {
      "file": "src/Foo.cs",
      "matches": [
        { "command": "check", "line": 12, "column": 5, "reason": "TODO found", "severity": "error" },
        { "command": "check", "line": 47, "column": 5, "reason": "TODO found", "severity": "error" }
      ]
    },
    {
      "file": "src/Bar.cs",
      "matches": [
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
    "errors": 3,
    "updated": 1
  },
  "groups": [
    {
      "file": "src/Foo.cs",
      "matches": [
        { "command": "check", "line": 12, "reason": "TODO found", "severity": "error", "rule_id": "no-todos" },
        { "command": "check", "line": 47, "reason": "TODO found", "severity": "error", "rule_id": "no-todos" }
      ]
    },
    {
      "file": "src/Bar.cs",
      "matches": [
        { "command": "check", "line": 3, "reason": "TODO found", "severity": "error", "rule_id": "no-todos" }
      ]
    },
    {
      "file": "config.json",
      "matches": [
        { "command": "set", "line": 1, "status": "updated" }
      ]
    }
  ]
}
```

### Mixed batch — grouped by command

```
tractor run tractor.yaml -f json -g command
```

```json
{
  "summary": {
    "passed": false,
    "total": 4,
    "files": 3,
    "errors": 3,
    "updated": 1
  },
  "groups": [
    {
      "command": "check",
      "matches": [
        { "file": "src/Foo.cs", "line": 12, "reason": "TODO found", "severity": "error", "rule_id": "no-todos" },
        { "file": "src/Foo.cs", "line": 47, "reason": "TODO found", "severity": "error", "rule_id": "no-todos" },
        { "file": "src/Bar.cs", "line": 3, "reason": "TODO found", "severity": "error", "rule_id": "no-todos" }
      ]
    },
    {
      "command": "set",
      "matches": [
        { "file": "config.json", "line": 1, "status": "updated" }
      ]
    }
  ]
}
```

### gcc output — single check vs run

Both produce identical gcc output with default flags:

```
tractor check "src/**/*.cs" -x "//comment[contains(.,'TODO')]" --reason "TODO found"
```

```
tractor run tractor.yaml
```

Both output:
```
src/Foo.cs:12:5: error: TODO found
src/Foo.cs:47:5: error: TODO found
src/Bar.cs:3:1: error: TODO found

3 errors in 2 files
```

(The run version includes `[no-todos]` after `error` when rule_id
is present.)

### gcc output — mixed batch

```
tractor run tractor.yaml
```

```
src/Foo.cs:12:5: error[no-todos]: TODO found
src/Foo.cs:47:5: error[no-todos]: TODO found
src/Bar.cs:3:1: error[no-todos]: TODO found
config.json: updated

3 errors, updated 1 file
```

---

## Text rendering for `command` in gcc/github formats

gcc renders command-specific templates:

| Command | gcc template                                    |
|---------|-------------------------------------------------|
| check   | `file:line:col: severity[rule_id]: reason`      |
| set     | `file:line: status`  (or `file: status` for file-level) |
| query   | `file:line:col: value`                          |
| test    | `symbol label (expected X, got Y)`              |

These are the same templates already used today by the per-command
renderers. The unified renderer selects the template based on the
match's `command` field.

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

### Step 2: Add `updated`/`unchanged` to Summary

Add proper `updated: usize` and `unchanged: usize` fields to Summary.
Populate them in set reports. Stop reusing `errors`/`warnings` for
set counts. Remove the `if matches!(report.kind, ReportKind::Set)`
special case in `json.rs`.

### Step 3: Flatten Report::run()

Change `Report::run(reports: Vec<Report>)` to merge all matches
into one flat list instead of wrapping as sub-reports. Remove the
`operations` field from Report. Compute the aggregate summary from
the merged matches.

### Step 4: Remove ReportKind

With `command` on each match and no nested reports, `ReportKind` is
redundant. Remove it. Update constructors and any code that pattern-
matches on `kind`.

### Step 5: Generalize grouping

Extend `Group` and `with_groups()` to support grouping by `command`
(in addition to `file`). Add `command` as a `-g` value. Update the
`-g` flag parsing.

### Step 6: Unify renderers

Replace the per-command render functions with a single
`render_report()`. The unified renderer uses the match's `command`
field to select per-match templates (gcc line format, text layout).
Command-specific defaults (format, view, grouping) stay in the CLI
layer — each subcommand sets its defaults before calling
`render_report()`.

---

## ViewField for `command`

Add `ViewField::Command` to the view system:

```rust
pub enum ViewField {
    // ... existing ...
    Command,
}
```

This allows explicit control:
- `-v command,file,reason,severity` — include command in output
- Default views for single-command entry points omit `Command`
- Default view for `run` includes `Command`

In text/gcc formats, `command` is rendered as a prefix or tag only
when the view includes it. In JSON/YAML/XML, it's always present
(it's structural data, not a view concern).

---

## Open Questions

1. **Test assertions in mixed batches.** A test assertion has per-
   assertion `expected` and pass/fail. With multiple test assertions
   in a batch, the summary `expected` field is no longer sufficient.
   Options: (a) drop `expected` from summary, track per-assertion in
   matches; (b) make `expected` per-match for test commands; (c) keep
   test as a summary-only result (no matches in the flat list, just
   a summary contribution). Leaning toward (b) — add `expected` as
   a match-level field for test results, similar to how `status` is
   match-level for set.

2. **Operation IDs.** Config entries currently don't have IDs (only
   rules within check operations have `id`). Should we add per-
   operation IDs? This would enable `-g operation` to group by specific
   operation instance rather than by command type. Deferred until
   there's a concrete use case.

3. **Nested grouping.** `-g command,file` would group first by command,
   then by file within each command group. This is explicitly deferred
   (cli-output-design.md: "One level of grouping"). Single-level is
   sufficient for now.

4. **`command` field name.** Alternatives considered: `op`, `type`,
   `operation`, `kind`. `command` matches tractor's CLI terminology
   (subcommands are "commands") and is unambiguous. It also parallels
   `rule_id` — both are annotations about what produced the match.
