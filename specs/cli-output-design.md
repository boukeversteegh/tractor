---
priority: 2
type: note
---

# CLI Output Design

Design decisions for tractor's output system: how commands produce data,
how users select what to see, and how output is serialized.

Supersedes parts of `report-model.md` (which still holds for the report
structure itself, pipeline model, and `-x` query semantics).

## The Core Challenge

Tractor's output must serve very different consumers. A developer
exploring code in a terminal wants to see matched XML fragments,
nothing more. A CI pipeline wants machine-parseable JSON with a
pass/fail verdict. A GitHub Actions workflow wants annotation syntax.
A downstream script piping tractor's output into another tool needs
valid, self-contained XML or JSON.

These aren't just formatting differences. They require different
**data**: the CI pipeline needs a summary the explorer doesn't. The
downstream script needs file paths preserved in the structure so they
survive piping. The GitHub workflow needs severity and reason fields
that don't exist in exploration mode.

The question isn't "how do we format output?" — it's "what is the
underlying data structure that all these consumers are viewing?"

### Challenge 1: Cross-file piping loses context

When you query across multiple files and pipe results into another
tool, the file context disappears:

```
tractor "src/**/*.cs" -x "//class" | another-tool
```

Each matched `<class>` element arrives without any indication of which
file it came from. Worse, if tractor outputs multiple XML fragments,
the result isn't valid XML — there's no single root element. And
multiple JSON objects without an array wrapper break standard parsers
like `jq`.

This means tractor needs some kind of **envelope** — a wrapper that
makes output valid XML/JSON and preserves metadata like file paths.

### Challenge 2: Different consumers need different parts

Consider a check that finds TODO comments across a codebase. Different
consumers want very different things from the same underlying results:

- **Developer in terminal**: just show me `file:line: message` per match
- **CI bot**: give me a JSON object with pass/fail, total count, and
  all matches with locations
- **GitHub Actions**: give me `::error file=...,line=...::message` per
  match, nothing else
- **Downstream script**: give me structured data with the matched source
  tree per match, grouped by file, so I can do further processing

All of these come from the same query results. The challenge is: how do
you let users control what they see without building separate code paths
for every combination?

### Challenge 3: Reports need both structure and a verdict

Check and test modes produce a **verdict** (pass/fail) alongside
matches. The verdict and the matches must be part of the same output
structure. You can't have a text summary printed to stderr and JSON
matches on stdout — that forces consumers to parse two streams.

The output must be a single self-contained structure that includes
everything: verdict, match count, and the matches themselves.

### Challenge 4: Messages belong to the data, not the renderer

When a check rule has a message like "TODO should be resolved", that
message needs to be available in every output format. A JSON consumer
shouldn't have to reconstruct the message from a template — it should
read the final interpolated string directly from the data. This matters
because message templates will eventually live in external rule files,
and the consumer shouldn't need access to those files.

This means the message isn't a rendering concern — it's a **field on
the match**, computed at report construction time, available in JSON,
XML, and text alike.

### Challenge 5: Grouping is structural

CI tools and PR annotators need results grouped by file — all violations
for `src/Foo.cs` together, then `src/Bar.cs`, etc. But this isn't just
a display choice. It changes the shape of the data:

- Flat: each match carries its own `file` field
- Grouped: `file` moves to the group, matches within a group don't
  repeat it

If text output groups by file but JSON output is flat, they're
representing different data structures. Every format should render the
**same** structure — text is just a human-readable projection of the
same shape as JSON and XML. This keeps rendering logic simple and
unified.

### Challenge 6: What is gcc?

The old `-o` flag had values like `value`, `gcc`, `json`, and `schema`.
These look like they're the same kind of thing, but they're not:

- `value` is a **field** on a match — it selects which part of the
  match data to show
- `gcc` is a **rendering format** — it takes multiple fields (file,
  line, column, severity, message) and combines them into a text
  template: `file:line:col: severity: message`
- `json` is a **serialization format** — it controls how the entire
  report is encoded
- `schema` is a **report section** — it replaces the normal output
  with a structural overview

These are three different operations mixed into one flag. The challenge
is figuring out the right decomposition — and it's not obvious, because
gcc and value both feel like "what to show" at first glance.

One way to think about gcc: it's essentially a string template
`{file}:{line}:{col}: {severity}: {message}`. You could implement it
as a shorthand for `-m` with that template. But `-m` produces a
**data field** — the interpolated message becomes part of the match,
visible in JSON and XML. gcc output is not data — nobody wants a
`"text": "src/Foo.cs:12:5: error: TODO found"` field in their JSON.
It's purely a rendering of existing fields.

This reveals the distinction between two kinds of templates:
- **`-m` is a message template** — it computes a `message` field that
  becomes part of the report data. The message is data.
- **`-f` is a rendering template** — it controls how match fields are
  serialized to stdout. gcc is a predefined rendering template. `-f`
  could later support custom rendering templates too, but these are
  a display concern, not data.

So gcc belongs in `-f`. It's a rendering format — like `text` and
`json`, but specialized for compiler-style output. The test: "does
this produce data that should appear in every output format, or does
it control how data is displayed?" Message templates produce data.
Rendering templates control display. gcc is the latter.

---

## The Design

The challenges above led to a decomposition into three orthogonal
parameters, a mandatory report envelope, and structural file grouping.

## 1. The Three-Parameter Decomposition

The old `-o` flag is replaced by three orthogonal parameters, each
controlling a different stage of the output pipeline:

| Parameter | Purpose | Controls |
|-----------|---------|----------|
| `-v` / `--view` | Field selection | Which fields appear on each match in the output |
| `-m` / `--message` | Computed field | Adds an interpolated `message` field to each match |
| `-f` / `--format` | Serialization | How the report is rendered to stdout |

These are fully orthogonal. Any `-v` works with any `-f`. `-m` adds data
regardless of format.

### `-v` / `--view`: Field Selection

`-v` controls which fields are included in the output for each match.
Values are match-level fields or report-level sections:

**Match fields:**
- `tree` — parsed source tree (the XML/JSON structure of the matched code)
- `value` — text content of the matched node
- `source` — exact matched source text
- `lines` — full source lines containing the match
- `file`, `line`, `column` — location fields
- `reason`, `severity` — check-mode annotations

**Report sections:**
- `summary` — the report summary (total, passed, files, etc.)
- `count` — total match count (shorthand for just the number)
- `schema` — structural overview of element types (query mode)

Values are composable via comma separation:

```
-v tree,summary      → matches with tree fragments + summary section
-v value,source      → matches with both value and source fields
-v summary           → just the summary, no matches
```

Each command has a **default set of fields** that are included unless
the user explicitly overrides with `-v`. Explicit `-v` replaces the
defaults entirely — if you say `-v value`, you get just the value,
no reason or severity. You opted out.

Default fields per command:
- **query**: `tree`
- **check**: `reason`, `severity` (plus location fields)
- **test**: `summary`

### `-m` / `--message`: Computed Message Field

`-m` defines a template that tractor interpolates and stores as a
`message` field on each match. This is **data, not rendering**.

```
-m "TODO should be resolved: {value}"
```

Produces a `message` field in every output format:

**JSON:**
```json
{
  "file": "src/Foo.cs",
  "line": 12,
  "value": "// TODO fix",
  "message": "TODO should be resolved: // TODO fix"
}
```

**XML:**
```xml
<match file="src/Foo.cs" line="12">
  <value>// TODO fix</value>
  <message>TODO should be resolved: // TODO fix</message>
</match>
```

**Text:**
```
TODO should be resolved: // TODO fix
```

The key insight: JSON/XML consumers read the `message` field directly.
They don't need to know the template or reconstruct the message
themselves. This matters because message templates will eventually be
stored in external rule files — the consumer must be able to get the
final message out of the response.

Template placeholders: `{file}`, `{line}`, `{col}`, `{value}`,
`{reason}`, `{severity}`.

#### Relationship to `--reason`

`--reason` and `-m` are independent. `--reason` sets the `reason`
field on each match — it's the rule's explanation of *what's wrong*.
`-m` computes a `message` field from a template that can reference
any field, including `{reason}`.

Without `-m`, there's no `message` field. The `reason` field stands
on its own. Renderers like `-f gcc` use `reason` directly in their
output template — they don't need a `message` field to exist.

This separation matters for multi-rule mode: each rule defines its
own `reason` (the *why*), but how reasons are rendered is controlled
by the output format, not the rule. Rules shouldn't decide how
file paths, line numbers, and severity prefixes are formatted —
that's the renderer's job.

### `-f` / `--format`: Serialization Format

`-f` controls how the report is serialized to stdout. It is purely a
rendering concern — it doesn't change what data is in the report.

| Value | Output |
|-------|--------|
| `text` | Human-readable plain text |
| `json` | JSON report envelope |
| `xml` | XML report envelope |
| `gcc` | One line per match: `file:line:col: severity: reason` |
| `github` | GitHub Actions annotation: `::error file=...,line=...::reason` |

`gcc` and `github` are specialized text rendering formats for tooling
integration. They pull fields directly from the match (`file`, `line`,
`column`, `severity`, `reason`) and lay them out in a fixed template.
They don't need a `message` field — they access the raw fields.

Like `-v`, `-f` has per-command defaults:
- **query**: `-f text`
- **check**: `-f gcc`
- **test**: `-f text`

This means `tractor check ... --reason "TODO found"` without any `-v`
or `-f` just works — it uses the check defaults (`-v reason,severity`
feeding into `-f gcc`) and renders:

```
src/Foo.cs:12:5: error: TODO found
```

Previously gcc and github were `-v` values, which created a category
error: they aren't fields on a match, they're rendering templates.
Moving them to `-f` resolves this. See Challenge 6 for the reasoning.

`-f` could later support custom rendering templates (e.g.
`-f "{file} [{severity}] {reason}"`), but these would be rendering
templates — distinct from `-m` which produces data.

---

## 2. The Report Envelope

Every tractor command produces a **report**. By default, or with
`-p report`, tractor emits the full report envelope. With any other
`-p` value, tractor emits the selected projection instead of the full
envelope.

### Why the default is an envelope

Without an envelope, query mode outputs bare fragments:

```xml
<function><name>main</name>...</function>
<function><name>helper</name>...</function>
```

This is not valid XML (no single root element). Similarly, bare JSON
objects without an array break standard parsers. The envelope keeps the
default output reliably parseable.

### Default report shape

**Query mode** (no verdict; summary omitted unless requested by the view):
```xml
<report>
  <results>
    <match file="src/Foo.cs" line="5" column="1">
      <tree><function><name>main</name>...</function></tree>
    </match>
  </results>
</report>
```

**Check/test/set mode** (summary container present):
```xml
<report>
  <summary>
    <success>false</success>
    <totals>
      <results>3</results>
      <files>2</files>
      <errors>3</errors>
    </totals>
  </summary>
  <results>
    <match file="src/Foo.cs" line="12" column="5">
      <value>// TODO fix</value>
      <reason>TODO found</reason>
      <severity>error</severity>
    </match>
  </results>
</report>
```

The same structure in JSON:

```json
{
  "summary": {
    "success": false,
    "totals": {
      "results": 3,
      "files": 2,
      "errors": 3
    }
  },
  "results": [
    {
      "file": "src/Foo.cs",
      "line": 12,
      "column": 5,
      "value": "// TODO fix",
      "reason": "TODO found",
      "severity": "error"
    }
  ]
}
```

### Projection shape

When `-p` is present and not `report`, the selected projection determines
the top-level shape:

- `-p results` emits the results list directly.
- `-p tree`, `value`, `source`, and `lines` emit sequences in JSON/YAML,
  `<results>`-wrapped sequences in XML, and newline-separated items in text.
- `-p summary`, `totals`, and `schema` emit those singular sections directly.
- `-p count` emits a bare scalar in text/JSON/YAML and a synthetic
  `<count>` root in XML.

### Contracts

- Parseability: XML output is always a single rooted document, JSON is
  always valid JSON, and YAML is always valid YAML.
- Content-independence: the output shape is determined by flags alone,
  not by whether the query returns 0, 1, or many results.

---


## 3. File Grouping

### The problem

Multi-file results need grouping by file for CI tools, PR annotations,
and human readability. But grouping is structural — it changes the data
shape, not just rendering.

### Design: groups as a layer on top of matches

Groups wrap matches without changing match contents. The group key
attribute names the grouping dimension:

```xml
<report>
  <groups>
    <group file="src/Foo.cs">
      <match line="12" column="5">
        <value>// TODO fix</value>
        <reason>TODO found</reason>
        <severity>error</severity>
      </match>
      <match line="47" column="5">
        <value>// TODO cleanup</value>
        <reason>TODO found</reason>
        <severity>error</severity>
      </match>
    </group>
    <group file="src/Bar.cs">
      <match line="3" column="1">
        <value>// TODO</value>
        <reason>TODO found</reason>
        <severity>error</severity>
      </match>
    </group>
  </groups>
</report>
```

In JSON:

```json
{
  "groups": [
    {
      "file": "src/Foo.cs",
      "matches": [
        { "line": 12, "column": 5, "value": "// TODO fix", "reason": "TODO found" },
        { "line": 47, "column": 5, "value": "// TODO cleanup", "reason": "TODO found" }
      ]
    },
    {
      "file": "src/Bar.cs",
      "matches": [
        { "line": 3, "column": 1, "value": "// TODO", "reason": "TODO found" }
      ]
    }
  ]
}
```

### Key properties

- **Matches are self-contained.** A match has the same fields whether
  it's in a group or flat. The `file` field moves to the group when
  grouped (since it's redundant), but the match object itself is
  unchanged.

- **Groups are a layer, not nesting.** One level of grouping. The group
  element is a thin wrapper that adds structure without modifying match
  contents.

- **The group key attribute is self-describing.** `<group file="...">` vs
  `<group rule-id="...">` makes the grouping dimension explicit. Future
  rule grouping uses the same pattern.

- **Text output renders the same structure.** Plain text output is a
  human-readable projection of the grouped data — file headers when the
  group changes, indented matches underneath. Not separate grouping logic.

### Flat vs grouped

- **Query mode default**: flat (`<matches>` with `file` on each match)
- **Check/test mode**: grouped by file (`<groups>` with `file` on group)
- Could add `--group-by file` flag to control explicitly

The choice between `<matches>` (flat) and `<groups>` (grouped) is
structural — the top-level element name tells the consumer which shape
to expect.

---

## 4. CLI Help Organization

Options are grouped by pipeline stage, ordered by importance:

| Group | Options | Purpose |
|-------|---------|---------|
| *(default)* | `-l`, `-s`, `-h` | Input (language, string source, help) |
| **Extract** | `-x`, `--raw`, `-W` | What to query and how |
| **View** | `-v`, `-m`, `-n`, `-d`, `--keep-locations` | What to include in output |
| **Format** | `-f`, `--no-pretty`, `--color`, `--no-color` | How to serialize |
| **Advanced** | `--parse-depth`, `-c`, `--verbose`, `--debug`, `-V` | Rarely used |
| **Check** | `--reason`, `--severity` | Check-specific (only on `check`) |
| **Test** | `-e`/`--expect`, `--error`, `--warning` | Test-specific (only on `test`) |
| **Set** | `--value` | Set-specific (only on `set`) |

Input options (`-l`, `-s`) are ungrouped so they appear first alongside
`-h` in the default Options section. This keeps the most common first
interaction (specifying files and language) at the top.

The `query` subcommand is now explicitly listed under Commands, making
it visible that tractor's primary mode is querying source code.

### `--raw` placement

`--raw` is in Extract, not Input or Advanced. It switches the tree
structure from semantic to raw tree-sitter, which changes what `-x`
queries against. It's a pipeline concern, not a display option.

---

## 5. View Name: "tree" not "ast"

The default view value is `tree`, not `ast`.

**Reasoning:**
- "AST" (Abstract Syntax Tree) is implementation jargon. The vision doc
  emphasizes low barrier to entry and AI-friendliness.
- What tractor shows is the *opposite* of syntax — it's the abstract
  structure with concrete syntax (braces, keywords, semicolons) removed.
  "Syntax" would be misleading.
- The vision doc uses "XML tree" to describe tractor's output.
- `tree` is short, neutral, and descriptive of what users see.

The internal codebase still uses "AST" in code comments, variable names,
and type names. Only user-facing text (help, error messages, view names)
avoids the term.

`ast` is accepted as an alias in the view parser for backward
compatibility, but is not shown in help text.

Help description: `tree — Parsed source tree (XML or JSON, depending on -f)`

---

## 6. Resolved Questions from Other Specs

### From report-model.md

| Open question | Resolution |
|---------------|------------|
| #1 Serialization targets | `-f` values: text, json, xml, gcc, github |
| #2 XML→text/JSON mapping | Same fields in every format; `-v` selects, `-f` renders |
| #3 Per-file grouping | `<groups>` wrapper; flat default for query, grouped for check/test |
| #6 Custom templates | `-m` is a computed `message` field, not a rendering concern |

### From rules.md

| Open question | Resolution |
|---------------|------------|
| Output flag redesign | Three orthogonal params: `-v`, `-m`, `-f` |
| Severity levels | Supported via `--severity` (error/warning) on check mode |

### From usecase-csharp-to-typescript-codegen.md

| Issue | Resolution |
|-------|------------|
| #4 File context lost in pipes | Report envelope preserves file paths in groups/matches |
| #5 Per-property class context | `-v tree` preserves full matched subtree per match |

---

## 7. Multi-Rule Check: Design for `--rules`

### Distance from current state

The single-rule inline `tractor check` already builds a Report with
summary, matches, reason, severity, and rule_id fields. The report
model supports everything multi-rule needs structurally. What's missing
is the orchestration layer.

### What exists

- Report model with `Summary`, `ReportMatch`, `Severity`, `rule_id`
- JSON envelope output (`-f json` produces `{summary, matches}`)
- `--reason`, `--severity` per check
- Clean pipeline: Input → Extract → View → Format
- gcc/github formatters
- View constants (no magic strings)

### What's needed

1. **Rule file parsing.** YAML schema definition, deserialize rule
   definitions. The schema should map 1:1 to CLI flags (see rules.md
   "CLI Parity" section). A single `tractor check` invocation should
   produce identical output to a rulefile containing one rule.

2. **`--rules` flag on check.** Glob pattern for rule files:
   ```
   tractor check --rules '**/tractor.yml'
   ```

3. **Multi-rule execution loop.** Iterate rules, run each query, merge
   matches into one Report with `rule_id` per match. Each rule brings
   its own `files` glob, `xpath`, `reason`, `severity`.

4. **File discovery optimization.** Multiple rules may target overlapping
   file sets. Options:
   - Parse each file once, evaluate all applicable rules against it
   - Merge glob patterns for minimal file I/O
   - Run rules independently and merge results (simplest, possibly
     good enough given tractor's parse caching)

5. **Per-file grouping in output.** All violations for a file grouped
   together, following the convention of established linters (eslint,
   clippy). This is the grouped report structure from section 3.

### Rule file schema (draft)

```yaml
rules:
  - id: no-todos
    files: "src/**/*.cs"
    xpath: "//comment[contains(.,'TODO')]"
    reason: "TODO should be resolved"
    severity: error

  - id: require-docs
    files: "src/**/*.cs"
    xpath: "//method[public][not(preceding-sibling::comment)]"
    reason: "Public methods should have documentation"
    severity: warning
```

Each rule maps directly to a `tractor check` invocation:
```
tractor check "src/**/*.cs" -x "//comment[contains(.,'TODO')]" \
  --reason "TODO should be resolved" --severity error
```

### Multi-rule report structure

```xml
<report>
  <summary>
    <passed>false</passed>
    <total>5</total>
    <files>3</files>
    <errors>3</errors>
    <warnings>2</warnings>
  </summary>
  <groups>
    <group file="src/Foo.cs">
      <match line="12" column="5">
        <value>// TODO fix</value>
        <reason>TODO should be resolved</reason>
        <severity>error</severity>
        <rule-id>no-todos</rule-id>
      </match>
      <match line="47" column="1">
        <value>public void Bar()</value>
        <reason>Public methods should have documentation</reason>
        <severity>warning</severity>
        <rule-id>require-docs</rule-id>
      </match>
    </group>
    <group file="src/Bar.cs">
      <match line="3" column="1">
        <value>// TODO</value>
        <reason>TODO should be resolved</reason>
        <severity>error</severity>
        <rule-id>no-todos</rule-id>
      </match>
    </group>
  </groups>
</report>
```

In text output (gcc format):
```
src/Foo.cs:12:5: error[no-todos]: TODO should be resolved
src/Foo.cs:47:1: warning[require-docs]: Public methods should have documentation
src/Bar.cs:3:1: error[no-todos]: TODO should be resolved

3 errors, 2 warnings in 2 files
```

### Later: rule grouping

When users want to see results organized by rule rather than by file,
the same `<group>` mechanism applies:

```xml
<group rule-id="no-todos">
  <group file="src/Foo.cs">
    <match line="12" column="5">...</match>
  </group>
  <group file="src/Bar.cs">
    <match line="3" column="1">...</match>
  </group>
</group>
```

This is nested grouping — deferred until there's a concrete use case
for rule-first organization.

---

## 8. Migration Path: Current → Full Design

### Current state (implemented)

| Feature | Status |
|---------|--------|
| `-x` / `--extract` | Done. XPath query on source. |
| `-v` / `--view` | Done, single value only. Values: tree, value, source, lines, gcc, github, count, schema, summary. |
| `-f` / `--format` | Done. Values: text, json. |
| `-m` / `--message` | Done. But currently only affects text rendering, not stored as field. |
| Report envelope | Done for `-f json` (check/test). Query mode streams for text, wraps for json. |
| File grouping | Not implemented. Matches are flat. |
| CLI help groups | Done. Input → Extract → View → Format → Advanced → command-specific. |
| View constants | Done. `view::TREE`, `view::GCC`, etc. |
| `tree` view name | Done. `ast` accepted as alias. |

### Step 1: `-m` as computed field

Current `-m` only affects text rendering. Change it to interpolate the
template at report construction time and store the result as a `message`
field on each `ReportMatch`. This makes the message available in JSON
and XML output, not just text.

`--reason` in check mode should also populate the `message` field
(it's `-m` without placeholders).

### Step 2: gcc and github move to `-f`

Currently `-v gcc` and `-v github` are view values that map to
`OutputFormat::Gcc` and `OutputFormat::Github`. They need to become
`-f` values instead.

This is a breaking change. Since tractor is alpha, this is acceptable.
The migration:
- Add `gcc` and `github` as `-f` values
- Remove them from `-v`
- Update check mode's default from `-v gcc` to `-f gcc`
- Update all integration tests and examples

After this, `-v` is purely field selection and `-f` is purely
serialization. No overlap.

### Step 3: File grouping in report

Add `<groups>` structure to the Report. For check/test mode, matches
are grouped by file by default. For query mode, matches stay flat
unless explicitly grouped.

The report data structure needs a `Group` type:
- Has a key (file path, rule id, etc.) and a key type
- Contains a list of matches
- Matches within a group omit the key field (e.g. no `file` on match
  when grouped by file)

Text rendering: iterate groups, print group header, then render matches
within the group. Same rendering logic regardless of format — text is
a projection of the same grouped structure.

### Step 4: `-v` composability

Accept comma-separated values: `-v tree,summary`. Parse into a set of
requested fields. Each field is either a match-level field (included
per match) or a report-level section (included at report level).

This changes the view parsing from returning a single `OutputFormat`
to returning a set of field selections.

### Step 5: Report envelope for query mode

Currently query mode streams results for text output and wraps in a
Report only for `-f json`. Make the envelope always present internally,
even for text — the text renderer just doesn't print the `<report>`
wrapper, but the data flows through the same pipeline.

This unifies the code path: all modes build a Report, all formatters
render a Report.

### Step 6: Multi-rule check

With grouping, message-as-data, and the report envelope in place,
multi-rule is mostly orchestration:
- Parse YAML rule files
- Run each rule's query
- Merge matches into groups by file
- Build summary across all rules
- Render via the same pipeline

---

## 9. How Text Rendering Works

A key insight from the design discussion: **plain text output should be
a render of the data structure, not a separate code path.** This keeps
rendering logic simple and unified across formats.

### The rendering model

For each output format, the renderer receives the same Report data and
walks the same structure:

**`-f json`**: Serialize the Report to JSON directly. Groups become
arrays of objects, fields become properties.

**`-f xml`**: Serialize the Report to XML directly. Groups become
nested elements.

**`-f text`**: Walk the same structure, render each piece as human-
readable text:
- Summary → "3 errors in 2 files"
- Group header → "src/Foo.cs:" (or blank line separator)
- Match fields → depends on `-v`: value prints the value, tree prints
  indented XML, source prints the source text

**`-f gcc`**: Walk matches, render each as `file:line:col: severity: reason`.
Uses match fields directly (`file`, `line`, `column`, `severity`, `reason`).

**`-f github`**: Walk matches, render each as `::error file=...,line=...::reason`.

### Field rendering in text

When `-v` selects multiple fields, text output renders them in order:

```
-v file,line,value → "src/Foo.cs  12  main"
-v value           → "main"
-v tree            → <function><name>main</name>...</function>
```

The exact text layout for multi-field views is TBD — could be
tab-separated, one-per-line, or configurable. Single-field views
are straightforward.

---

## Open Questions (remaining)

1. **Multi-field text layout.** When `-v tree,summary` or `-v value,source`
   selects multiple fields, how are they laid out in `-f text`? Tab-separated
   columns? One field per line? Labeled sections?

2. **`--group-by` flag vs implicit grouping.** Should grouping be
   explicitly requested (`--group-by file`) or implied by the command
   (check always groups)? Or both — command sets a default, flag overrides?

3. **`-v` as XPath on the report.** The report-model.md envisions `-v`
   (originally `-q`) accepting full XPath expressions on the report XML.
   Current implementation uses predefined shorthands only. Full XPath
   is deferred. Comma-separated field names work as the simple syntax;
   XPath union (`value | file`) would be the advanced syntax.

4. **Rule file schema details.** Exact YAML structure, how to handle
   markdown-as-rule (frontmatter with rule params, body with description
   and examples), valid/invalid example format.

5. **`-f xml` for report envelope.** Designed but not yet implemented.

6. **Schema view in grouped reports.** Does `-v schema` make sense when
   results are grouped by file? Schema across all files, or per-file
   schemas within each group?

7. **Custom rendering templates for `-f`.** `-f` could support custom
   templates (e.g. `-f "{file} [{severity}] {reason}"`). These are
   *rendering* templates distinct from `-m` *data* templates. Design
   the syntax and how to distinguish from named formats.
