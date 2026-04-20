# Count and Schema views bypass the Report pipeline

**Status**: Resolved by the projection/render-path refactor.

`-v count` and `-v schema` now flow through the normal report builder and
renderer, so they compose with `-f` and the new `-p` projection model. Bare
scalar count output is available explicitly via `-p count`.

## Problem

When `-v count` or `-v schema` is specified, `run_query` short-circuits before building
a `Report`. It prints the result directly to stdout and returns.

```
matches = query_files_batched(...)
if view.has(Count)  → println!("{}", count);        // never enters Report
if view.has(Schema) → print_schema_from_matches();  // never enters Report
else                → build_query_report() → render_query_report()
```

This means count and schema can never be composed with:
- `-f json` / `-f yaml` / `-f xml` — the format flag is silently ignored
- `-v count,summary` — the summary is silently ignored
- Any future structured output that wraps results in an envelope

The user gets plain text regardless of `-f`, which is surprising.

## Why it matters

- **User surprise**: `tractor "src/**/*.rs" -x "//function" -v count -f json` outputs `42`
  (a bare integer), not `{"count": 42}`. There's no way to get a machine-readable count.
- **Composability**: `-v count,tree` is a valid ViewSet parse, but count wins and tree is
  silently dropped. The ViewSet model promises field composition, but these two fields opt
  out of it.

## Recommendation

Make `Count` and `Schema` summary-level fields on the Report rather than bypass paths:

1. `Count`: build a Report with an empty `matches` vec and `summary.total` set. Renderers
   emit `{"total": 42}` (json), `<total>42</total>` (xml), or `42` (text) based on format.
2. `Schema`: either make it a Report field (`schema: Option<SchemaNode>`) or keep it as a
   separate output mode with an explicit `tractor schema` subcommand.

Option 2 for Schema might be cleaner since it's structurally different from a match report.

## Priority

Medium. The count case is a real composability gap. Schema is more niche.
