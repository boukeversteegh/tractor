# Explore path ignores -f (biggest functional gap)

## Problem

`tractor file.js -f json` outputs colored XML tree instead of JSON.

`explore_files` and `explore_inline` in `matcher.rs` completely ignore `ctx.output_format`.
They render the AST tree directly via `render_node` regardless of what `-f` flag was given.
The explore path (no `-x` XPath) was never wired into the report/format system.

## Reproduction

```sh
tractor src/main.rs              # works: shows XML tree
tractor src/main.rs -f json      # broken: still shows XML tree (colored)
tractor src/main.rs -f yaml      # broken: still shows XML tree
```

## Why this matters

The explore path is the primary "what does the AST look like?" workflow. Being able to
get `-f json` output is useful for piping to `jq`, scripting, CI, etc.

## Fix

Route the explore path through the report model, the same way query does:

1. Build a `Report` from the explored files (one `ReportMatch` per file, where the
   `xml_fragment` is the full document tree).
2. Call `render_query_report` (or the appropriate format renderer) with the report.

The explore path currently builds a `Match` with `xml_fragment = full document XML`
and `value = text content`. That match can be wrapped in a `ReportMatch` and put in a
`Report` just like a query result.

For the tree view (`-v tree -f text`), the current `render_node` output should be
preserved as-is — the report wrapper just adds routing.

## Files

- `tractor/src/pipeline/matcher.rs`: `explore_files`, `explore_inline`
- After fixing todo/1 (render dispatch): call `render_query_report` from both

## Priority

High. This is a functional bug visible to any user who tries `-f json` without `-x`.
