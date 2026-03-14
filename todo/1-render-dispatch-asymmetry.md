# Render dispatch asymmetry: query vs check

## Problem

`render_check_report` lives in `tractor/src/pipeline/format/mod.rs` (the format layer),
but the equivalent dispatch for query — `render_query_output` — is a private function
inside `tractor/src/modes/query.rs` (the mode layer).

Both functions do the same structural thing: match on `ctx.output_format` and call the
right renderer. Having them in different layers is inconsistent and makes it harder to
see the full picture of what formats are supported.

## Fix

Move `render_query_output` to `format/mod.rs` as `render_query_report(report, ctx)`,
mirroring `render_check_report`. `query.rs` builds the report and calls it.

## Priority

Low. Purely structural, no behavior impact.
