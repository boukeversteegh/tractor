---
priority: 3
type: note
---

# Rule System

Build a native rule system into tractor so users can define XPath-based
linting rules in YAML files and run them with a single command.

## Background

There is a working proof-of-concept using Taskfile as a wrapper
(`d:\hippocampus\developer-tools\tractor`). Each rule is a separate
tractor command in the Taskfile. This works but has limitations:

- Each rule runs as a separate tractor process (performance overhead)
- Results aren't aggregated per-file
- Output format isn't unified or consistent
- Requires Taskfile as an external wrapper

## Invocation

Users should be able to run something like:

```sh
tractor
tractor --rules '**/tractor.yml'
```

## Rule File Format

Probably YAML. Each rule should be a thin wrapper around tractor CLI
parameters - the fields should map directly to CLI flags. This means
you can experiment writing rules directly in the shell without setting
up a rule file, and don't need to convert conceptually.

A single tractor command should give the same output as running tractor
with a rulefile containing one rule.

<!-- TODO: define exact schema. Look at the experimental YAML files
     in the hippocampus repo for the current shape. -->

### Rules as Markdown (idea)

Each rule could also have a markdown file where frontmatter defines
the tractor parameters, and the body contains the description and
positive/negative examples. This way each convention is clear and
can be pointed to by teammates.

### Valid/Invalid Examples

Each rule has valid and invalid code examples that serve two purposes:

1. **Documentation** - show what the rule catches and what it allows
2. **Regression testing** - when tractor is updated and parse trees
   change, users can discover if their queries broke

A query that would never match anything is a common mistake. Examples
provide a sanity check against this.

## File Discovery & Filtering

Not all rules apply to all files. This implies a file discovery and
filtering mechanism. Options considered:

1. List all files, apply rules based on each rule's glob pattern
2. Merge glob patterns for minimal file discovery, then re-match each
   file with each rule's glob to decide if it applies
3. Run rules separately and merge results

For performance and usability, results should be reported per-file
(all violations for a file grouped together), following the convention
of established linters.

## Output

### Design Principles

- Output should be fully under the user's control
- When XML or JSON format is chosen, the full response must be
  machine-parseable: no plaintext summaries, messages or statistics
  mixed in. Output should be a single XML/JSON object
- Don't output anything for rules that simply passed (unless providing
  a summary of checked rules)
- The violation message shown per violation should be configurable

### Formats

Must support multiple reporting formats for tooling integration:

- gcc format (for IDE integration)
- JSON
- XML
- GitHub annotations (for CI)
- Possibly LSP-compatible output

### Current CLI Issues

The current flags (`-o`, `--message`, `--error`) are not clear about
which level of output they control. `--expect` also changes output
under some circumstances. This needs to be cleaned up so the output
structure is consistent and predictable.

<!-- TODO: design the output flag system properly -->

## Rule Descriptions & Developer Help

Tractor should be extremely helpful to developers when a violation is
detected. Options:

- Output full rule descriptions with each violation (can be verbose)
- Output descriptions in a summary at the end
- Provide a `tractor show <rule>` command that displays the rule with
  its description and positive/negative examples
- Some combination of the above

<!-- TODO: decide on the approach -->

## CLI Parity

The rule file should be a thin wrapper around CLI parameters. A user
should be able to run a single tractor command that gives the same
output as running tractor with a rulefile containing one rule.

For valid/invalid examples, this may require multiline input:
```sh
--valid="$(cat <<'EOF'
...code...
EOF
)"
```
This is acceptable since these commands will commonly be generated
by AI assistants.

## Open Questions

- Exact rule file schema (YAML vs markdown with frontmatter vs both)
- How to handle rules that apply to different languages/file types
- Output flag redesign (`-o`, `--message`, `--error`, `--expect`)
- How verbose should violation output be by default
- `tractor show` command design
- Performance strategy for multi-rule evaluation on large codebases
- Whether to support rule severity levels (error/warning/info)
