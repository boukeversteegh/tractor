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

| Parameter        | Stage  | Purpose                          | Values                                          |
|------------------|--------|----------------------------------|-------------------------------------------------|
| `-x`             | source | Query source code ASTs           | XPath expression                                |
| `-q`             | report | Query/project the report         | XPath expression or predefined shorthand (`value`, `count`, `schema`, etc.) |
| `--format / -f`  | output | Serialization                    | `text` (default), `json`, `github`              |
| `--reason`       | match  | Violation text (check)           | string                                          |
| `--expect`       | report | Assertion (test only)            | `none`, `some`, or a number                     |
| `--severity`     | match  | Violation severity (check)       | `error` (default), `warning`                    |

`-x` and `-q` are the same operation (XPath query) at different pipeline stages. `--report` and `--view` are replaced by `-q`.

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

## The Pipeline Model

Tractor is a two-stage XPath pipeline. The same operation (parse → query) happens twice:

```
source files → parse → AST (XML) → -x query → matches → report (XML) → -q query → output
               stage 1: source                  stage 2: report
```

**Stage 1** (`-x`): Query source code ASTs. Produces matches.
**Stage 2** (`-q`): Query the report. Selects and projects output.

Both stages use XPath. The report is an XML document — querying it with `-q` is the same operation as querying source with `-x`. And because the output is XML, you can pipe it back into tractor:

```bash
# Two commands (piping):
tractor **/*.cs -x "//method[public]" --format xml | tractor --lang xml -x "//match/name"

# One command (same result, optimized):
tractor **/*.cs -x "//method[public]" -q "match/name"
```

The `-q` flag is conceptually identical to piping into another tractor — it's faster because tractor can skip computing fields you didn't ask for.

### Report as XML

The report has a concrete XML representation:

```xml
<report>
  <summary>
    <passed>false</passed>
    <total>3</total>
    <files>2</files>
  </summary>
  <matches>
    <match file="src/Foo.cs" line="12" col="5">
      <value>Foo</value>
      <source>public class Foo {</source>
      <ast>
        <class>
          <name>Foo</name>
          <body>...</body>
        </class>
      </ast>
      <reason>TODO should be resolved</reason>
      <severity>error</severity>
    </match>
  </matches>
  <schema>
    ...
  </schema>
</report>
```

### The `<ast>` boundary

The `<ast>` element embeds the matched source AST fragment — it contains language-specific elements (`class`, `function`, `name`, etc.) that could collide with report-level element names.

**Rule**: `-q` treats `<ast>` as a boundary. Descendant queries (`//`) do not descend into `<ast>` unless you explicitly step into it.

```bash
-q value            →  //value (outside ast)       →  match value elements
-q summary/total    →  //summary/total             →  the count
-q ast//name        →  explicitly enters ast        →  AST name elements
-q match/@file      →  //match/@file               →  file attributes
```

Same auto-prefix rule as `-x`: bare name becomes `//name`.

**Implementation options** (in order of preference):

1. **Document boundary** (preferred): Store each match's AST as a separate sub-document. Standard XPath already doesn't cross document boundaries with `//`. Access via `doc(source)` or similar function. Cleanest — uses standard XPath semantics, no rewriting or hooks needed. Depends on xee multi-document support.
   ```bash
   -q value                  # report-level, no ast
   -q "doc(source)//name"    # explicitly enter the AST sub-document
   -q "doc(source)"          # see the full AST content
   ```
   Tradeoff: AST is not inline in raw XML output. User does `-q doc(source)` to see it.

2. **Evaluation hook**: Intercept descendant axis traversal, skip `<ast>` children. AST stays inline in the XML. Needs a way to detect explicit `ast/...` paths. Depends on xee extensibility.

3. **Query rewriting**: Automatically add `[not(ancestor::ast)]` to descendant queries. Fragile with complex XPath expressions.

**Pragmatic note**: Report element names (`match`, `value`, `summary`, `total`, `schema`, `reason`, `severity`) and AST element names (`class`, `function`, `method`, `comment`, `attribute`) are already naturally distinct. Collisions are unlikely in practice, so the boundary is a guarantee rather than a frequent necessity.

### Predefined `-q` shorthands

Common queries get short names for convenience. These are optimized — tractor can skip computing unused fields:

```bash
-q value            ≡  -q "//match/value"          # text content per match
-q source           ≡  -q "//match/source"         # matched source text
-q ast              ≡  -q "//match/ast"            # AST fragments (query default)
-q summary          ≡  -q "//summary"              # just the summary
-q count            ≡  -q "//summary/total"         # just the count
-q schema           ≡  -q "//schema"               # structural tree (query only)
```

Custom queries work too — anything that isn't a predefined name is treated as XPath:

```bash
-q "match/ast//name"                               # AST name elements
-q "match[@file='src/Foo.cs']"                     # matches from one file
-q "summary|match/value"                           # summary + values
```

### The pipe asymmetry

Ideally, `-q A -q B` should equal `-q A/B` — pure composition. But it doesn't, because there's a **report-wrapping transformation** after the first query:

```
source → -q₁ → [wrap into report] → -q₂ → output
              ↑ hidden step
```

The first `-q` (currently `-x`) queries source ASTs and builds a report — injecting `<match>` wrappers, `<summary>`, metadata attributes (`file`, `line`, `col`), etc. This is a structural transformation, not a pure projection.

Subsequent `-q`s are pure projections on the report. So the first query is special: **query + build report**. The rest are just XPath.

This means `-x` being a separate flag may actually be honest — it signals "this is the source query that builds the report." Making it `-q` too would hide the asymmetry. Or, if everything becomes `-q`, the first one is implicitly the source query and the report-building is always implied.

### Map vs reduce ambiguity

`-x` is a **map** operation: it runs per-file, independently. Results are flattened and wrapped into the report. This means:

```bash
-x "count(//method)"       →  per-file count (one number per file)
-q "count(//match)"        →  global count across all files
```

The report-building step is the **reduce** — it aggregates per-file results into one structure. `-q` runs on the aggregated result.

When chaining `-q A -q B`, the semantics are ambiguous:

- **Map**: `-q B` applied to each node returned by A (like `for each result of A: query B`)
- **Reduce**: `-q B` applied to the entire output of A as a collection (like `wrap results of A, then query B`)

In XPath, a query takes a context node and returns a node set. If `-q A` returns 5 nodes, `-q B` needs a context:
- Map: each of the 5 nodes becomes a context → 5 evaluations of B
- Reduce: the 5 nodes are wrapped in a container → 1 evaluation of B on the container

Unix commands each decide for themselves (grep maps, wc reduces, sort operates on the whole stream). There's no universal rule. Tractor needs to be explicit about this.

### Multiple `-q` queries

Design TBD. Key questions:
- Is chaining map or reduce? Or does it depend on the query?
- Should there be explicit syntax for map vs reduce? (e.g., `-q` for reduce, `-q:each` for map?)
- Or is the answer: just use XPath's own `for` expressions and aggregation functions within a single `-q`?

### Parameters (revised)

| Parameter        | Purpose                          | Values                                          |
|------------------|----------------------------------|-------------------------------------------------|
| `-x`             | Stage 1: query source AST       | XPath expression                                |
| `-q`             | Stage 2: query the report        | XPath expression or predefined shorthand         |
| `--format / -f`  | Serialization of output          | `text` (default), `json`, `github`              |
| `--reason`       | Per-match violation text (check) | string                                          |
| `--expect`       | Assertion (test only)            | `none`, `some`, or a number                     |
| `--severity`     | Violation severity (check)       | `error` (default), `warning`                    |

`--report` and `--view` are replaced by `-q`. The report is XML; querying it is just XPath.

---

## Serialization (`--format`)

`--format` controls how the output is serialized. It is orthogonal to what's selected by `-q`.

### `--format text` (default)

Human-readable plain text. Rendering depends on the command and query:
- **Query**: renders selected elements (AST fragments, values, etc.)
- **Check**: gcc-style violation lines + summary footer
- **Test**: pass/fail line with indented match detail on failure

### `--format json`

Machine-parseable. The selected elements become JSON. Each XML element type has a standard JSON mapping (e.g., `<summary>` → `{"passed": false, "total": 3, ...}`).

### `--format github`

GitHub Actions workflow commands. Match-level annotations only.

---

## Examples

### Query examples

```bash
# Default: show AST fragments (-q ast is default for query)
tractor "src/**/*.cs" -x "//function"

# Show just the matched text content
tractor "src/**/*.cs" -x "//function/name" -q value

# Show source lines in context
tractor "src/**/*.cs" -x "//function" -q lines

# Show structural overview
tractor "src/**/*.cs" -x "//class" -q schema

# Just the count
tractor "src/**/*.cs" -x "//function" -q count

# Full report as JSON
tractor "src/**/*.cs" -x "//function" --format json

# Drill into AST within matches
tractor "src/**/*.cs" -x "//class" -q "ast//name"

# Custom XPath on report
tractor "src/**/*.cs" -x "//function" -q "match[@file='src/Foo.cs']/value"
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

**Just the summary as JSON:**
```bash
tractor check "src/**/*.cs" -x "//comment[contains(.,'TODO')]" --reason "TODO" -q summary --format json
```
```json
{"passed": false, "total": 3, "files": 2, "errors": 3, "warnings": 0}
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
- **Report**: The full output is a structured XML tree called a "report".
- **Two-stage pipeline**: `-x` queries source ASTs (stage 1), `-q` queries the report (stage 2). Same XPath at both stages.
- **`-q` replaces `--report` and `--view`**: One XPath-based parameter for all report selection and projection. Predefined shorthands (`value`, `count`, `schema`, etc.) for common queries.
- **`<ast>` boundary**: The `<ast>` element in the report wraps source AST fragments. `-q` does not descend into `<ast>` by default. Explicit `ast//...` to cross the boundary.
- **Serialization** (`--format / -f`): Orthogonal. `text`, `json`, `github`.
- **Reason** (`--reason`): Per-match violation text in check mode.
- **Schema**: An element in the report tree, derived from matches. Query only.
- **Multi-rule reports**: Flat match list with `rule_id` per match (only with `--rules`). Summary by severity.
- **Inline check**: No `rule_id` for ad-hoc checks.
- **Summary labels**: Derived from context. No CLI flag.
- **Composability**: Output can be piped back into tractor. `-q` is an optimization of piping.

## Open Questions

1. **Serialization targets**: `--format` values: `text`, `json`, `github`. Others needed? (SARIF? XML?)

2. **JSON mapping**: Each XML element type needs a standard JSON representation. What are the mapping rules?

3. **Severity and exit codes**: Should `--severity warning` mean exit 0 on violations? Or separate flag?

4. **Per-file grouping**: Rendering option for plain text. What controls it?

5. **Multiple `-q`**: Do multiple `-q` flags chain (pipeline) or union (multiple selections)? Or both with different syntax?

6. **AST boundary implementation**: Document boundary (preferred) vs evaluation hook vs query rewriting. Needs xee library investigation — does it support sub-documents? Custom axis traversal?

7. **Custom templates**: Do we still want `--view "{file}:{line}: {value}"` style templates alongside `-q`? Or is XPath sufficient? Templates are more ergonomic for simple formatting.
