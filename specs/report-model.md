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

| Parameter       | Purpose                              | Values                                    |
|-----------------|--------------------------------------|-------------------------------------------|
| `--format / -f` | Serialization of the report          | `text` (default), `json`, `github`        |
| `--view`        | Projection — what to show            | `xml` (default), `value`, `source`, `lines`, `gcc`, `schema`, `count`, custom template |
| `--reason`      | Per-match violation text (check)     | string                                    |
| `--expect`      | Assertion (test only)                | `none`, `some`, or a number               |
| `--severity`    | Violation severity (check)           | `error` (default), `warning`              |

**No `--description` flag** — summary labels are derived from context (rule name, xpath expression, etc.)

---

## The Report

The full output of any tractor command is a **report**. A report is a structured data object — conceptually a tree (tractor's native data structure). All output controls are **selectors on this tree**.

### Report structure

```
report
  summary (check, test only)
    - passed
    - total
    - files_affected
    - errors, warnings            # severity breakdown (check)
    - expected (test only)
  matches[]
    - file, line, column, end_line, end_column
    - value                       # text content
    - source                      # exact matched source text
    - lines                       # source lines in context
    - xml                         # AST fragment
    - reason (check only)         # violation description
    - severity (check only)       # error/warning
    - rule_id (multi-rule only)   # which rule matched
  schema (query only)             # derived from matches, structural tree
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

```
match
  - file                          # source file path
  - line, column                  # start position
  - end_line, end_column          # end position
  - value                         # text content of matched node
  - source                        # exact matched source text
  - lines                         # source lines in context
  - xml                           # matched AST as XML
```

### Common: Summary (shared by check and test)

Both check and test produce a summary. The shared base:

```
summary
  - passed (bool)                 # did the command succeed?
  - total                         # match count
  - files_affected                # distinct file count
```

Check extends with:
```
  - errors                        # count of error-severity matches
  - warnings                      # count of warning-severity matches
```

Test extends with:
```
  - expected                      # the assertion (none/some/N)
```

---

## Two Levels of Selection (`--view`)

`--view` is the projection mechanism. It operates at two levels on the report tree:

### Report-level views — which section of the report to emit

- Full report (default for `--format json`)
- Just `matches` (default for plain-text query)
- Just `summary`
- Just `summary/total` — i.e., the count
- Just `schema` (query only — derived from match XML fragments)

### Match-level views — which field(s) of each match to render

- `xml` — AST fragment (query default)
- `value` — text content of the matched node
- `source` — exact matched source text
- `lines` — source lines in context
- `gcc` — `{file}:{line}:{column}: {reason}` (check default)
- Custom template — user-defined with field references

These are **projections**, not transformations. The underlying data is the same; you're choosing what to see.

### The selector mental model

Conceptually, all view controls are XPath-like selectors on the report tree. The implementation uses fixed known values for performance, but the mental model is consistent:

```
--view xml        →  for each match: ./xml
--view value      →  for each match: ./value
--view gcc        →  for each match: {file}:{line}:{col}: {reason}
--view schema     →  /report/schema
--view count      →  /report/summary/total
```

This extends to custom templates too — `{value}`, `{line}`, `{file}` are field selectors on the match node.

In JSON (`--format json`), `--view` is irrelevant — the full report with all fields is always emitted.

---

## Serialization (`--format`)

`--format` controls how the report is serialized to stdout. It is orthogonal to what's in the report.

### `--format text` (default)

Human-readable plain text. Rendering depends on the command and `--view`:
- **Query**: renders each match using the selected view (xml, value, source, etc.)
- **Check**: gcc-style violation lines + summary footer
- **Test**: pass/fail line with indented match detail on failure

### `--format json`

Machine-parseable. The full report becomes a JSON object. All fields are included regardless of `--view`.

### `--format github`

GitHub Actions workflow commands. Match-level annotations only — no report envelope.

---

## Examples

### Query examples

```bash
# Default: show AST fragments (--view xml is default)
tractor "src/**/*.cs" -x "//function"

# Show just the matched text content
tractor "src/**/*.cs" -x "//function/name" --view value

# Show source lines in context
tractor "src/**/*.cs" -x "//function" --view lines

# Show structural overview
tractor "src/**/*.cs" -x "//class" --view schema

# Full report as JSON (all fields included)
tractor "src/**/*.cs" -x "//function" --format json
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
- **Serialization** (`--format / -f`): Orthogonal to content. Values: `text`, `json`, `github`.
- **Match views** (`--view`): Separate from serialization. Projections on the report tree at report-level and match-level.
- **Reason** (`--reason`): Per-match violation text in check mode. Distinct from view/format.
- **Schema**: An element in the report tree, derived from matches. Query only — not available in check/test.
- **Count**: A report-level projection (`summary.total`). Not a format.
- **Multi-rule reports**: Flat match list with `rule_id` per match (only when using `--rules`). Summary breaks down by severity. Grouping by file/rule is a rendering option.
- **Inline check**: No `rule_id` for ad-hoc checks.
- **Summary description**: Derived from context (rule name, xpath). No CLI flag needed.

## Open Questions

1. **Serialization targets**: `--format` values: `text`, `json`, `github`. Are there others needed? (SARIF? XML?)

2. **JSON granularity**: Does `--format json` always include all fields (xml, source, lines, value), or can `--view` filter JSON output too?

3. **Severity and exit codes**: Should `--severity warning` mean exit 0 on violations? Or should exit behavior be a separate flag?

4. **Per-file grouping**: Plain-text rendering option. What controls it? (flag? view?)

5. **Template syntax**: Custom `--view` templates use `{value}`, `{line}`, `{file}`. Should these stay as curly-brace placeholders, or align more with XPath?

6. **View defaults per command**: Query defaults to `--view xml`, check defaults to `--view gcc`. Should test have its own default view?
