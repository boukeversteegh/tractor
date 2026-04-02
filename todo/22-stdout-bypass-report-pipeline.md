# `--stdout` should bypass the report pipeline

## Context

Set/update `--stdout` mode currently routes the modified file content
through the report pipeline by attaching it as `output_content` on
grouped `Report` nodes. This requires:

- `compute_set_output` to build a `HashMap<String, String>` of file → content
- `with_file_outputs` to staple content onto file groups after grouping
- `ViewField::Output` as a pseudo-view-field that means "show the whole file"
- `build_set_inline_report` that manually constructs a pre-grouped `Report`,
  bypassing `ReportBuilder` entirely (hardcoded `Totals`, no flat matches)

This conflates two fundamentally different questions:

1. **"What happened?"** — which matches were found, what changed, status
   (the report's job)
2. **"Give me the result"** — the transformed file content on stdout
   (an output mode, not a report concern)

## Proposal

`--stdout` is not a view of the report — it's an alternative output mode,
like `--dry-run` that prints to stdout instead of writing to disk.

- Set/update always produce flat matches with status, location, value
  (identical structure whether `--stdout` or in-place)
- `--stdout` prints the modified source directly, bypassing the report
  pipeline entirely
- Remove `output_content` from `Report`, `with_file_outputs`,
  `ViewField::Output`, `build_set_inline_report`
- `--stdout` becomes orthogonal to `--format` / `-v` — you could
  conceivably combine `--stdout` with `-f json` to get both the
  transformed content on stdout and a report on stderr

## What this simplifies

- No pre-grouped report construction (the inline set hack)
- No `output_content` field threading through report model and renderers
- No special "Output" view field that doesn't behave like other view fields
- Set/update report structure becomes identical to query report structure
  (flat matches, grouped by the standard pipeline)

## Single-file vs multi-file

The bypass only works cleanly for single-file / stdin input — raw content
on stdout, done. Multi-file `--stdout` needs structure: which content
belongs to which file? That's inherently a report concern.

Resolution: `--stdout` is syntactic sugar with two behaviors:

- **Single file / stdin**: print raw transformed content directly, bypass
  the report pipeline entirely
- **Multi-file**: equivalent to `-v file,output` — the report pipeline
  handles it, `output` is a real view field containing the per-file
  transformed content

This means `ViewField::Output` and `output_content` on `Report` are still
needed for the multi-file case, but `build_set_inline_report` and the
pre-grouped hack can be eliminated — single-file stdout doesn't touch
the report at all, and multi-file stdout uses the standard flat-match
→ grouping → rendering pipeline.

## Deeper issue: diagnostics vs artifacts

A set operation produces two fundamentally different kinds of output:

1. **Diagnostics** — what happened: matches, locations, status (updated/unchanged).
   Same structure as query/check. Consumed by humans, CI, linters.
2. **Artifacts** — what was produced: transformed file content.
   Consumed by the next tool in a pipe, or written to disk.

These don't belong in the same data structure. Per-match patches and
full-file output are different things with different consumers. Mixing
them in one flat match list forces every renderer to distinguish "real
match vs file-output carrier." Grouping doesn't naturally solve this
either — you can't hoist a full-file output from a special match node
to the group level without special-case logic.

The report model may need a separate `artifacts` or `outputs` section
alongside `results`:

```yaml
results:       # diagnostics — what happened
  - file: config.json
    line: 5
    status: updated
outputs:       # artifacts — what was produced
  - file: config.json
    content: "{ ... }"
```

This keeps diagnostics clean (same shape as query/check), gives
`--stdout` a clear data source, and avoids the "is this match real or
synthetic" problem.

## Open questions

- Should `--stdout` with `-f json` embed artifacts in the JSON report,
  or should raw content go to stdout and the report to stderr?
- Should multi-file `--stdout` in text format use a delimiter between
  files, or rely on the report structure (file header + content)?
- Is `outputs` part of the Report model, or a separate return value
  from the executor alongside the report?
