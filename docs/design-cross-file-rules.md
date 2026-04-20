# Design: Cross-File Rules

How to extend tractor so that rules can span multiple files while keeping
XPath as the only query language and letting users compose their own
patterns.

---

## Problem

Tractor processes each file in isolation. A file is parsed into XML,
XPath rules run against that single document, and the AST is dropped.
This means you cannot write rules about relationships between files.

### Examples of what's impossible today

1. **Layer violations** — "files in `src/domain/` must not import from
   `src/infrastructure/`." You can find imports in a single domain file,
   but you can't express the architectural constraint as a rule that
   covers all domain files at once and knows about the project layout.

2. **File correspondence** — "every `*_migration.cs` must have a
   matching `*_migration.designer.cs`." Requires knowing what files
   exist outside the file being checked.

3. **Interface/implementation matching** — "every interface in
   `src/interfaces/I*.ts` must have a class in `src/impl/` whose name
   matches." Requires cross-referencing declarations across file sets.

4. **Cross-file counting** — "a function that is called from more than
   20 files should be flagged at its definition site." Requires
   aggregating call sites across all files and reporting at the
   definition.

5. **Dead exports** — "a symbol that is exported but never imported by
   any other file." Requires comparing export declarations against
   import declarations project-wide.

6. **Naming/structure consistency** — "every file in `src/handlers/`
   must export a function whose name matches the filename." Requires
   relating file-system metadata to AST content.

All of these are **project-level conventions** — the kind of hard-won
lessons that tractor's mission ("Write a rule once. Enforce it
everywhere.") is designed to capture.

---

## Design Principles

1. **XPath is the only query language.** Cross-file rules use the same
   XPath 3.1 that single-file rules use. No new DSL for aggregation,
   filtering, or assertion. XPath 3.1 already has `count()`,
   `distinct-values()`, `for`/`let`/`return`, `some`/`every`,
   `contains()`, `matches()`, `tokenize()` — enough to express
   aggregation and cross-referencing.

2. **Users write their own rules.** Tractor provides the *mechanism*
   (a queryable document spanning multiple files) but not *policy*
   (no built-in "circular dependency detector" or "fan-out checker").
   Users should never have to wait for tractor to implement a pattern.
   Even if it's clunky — a multi-pass pipeline where one query tags
   things and the next gathers them — that's fine. The user is in
   control.

3. **Multi-pass composition is a first-class concept.** Some cross-file
   patterns are too complex for a single query. Users should be able
   to chain operations: extract facts in pass 1, then query those
   facts in pass 2. Each pass produces XML; the next pass queries it.

4. **No built-in graph algorithms.** Architectural enforcement is
   important, but tractor should not ship built-in `circular-dependency`,
   `fan-out`, `layer-violation` pattern matchers. Those are things users
   can build themselves from the primitives tractor provides.

5. **Memory-conscious.** Loading full ASTs for thousands of files into
   one document is impractical. The design must offer ways to limit
   what's loaded (scoped file sets, skeleton extraction).

---

## Core Mechanism: The Project Document

### Concept

A new operation type — working name `cross-check` — builds a single
XML document (the "project document") by merging ASTs from multiple
files, then runs XPath rules against it.

```xml
<project>
  <file path="src/domain/user.ts" language="typescript">
    <import>
      <source>../../infrastructure/db</source>
    </import>
    <class>
      <name>User</name>
      <method><name>save</name><public/></method>
    </class>
    <export><name>User</name></export>
  </file>
  <file path="src/infrastructure/db.ts" language="typescript">
    <class>
      <name>Database</name>
      ...
    </class>
  </file>
  ...
</project>
```

Every file's AST is wrapped in a `<file>` element with `path` and
`language` attributes. XPath rules run against this merged tree with
full cross-file visibility.

### Configuration

```yaml
cross-check:
  files: ["src/**/*.ts"]
  rules:
    - id: domain-no-infra
      xpath: >-
        //file[contains(@path, '/domain/')]/import
          [contains(.//source, 'infrastructure')]
      reason: "Domain layer must not import from infrastructure"
      severity: error
```

This is structurally identical to a normal `check:` — same rule
properties (`id`, `xpath`, `reason`, `severity`), same YAML shape.
The only difference is the *document* the XPath runs against: a merged
project document instead of a single file.

### XPath examples for each use case

**Layer violation:**
```xpath
(: Find imports in domain/ that reference infrastructure/ :)
//file[contains(@path, '/domain/')]/import
  [contains(.//source, 'infrastructure')]
```

**File correspondence:**
```xpath
(: Find migration files that lack a designer file :)
for $f in //file[matches(@path, '_migration\.cs$')]
let $designer := replace($f/@path, '_migration\.cs$', '_migration.designer.cs')
where not(//file[@path = $designer])
return $f
```

**Interface/implementation matching:**
```xpath
(: Interfaces with no matching implementation class :)
for $iface in //file[contains(@path, '/interfaces/')]/interface/name
let $impl-name := substring-after($iface, 'I')
where not(//file[contains(@path, '/impl/')]/class[name = $impl-name])
return $iface
```

**Cross-file counting:**
```xpath
(: Functions called from more than 20 distinct files :)
for $fn in //file//function/name
let $callers := //file[.//call[name = $fn]]
where count($callers) > 20
return $fn
```

**Dead exports:**
```xpath
(: Exports that no other file imports :)
for $exp in //file/export/name
let $exporter := $exp/ancestor::file/@path
where not(//file[@path != $exporter]//import[contains(.//source, $exp)])
return $exp
```

**Name/structure consistency:**
```xpath
(: Handler files that don't export a matching function :)
for $f in //file[contains(@path, '/handlers/')]
let $expected := replace(
  replace($f/@path, '.*/([^/]+)\.ts$', '$1'), '-', '_'
)
where not($f/export/function[name = $expected])
return $f
```

All pure XPath 3.1. No new syntax. AI can write these.

---

## Skeleton Extraction

### The memory problem

A project with 5,000 TypeScript files might produce 50 million XML
nodes if full ASTs are loaded. This is too much for a single in-memory
document.

### Solution: extract only what's needed

An `extract` option runs an XPath query per-file to select which
subtrees to include in the project document. Only matching subtrees
are merged; everything else is discarded.

```yaml
cross-check:
  files: ["src/**/*.ts"]
  extract: "//import | //class | //function | //export | //interface"
  rules:
    - id: domain-no-infra
      xpath: "..."
```

With `extract`, each `<file>` element contains only imports, classes,
functions, exports, and interfaces — not statement bodies, expressions,
or variable assignments. This could reduce memory by 10-50x.

If `extract` is omitted, full ASTs are loaded (suitable for small
scopes).

### Safety limits

```yaml
cross-check:
  files: ["src/**/*.ts"]
  max-files: 500              # Abort if glob matches more than this
  extract: "//import | //class"
  rules: [...]
```

This prevents accidental loading of huge file sets.

---

## Pipeline Composition (Multi-Pass)

### Motivation

Some patterns require multiple steps. For example:

1. Tag every class with its file path and visibility
2. Count how many files reference each class
3. Flag classes that are public but referenced from only one file

XPath 3.1 can do this in a single query on a project document, but
for very large projects the query becomes impractical (O(n^2)). A
pipeline lets each step produce a smaller intermediate result.

### Mechanism

Operations in a config file can carry an `id`. A subsequent operation
can reference a previous operation's results by that `id` instead of
reading from source files.

```yaml
operations:
  # Pass 1: extract all class definitions with their file paths
  - query:
      id: class-defs
      files: ["src/**/*.ts"]
      queries:
        - xpath: "//class/name"
      # Output: report XML with matches, each having file, line, value

  # Pass 2: extract all class usages (references)
  - query:
      id: class-refs
      files: ["src/**/*.ts"]
      queries:
        - xpath: "//call/name | //type/name"

  # Pass 3: cross-reference definitions against usages
  - cross-check:
      # Instead of `files:`, reference previous results
      sources: [class-defs, class-refs]
      rules:
        - id: unused-class
          xpath: >-
            //source[@id='class-defs']/match[
              not(value = //source[@id='class-refs']/match/value)
            ]
          reason: "Class {value} is defined but never referenced"
```

Each pass's output is a standard tractor report (XML). The
`cross-check` with `sources` merges referenced reports into a
queryable document:

```xml
<project>
  <source id="class-defs">
    <match file="src/models/User.ts" line="5" value="User"/>
    <match file="src/models/Admin.ts" line="3" value="Admin"/>
  </source>
  <source id="class-refs">
    <match file="src/handlers/login.ts" line="12" value="User"/>
  </source>
</project>
```

The XPath in pass 3 runs against this merged report. No new DSL —
just XPath on XML.

### Alternative: inline pipeline

For simpler cases, a single `cross-check` could have an inline
extraction step:

```yaml
cross-check:
  files: ["src/**/*.ts"]
  extract: "//import | //class | //interface"
  rules:
    - id: interface-has-impl
      xpath: >-
        for $iface in //file[contains(@path, 'interfaces')]/interface/name
        let $impl := substring-after($iface, 'I')
        where not(//file[contains(@path, 'impl')]/class[name = $impl])
        return $iface
      reason: "Interface {.} has no implementation"
```

Here `extract` does the skeleton extraction (pass 1) and the rule
XPath does the cross-referencing (pass 2) in a single operation.

---

## How This Fits Into Tractor's Architecture

### Existing pipeline (single-file)

```
files → parse (parallel) → AST → XPath (-x) → matches → report → XPath (-q) → output
         per file             per file           aggregated
```

### Extended pipeline (cross-file)

```
files → parse (parallel) → ASTs → extract (per-file) → merge → project doc → XPath → matches → report
         per file            per file                    once      once
```

### Integration points

- **New operation type**: `CrossCheckOperation` alongside existing
  `QueryOperation`, `CheckOperation`, etc. in `executor.rs`.

- **Config schema**: new `cross-check:` key in `tractor_config.rs`
  with `files`, `extract`, `max-files`, `rules`.

- **Report model**: cross-file violations use the same `ReportMatch`
  struct. The `file` field points to the source file where the
  violation was found (from the `<file path="...">` ancestor in the
  project document).

- **CLI**: possibly `tractor cross-check` subcommand, or just
  `tractor run config.yaml` where the config contains `cross-check`
  operations.

---

## Relationship to Existing Two-Stage Pipeline

Tractor already has a two-stage pipeline: `-x` (per-file XPath) and
`-q` (XPath on the report). The report is already XML.

`cross-check` is conceptually a *different Stage 1* — instead of
running XPath per-file independently, it builds a merged document and
runs XPath once across all files. Stage 2 (`-q`) works the same way
on the resulting report.

This means the existing report model, output formats (gcc, json,
github), and `-q` projections all work unchanged for cross-file rules.

---

## Performance Considerations

### Scoping is critical

Cross-file rules should always be scoped to the smallest relevant
file set. A layer violation rule doesn't need the entire project —
just the domain files and their imports.

### Extraction reduces memory

With `extract: "//import"`, a 1000-file project might produce a
project document with ~10,000 nodes instead of ~10,000,000. This is
the difference between "works on a laptop" and "needs a build server."

### Query complexity

XPath queries on a project document can be O(n^2) if they cross-
reference all files against all files (e.g., "for each function, count
callers across all files"). For these patterns, the multi-pass pipeline
is more efficient: extract in O(n), then cross-reference the smaller
intermediate results.

### Parallelism

The extraction/parsing phase is embarrassingly parallel (same as today).
The merge and XPath evaluation phases are sequential but operate on a
single (hopefully smaller) document.

---

## XPath 3.1 Capabilities Relevant to Cross-File Rules

Functions that make cross-file queries practical:

| Function / Feature | Use |
|-|-|
| `count()` | Count files, references, violations |
| `distinct-values()` | Deduplicate names, paths |
| `for $x in ... return` | Iterate over one set, check against another |
| `let $x := ... return` | Bind intermediate values |
| `some $x in ... satisfies` | Existential check ("at least one file has...") |
| `every $x in ... satisfies` | Universal check ("all files must have...") |
| `contains()`, `starts-with()`, `ends-with()` | String matching on paths/names |
| `matches()` | Regex matching on paths/names |
| `replace()` | Transform strings (e.g., strip prefix/suffix) |
| `tokenize()` | Split paths, parse module names |
| `string-join()` | Build messages with context |

These are all standard XPath 3.1, supported by the xee engine tractor
already uses.

### Potential limitation: `count(distinct-values(...))`

XPath 3.1 has `distinct-values()` which returns a sequence of atomic
values. `count(distinct-values(//file//call/name))` should work for
counting distinct called function names. This needs verification with
xee but is standard XPath 3.1.

---

## What's Deferred

- **Import path resolution.** Matching `import '../domain/user'` to
  `src/domain/user.ts` requires understanding of module resolution
  (tsconfig paths, Python sys.path, etc.). For now, users can use
  `contains()` / `matches()` on raw import strings. Proper resolution
  could be added later — see `design-call-graph-exploration.md` for
  a survey of libraries (notably `github/stack-graphs`) that could
  provide this, and the use cases in this document that would become
  precise rather than heuristic as a result.

- **Incremental/cached indexing.** Re-parsing all files on every run
  is acceptable for CI. Caching parsed ASTs or extracted facts for
  watch-mode is a later optimization.

- **Graph algorithms.** Circular dependency detection, strongly
  connected components, topological sort. Users can approximate these
  with multi-pass XPath pipelines. Dedicated graph support (e.g., via
  petgraph) could be added later if user demand warrants it, but as
  a user-facing query mechanism, not built-in rules.

- **Mega-document CLI.** An interactive `tractor project src/**/*.ts`
  command that lets you run ad-hoc XPath on the project document.
  Useful for exploration but not needed for CI-oriented rules.

---

## Open Questions

1. **Is `extract` XPath or a simpler selector?** XPath is the natural
   choice (consistent), but a simpler syntax like
   `extract: [import, class, function]` (element names) might be more
   ergonomic for the common case. Could support both: bare names as
   shorthand, XPath for complex extraction.

2. **How are violations located?** When a cross-file XPath matches a
   node, the violation's file/line/column comes from the nearest
   `<file>` ancestor in the project document. But what about queries
   that return constructed values (e.g., `for ... return $name`)
   rather than existing nodes? The location might need to be extracted
   differently.

3. **Pipeline data shape.** When one operation's results feed into
   another via `sources`, the intermediate data is a tractor report
   (matches with file, line, value). Is this always the right shape?
   Some pipelines might want richer intermediate data (e.g., full AST
   subtrees, not just text values).

4. **Practical size limits.** How many files can a project document
   handle with skeleton extraction? 500? 5,000? 50,000? This needs
   benchmarking with xee/xot.

5. **Error reporting quality.** When an XPath query on a 1000-file
   project document fails or returns unexpected results, how do you
   debug it? Tractor's existing `tractor render` command (show the
   XML tree) should extend to project documents.

6. **Interaction with `diff-files` / `diff-lines`.** Should cross-file
   rules respect git diff filtering? A layer violation in an unchanged
   file is still a violation — but maybe you only want to report it
   when the file is touched. This is a policy question.

---

## Prior Art

- **CodeQL**: Full relational query language over a project-wide
  database. Very powerful but complex; requires compilation and a
  large toolchain. Tractor aims to be simpler.

- **Semgrep**: Has limited cross-file analysis via taint tracking.
  Rules are per-file; cross-file is a premium feature with a
  different engine.

- **ESLint**: Per-file only. Cross-file analysis requires plugins
  that manage their own state.

- **Architect (ArchUnit / NetArchTest)**: Library-based architectural
  testing. Users write tests in the host language (Java, C#).
  Powerful but requires writing code, not declarative rules.

Tractor's approach (XPath on a merged document) is unique: declarative,
language-agnostic, and user-composable without writing code.
