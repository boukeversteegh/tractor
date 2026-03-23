# Batch Execution Architecture

Design and implementation plan for batch execution — running multiple rules/commands
from configuration files with efficient single-parse-per-file processing.

---

## Motivation

Tractor's original purpose is to serve as a linter / static analyzer that runs
multiple checks on a codebase and returns a unified report. Today each CLI
invocation runs a single command. Running 20 rules means 20 invocations, each
re-parsing every file. Batch execution solves this by:

- Parsing each file exactly once, applying all applicable rules against it
- Producing a single unified report across all rules
- Loading rule definitions from configuration files (YAML)

### Design Principles

1. **A rule definition maps 1:1 to a CLI command.** You can experiment ad-hoc
   on the command line, then promote the exact same parameters into a config file.
2. **Any CLI command should be batchable.** Check rules are the primary use case,
   but queries, tests, and mutations should also be expressible in config files.
3. **Configuration files can be spread across directories.** Tractor discovers
   and merges them all, running everything in one batch.
4. **Efficiency by default.** 20 rules on 500 files = 500 parses, not 10,000.

---

## The Core Problem

Today, parsing and querying are **fused** in `query_files_batched()`
(`tractor/src/pipeline/matcher.rs:128`). Each file is parsed, queried, and the
parsed document is immediately dropped. The per-file closure in the `par_iter`
(lines 146–183) does parse → query → collect as one unit.

To support batch execution, parsing must be separated from querying so that a
parsed document can be reused across multiple rules.

---

## Architectural Challenges

### 1. Separate parsing from querying

The foundational change. Currently `query_files_batched` creates a
`DocumentResult` inside a parallel closure and drops it after a single query.
We need:

- **Parse phase**: file → retained parsed document
- **Query phase**: document × xpath → matches

The `XPathEngine` and `xot::Xot` documents must live long enough for multiple
queries to run against them.

**Memory concern**: Retaining all parsed documents simultaneously could blow up
memory on large codebases. Strategy: process files in batches, apply *all* rules
per batch, then drop parsed documents before the next batch.

### 2. File set computation across rules

Different rules target different file globs. We need to:

- Collect all file globs from all rules
- Compute the union of matching files
- For each file, know which subset of rules applies
- Parse the file once, run only the applicable rules against it

This is the "inverted index" approach (option 2 from `specs/rules.md:66-74`):
merge glob patterns for minimal file discovery, then re-match each file against
each rule's glob.

### 3. Configuration file format and loading

A rule in YAML maps 1:1 to a CLI command (per `specs/report-model.md:539-567`):

```yaml
- id: no-todos
  command: check
  files: "src/**/*.cs"
  xpath: "//comment[contains(.,'TODO')]"
  reason: TODO should be resolved
  severity: error

- id: require-docs
  command: check
  files: "src/**/*.cs"
  xpath: "//method[public][not(comment)]"
  reason: Public methods must have documentation
  severity: warning
```

Every CLI flag has a YAML property counterpart:

| CLI flag       | YAML property | Purpose                          |
|----------------|---------------|----------------------------------|
| (subcommand)   | `command`     | Which mode (check, test, query)  |
| (positional)   | `files`       | File glob pattern                |
| `-x`           | `xpath`       | Source query                     |
| `--reason`     | `reason`      | Violation message (check)        |
| `--severity`   | `severity`    | `error` or `warning` (check)    |
| `--expect`     | `expect`      | Expected count (test)            |
| `-m`           | `message`     | Message template                 |
| (auto)         | `id`          | Rule identifier (config only)    |

### 4. Report aggregation

The report model already has `rule_id: Option<String>` on `ReportMatch`
(`tractor-core/src/report.rs`), designed for this. Challenges:

- Multiple rules produce independent `Vec<Match>` results
- These must merge into a single `Report` with a unified `Summary`
- Summary aggregates errors/warnings across all rules
- Matches sortable by file (all violations for one file together) or by rule

### 5. Splitting global vs per-rule context

`RunContext` currently holds everything. In batch mode, some settings are global
while others are per-rule:

| Global (shared)                        | Per-rule                    |
|----------------------------------------|-----------------------------|
| concurrency, color, verbose            | xpath expression            |
| tree_mode, parse_depth                 | files glob                  |
| output_format                          | reason, severity            |
| limit, depth                           | message template            |
| ignore_whitespace                      | expect (test)               |

We need to factor `RunContext` into a `BatchContext` (global settings) and
per-rule parameters (`RuleContext`).

### 6. Command heterogeneity in batch mode

A single config file could mix check rules, test assertions, queries, and
mutations. Questions:

- Does a single batch report combine check violations and test failures?
- Exit code semantics? (any check error → exit 1? any test failure → exit 1?)
- Can set/update (write) operations coexist with read-only operations?

Pragmatic approach: **start with batch check only**, then generalize.

---

## What's Already in Place

- `ReportMatch.rule_id` — ready for multi-rule tagging
- `Report::check()` with `Summary` — needs aggregation across rules
- `report.with_groups()` — file grouping already works
- Exponential batching infrastructure in `matcher.rs`
- All output formats (gcc, github, json, yaml, xml) — just render the merged report
- The specs (`specs/rules.md`, `specs/report-model.md`) define the design direction

---

## Implementation Plan

### Step 1: Factor parsing out of the query pipeline

Separate `query_files_batched` into parse and query phases so that a parsed
document can be reused across multiple XPath queries.

- [ ] Create `ParsedDocument` struct that retains the parsed xot document,
      source lines, doc handle, and file path
- [ ] Create `parse_file()` function that returns `ParsedDocument`
- [ ] Create `query_parsed_document()` function that takes a `ParsedDocument`
      and XPath expression, returns `Vec<Match>`
- [ ] Refactor `query_files_batched()` to use the new two-phase functions
      (no behavior change — existing tests must pass)
- [ ] Verify that `XPathEngine` can run multiple queries against the same
      document without issues

### Step 2: Define the rule/config file schema

Create the data model for rule definitions and a YAML deserializer.

- [ ] Define `RuleDefinition` struct mirroring CLI parameters:
      `id`, `command`, `files`, `xpath`, `reason`, `severity`, `message`,
      `expect`
- [ ] Define `RuleSet` struct (a collection of rules from one file)
- [ ] Implement YAML deserialization (serde_yaml)
- [ ] Add validation: required fields per command type (e.g., check requires
      xpath; test requires expect)
- [ ] Add config file discovery: `--rules` flag with glob pattern
      (e.g., `--rules '**/tractor.yml'`)

### Step 3: Build the multi-rule query loop

The core batch algorithm that parses each file once and applies all relevant
rules.

- [ ] Implement file set computation: collect all rule globs → union of files →
      per-file rule applicability map
- [ ] Implement the batch-process loop:
      ```
      for each file batch (parallel):
        for each file:
          parse file → document
          for each applicable rule:
            query document → tag matches with rule_id
          drop document
      ```
- [ ] Collect all matches across all rules
- [ ] Sort matches by (file, line, column) for unified output
- [ ] Build aggregated `Summary` (total errors, warnings across all rules)
- [ ] Build unified `Report` with `rule_id` on each match

### Step 4: Wire up CLI entry point

Connect batch execution to the CLI.

- [ ] Add `--rules` flag to the top-level CLI (or as a new subcommand)
- [ ] Load config files from the `--rules` glob pattern
- [ ] Build `BatchContext` from global CLI flags
- [ ] Route to batch execution when `--rules` is present
- [ ] Ensure single inline commands (`tractor check -x ...`) still work
      unchanged

### Step 5: Split RunContext into global and per-rule parts

Factor the context so batch mode and single-command mode share code cleanly.

- [ ] Extract `BatchContext` struct with global settings (concurrency, color,
      format, tree_mode, etc.)
- [ ] Extract `RuleContext` struct with per-rule settings (xpath, reason,
      severity, message, files)
- [ ] Make existing `RunContext` composable from `BatchContext` + `RuleContext`
- [ ] Ensure backward compatibility: single-command mode constructs `RunContext`
      as before

### Step 6: Output and reporting for batch mode

Ensure all output formats render multi-rule reports correctly.

- [ ] GCC format: include `rule_id` in output (e.g.,
      `file:line:col: error[no-todos]: reason`)
- [ ] GitHub format: include `rule_id` in annotation title
- [ ] JSON/YAML/XML: include `rule_id` field per match
- [ ] Text format: render multi-rule output with rule context
- [ ] Summary line: "N errors, M warnings from K rules in F files"
- [ ] Support `--group file` (all violations per file) and `--group rule`
      (all violations per rule)

### Future Steps (out of scope for now)

- [ ] Batch mode for non-check commands (query, test, set, update)
- [ ] Pipeline support: output of one rule feeds into the next
- [ ] Rule file discovery conventions (auto-find `tractor.yml` in repo root,
      `.tractor/` directories, etc.)
- [ ] `tractor show <rule-id>` command for rule documentation
- [ ] Rule files as Markdown with frontmatter
- [ ] Per-rule valid/invalid examples for regression testing
- [ ] SARIF output format for IDE integration
- [ ] LSP integration for real-time linting

---

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Memory pressure from retaining parsed documents | Batch-then-drop: process files in batches, apply all rules per batch, drop documents before next batch |
| XPath engine can't reuse parsed documents | Verify early in Step 1; if not, clone or re-parse (still saves TreeSitter work) |
| Bad XPath in one rule aborts entire batch | Per-rule error collection; report which rules failed, continue with the rest |
| Thread pool initialized multiple times | Initialize once in batch context, not per-rule |
| Config file schema changes break users | Version the schema; keep it minimal and additive |

---

## Algorithm: Batch Execution Core Loop

```
BATCH-EXECUTE(rules, global_config):
  1. file_map = {}                          // file → [applicable rules]
     for rule in rules:
       for file in expand_glob(rule.files):
         file_map[file].append(rule)

  2. all_files = sorted(file_map.keys())
     batches = exponential_batches(all_files, global_config.concurrency)

  3. all_matches = []
     for batch in batches:
       batch_matches = parallel_for file in batch:
         doc = parse_file(file, global_config)
         file_matches = []
         for rule in file_map[file]:
           matches = query_document(doc, rule.xpath)
           for m in matches:
             m.rule_id = rule.id
             m.reason = rule.reason
             m.severity = rule.severity
           file_matches.extend(matches)
         drop(doc)
         return file_matches
       all_matches.extend(flatten(batch_matches))

  4. all_matches.sort_by(file, line, column)

  5. summary = aggregate_summary(all_matches)
     report = Report::check(all_matches, summary)
     render(report, global_config.format)
```
