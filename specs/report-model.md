# Tractor Report Model

Design notes for the tractor report model — how commands produce structured data,
how users select what to see, and how output is serialized.

<!-- claude session: 1c0812be-f91f-43aa-8b20-f8210699f5f7 -->

---

## Starting Point: The Current (as-is) Output System

Current output-related CLI flags:

| Flag        | What it does                                                      |
|-------------|-------------------------------------------------------------------|
| `-o`        | Format: xml, lines, source, value, gcc, json, count, schema, github |
| `-m`        | Message template ({value}, {line}, {col}, {file})                 |
| `--expect`  | Enables "test mode" — suppresses streaming, adds pass/fail       |
| `--error`   | Per-match template, only when --expect fails (always outputs gcc) |
| `--warning` | Makes --expect failures exit 0 with warning symbol               |

### Problems with the current design

1. **`--error` is misleadingly named** — it's not a severity level, it's a message template that only activates when `--expect` fails.
2. **`--expect` silently changes output behavior** — it suppresses the normal `-o` format and replaces it with a test summary + error details.
3. **`--message` and `--error` are both message templates** for different contexts, but this isn't obvious.
4. **No clean separation** between "query results" and "test/lint verdict."
5. **`-o` conflates two concerns** — serialization format (json, github) and match rendering (xml, lines, value, gcc) are controlled by the same flag.
6. **`-o count` and `-o schema`** are aggregates that replace the entire output, fundamentally different from match renderers.

### The implicit modes

The current flat flag space has implicit modes detected by flag combinations:

| Mode    | Triggered by         | Behavior change                            |
|---------|----------------------|--------------------------------------------|
| Explore | no `-x`              | Show full XML/schema                       |
| Query   | `-x`                 | Find matches, stream output                |
| Test    | `-x` + `--expect`    | Suppress streaming, show pass/fail summary |
| Mutate  | `-x` + `--set`       | Collect matches, rewrite files             |

These are already distinct code paths in `main.rs`. The idea: make them explicit subcommands.

### How current -o values map to output layers

| Old `-o` value | Layer it controls | What it actually does              |
|----------------|-------------------|------------------------------------|
| `gcc`          | match rendering   | `file:line:col: error: msg`        |
| `json`         | serialization     | JSON array of matches              |
| `github`       | serialization     | GitHub Actions annotation syntax   |
| `xml`          | match rendering   | XML fragment of matched AST        |
| `lines`        | match rendering   | Full source line(s) containing match |
| `source`       | match rendering   | Exact matched source text          |
| `value`        | match rendering   | Text content only                  |
| `count`        | aggregate         | Replaces output with a number      |
| `schema`       | aggregate         | Replaces output with structural tree |

### The envelope problem

When a command has a verdict (check, test), the entire output needs to be self-consistent. Consider `tractor check ... -o json` (old flag):

**Bad** (mixing text and JSON):
```
3 violations in 2 files
[{"file":"Foo.cs","line":12,...},...]
```

**Good** (verdict is part of the JSON structure):
```json
{
  "summary": {"violations": 3, "files": 2},
  "matches": [
    {"file": "Foo.cs", "line": 12, "column": 5, "value": "// TODO fix", "reason": "TODO found"}
  ]
}
```

The serialization must wrap *everything* — verdict and matches together.

---

## New Design (to-be)

### Subcommands

```
tractor [files] -x ...              # query (default, today's behavior)
tractor set [files] -x ... <value>  # mutate
tractor check [files] ...           # lint (every match is a violation)
tractor test [files] ...            # assertion (verify count)
```

Subcommand comes first (like git). Clap tries the first positional as a subcommand; if it doesn't match `set`/`check`/`test`, it falls through to the default query mode. Only conflict: a file literally named `set`, `check`, or `test` (no extension) — acceptable edge case.

Default (no subcommand) = query/explore mode. Most common operation, deserves zero ceremony.

### Core Insight

Each tractor command produces **structured data**: a **report**. The output pipeline has three independent concerns:

1. **Structure** — determined by the command (query, check, test). What sections and fields exist in the report.
2. **Selection** — controlled by `--view`. What parts of the report to project/render (report-level and match-level).
3. **Serialization** — controlled by `--format`. How the report is serialized to stdout (json, text, github).

These concerns are orthogonal. You can serialize any command's report in any format, and select any view within it.

### New Parameters

| Parameter        | Level  | Purpose                          | Values                                          |
|------------------|--------|----------------------------------|-------------------------------------------------|
| `--format / -f`  | output | Serialization of the report      | `text` (default), `json`, `github`              |
| `--report`       | report | Which report section to emit     | `auto` (default), `matches`, `summary`, `schema`, `count` |
| `--view`         | match  | How to render each match         | `xml` (default), `value`, `source`, `lines`, `gcc` |
| `--template`     | match  | Custom match template            | string with field refs: `{value}`, `{file}`, `{line}`, etc. |
| `--reason`       | match  | Violation text (check)           | string                                          |
| `--expect`       | report | Assertion (test only)            | `none`, `some`, or a number                     |
| `--severity`     | match  | Violation severity (check)       | `error` (default), `warning`                    |

**No `--description` flag** — summary labels are derived from context (rule name, xpath expression, etc.)

---

## The Report

The full output of any tractor command is a **report**. A report is a structured data object — conceptually a tree (tractor's native data structure). All output controls are **selectors on this tree**.

### Report structure

```yaml
report:
  summary:                        # check, test only
    passed:                       # did the command succeed?
    total:                        # match count
    files_affected:               # distinct file count
    errors:                       # error-severity count (check)
    warnings:                     # warning-severity count (check)
    expected:                     # assertion: none/some/N (test)

  matches:
    - file:                       # source file path
      line:                       # start line
      column:                     # start column
      end_line:
      end_column:
      value:                      # text content of matched node
      source:                     # exact matched source text
      lines:                      # source lines in context
      xml:                        # AST fragment
      reason:                     # violation description (check only)
      severity:                   # error/warning (check only)
      rule_id:                    # which rule matched (multi-rule only)

  schema:                         # query only, derived from matches
```

### What each command includes

```
query report:  matches[] + schema
check report:  summary + matches[] (with reason, severity per match)
test report:   summary (with expected) + matches[]
```

**Schema** is only in query reports. In check mode, schema would be confusing — empty on pass, or only showing violating node structure on fail. Schema is derived from matches, so it's relatively cheap to compute when needed.

### Common: Match (shared across all commands)

Every command produces matches. This is the shared base:

```yaml
match:
  file:                           # source file path
  line:                           # start position
  column:
  end_line:                       # end position
  end_column:
  value:                          # text content of matched node
  source:                         # exact matched source text
  lines:                          # source lines in context
  xml:                            # matched AST as XML
```

### Common: Summary (shared by check and test)

```yaml
summary:
  passed:                         # did the command succeed?
  total:                          # match count
  files_affected:                 # distinct file count
  errors:                         # error-severity count (check)
  warnings:                       # warning-severity count (check)
  expected:                       # assertion: none/some/N (test)
```

---

## Two Levels of Selection

Selection operates at two independent levels on the report tree, controlled by separate parameters:

### Report-level: `--report` — which section of the report to emit

Controls which top-level section(s) of the report to include in the output.

- `auto` (default, unset) — depends on the command:
  - Query: `matches`
  - Check: `summary` + `matches`
  - Test: `summary` + `matches`
- `matches` — only the match list
- `summary` — only the summary
- `count` — only `summary.total` (shorthand)
- `schema` — only the schema (query only)

### Match-level: `--view` — how to render each match

Controls how each individual match is displayed. Only relevant when matches are included in the output.

Predefined views:
- `xml` — AST fragment (query default)
- `value` — text content of the matched node
- `source` — exact matched source text
- `lines` — source lines in context
- `gcc` — `{file}:{line}:{column}: {reason}` (check default)

Custom templates via `--template`:
- `--template "{value}"` — equivalent to `--view value`
- `--template "{file}:{line}: {value}"` — custom format
- `--template '{"file": "{file}", "line": {line}}'` — custom JSON structure per match

Predefined views are named shortcuts for common templates:
```
--view value   ≡  --template "{value}"
--view gcc     ≡  --template "{file}:{line}:{col}: {reason}"
--view source  ≡  --template "{source}"
```

### How they combine

`--report` and `--view` are independent. `--report` controls what sections appear; `--view` controls how matches render within those sections.

```
--report=auto --view=gcc      →  summary + gcc-style match list (check default)
--report=auto --view=lines    →  summary + source lines per violation
--report=matches --view=value →  just values, no summary
--report=schema               →  just schema, --view is irrelevant
--report=count                →  just a number, --view is irrelevant
```

### The selector mental model

Conceptually, all selection controls are XPath-like selectors on the report tree. The implementation uses fixed known values for performance, but the mental model is consistent:

```
--report=matches              →  /report/matches
--report=summary              →  /report/summary
--report=schema               →  /report/schema
--report=count                →  /report/summary/total
--view xml                    →  for each match: ./xml
--view value                  →  for each match: ./value
--template "{file}:{line}"    →  for each match: custom projection
```

This extends to custom templates too — `{value}`, `{line}`, `{file}` are field selectors on the match node.

---

## Serialization (`--format`)

`--format` controls how the report is serialized to stdout. It is orthogonal to what's in the report.

### `--format text` (default)

Human-readable plain text. Rendering depends on the command and `--view`:
- **Query**: renders each match using the selected view (xml, value, source, etc.)
- **Check**: gcc-style violation lines + summary footer
- **Test**: pass/fail line with indented match detail on failure

### `--format json`

Machine-parseable. The selected report section(s) become a JSON object. `--report` still controls what's included — `--format json --report summary` gives you just the summary as JSON. When `--report auto`, the default sections for the command are emitted.

### `--format github`

GitHub Actions workflow commands. Match-level annotations only — no report envelope.

---

## Examples

### Query examples

```bash
# Default: show AST fragments (--view xml, --report matches)
tractor "src/**/*.cs" -x "//function"

# Show just the matched text content
tractor "src/**/*.cs" -x "//function/name" --view value

# Show source lines in context
tractor "src/**/*.cs" -x "//function" --view lines

# Show structural overview (report-level selection)
tractor "src/**/*.cs" -x "//class" --report schema

# Just the count
tractor "src/**/*.cs" -x "//function" --report count

# Full report as JSON (all fields included)
tractor "src/**/*.cs" -x "//function" --format json

# Custom template per match
tractor "src/**/*.cs" -x "//function/name" --template "{file}:{line}: {value}"
```

### Check examples

**Plain text (default):**
```bash
tractor check "src/**/*.cs" -x "//comment[contains(.,'TODO')]" --reason "TODO should be resolved"
```
```
src/Foo.cs:12:5: error: TODO should be resolved
src/Foo.cs:47:5: error: TODO should be resolved
src/Bar.cs:3:1: error: TODO should be resolved

3 errors in 2 files
```

**JSON:**
```bash
tractor check "src/**/*.cs" -x "//comment[contains(.,'TODO')]" --reason "TODO should be resolved" --format json
```
```json
{
  "summary": {"passed": false, "total": 3, "files": 2, "errors": 3, "warnings": 0},
  "matches": [
    {"file": "src/Foo.cs", "line": 12, "column": 5, "value": "// TODO fix", "reason": "TODO should be resolved", "severity": "error"},
    {"file": "src/Foo.cs", "line": 47, "column": 5, "value": "// TODO cleanup", "reason": "TODO should be resolved", "severity": "error"},
    {"file": "src/Bar.cs", "line": 3, "column": 1, "value": "// TODO", "reason": "TODO should be resolved", "severity": "error"}
  ]
}
```

**GitHub annotations:**
```bash
tractor check "src/**/*.cs" -x "//comment[contains(.,'TODO')]" --reason "TODO should be resolved" --format github
```
```
::error file=src/Foo.cs,line=12,col=5::TODO should be resolved
::error file=src/Foo.cs,line=47,col=5::TODO should be resolved
::error file=src/Bar.cs,line=3,col=1::TODO should be resolved
```

### Test examples

**Plain text (default):**
```bash
tractor test "src/**/*.cs" -x "//comment[contains(.,'TODO')]" --expect none
```
```
✗ expected none, got 3
  src/Foo.cs:12:5: // TODO fix
  src/Foo.cs:47:5: // TODO cleanup
  src/Bar.cs:3:1: // TODO
```

**JSON:**
```bash
tractor test "src/**/*.cs" -x "//class" --expect some --format json
```
```json
{
  "summary": {"passed": true, "expected": "some", "total": 5, "files": 3},
  "matches": [
    {"file": "src/Foo.cs", "line": 1, "column": 1, "value": "Foo"},
    ...
  ]
}
```

### Multi-rule check example

**Plain text (default, flat):**
```
src/Foo.cs:12:5: error[no-todos]: TODO should be resolved
src/Bar.cs:3:1: error[no-todos]: TODO should be resolved
src/Foo.cs:47:5: warning[require-docs]: Missing XML doc comment

2 errors, 1 warning in 2 files
```

**JSON:**
```json
{
  "summary": {"passed": false, "total": 3, "files": 2, "errors": 2, "warnings": 1},
  "matches": [
    {"file": "src/Foo.cs", "line": 12, "column": 5, "value": "// TODO fix", "rule_id": "no-todos", "reason": "TODO should be resolved", "severity": "error"},
    {"file": "src/Bar.cs", "line": 3, "column": 1, "value": "// TODO", "rule_id": "no-todos", "reason": "TODO should be resolved", "severity": "error"},
    {"file": "src/Foo.cs", "line": 47, "column": 5, "value": "Bar()", "rule_id": "require-docs", "reason": "Missing XML doc comment", "severity": "warning"}
  ]
}
```

For **inline check** (ad-hoc `tractor check -x ... --reason ...`), there's no `rule_id` — it's a one-off check, not a named rule.

Grouping by file or by rule is a rendering option, not a structural change. The data is the same flat list either way.

---

## Resolved Decisions

- **Subcommands**: `tractor` (query, default), `tractor check` (lint), `tractor test` (assertion), `tractor set` (mutation).
- **Report**: The full output is called a "report" — a structured data tree.
- **Three orthogonal output concerns**:
  - `--format / -f` — serialization (how the report is encoded): `text`, `json`, `github`
  - `--report` — report-level selection (what sections to include): `auto`, `matches`, `summary`, `schema`, `count`
  - `--view` / `--template` — match-level rendering (how each match is displayed): `xml`, `value`, `source`, `lines`, `gcc`, or custom template
- **Reason** (`--reason`): Per-match violation text in check mode.
- **Schema**: An element in the report tree, derived from matches. Query only — not available in check/test.
- **Count**: A report-level projection (`--report count` → `summary.total`). Not a format.
- **Multi-rule reports**: Flat match list with `rule_id` per match (only when using `--rules`). Summary breaks down by severity. Grouping by file/rule is a rendering option.
- **Inline check**: No `rule_id` for ad-hoc checks.
- **Summary description**: Derived from context (rule name, xpath). No CLI flag needed.

## Open Questions

1. **Serialization targets**: `--format` values: `text`, `json`, `github`. Are there others needed? (SARIF? XML?)

2. **JSON match fields**: When matches are included in JSON output, are all match fields always present (xml, source, lines, value), or can `--view` filter which fields appear?

3. **Severity and exit codes**: Should `--severity warning` mean exit 0 on violations? Or should exit behavior be a separate flag?

4. **Per-file grouping**: Plain-text rendering option. What controls it? (flag? `--report` variant?)

5. **Template syntax**: Custom `--template` uses `{value}`, `{line}`, `{file}`. Should these stay as curly-brace placeholders, or align more with XPath?

6. **View defaults per command**: Query defaults to `--view xml`, check defaults to `--view gcc`. What does test default to?

7. **`--view` for check/test**: Can you override the match-level view in check (e.g., `--view lines` to see violating code in context)? Or is check rendering fixed?
