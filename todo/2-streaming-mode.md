# Streaming mode: restore and clarify

## Background

`query_files_batched` has a `collect: bool` parameter. When `collect=false` it streams
results to stdout as each batch completes, rather than accumulating all matches in memory.
This is an important feature for large repos (5k+ files): the user gets intermediate
output while the query is still running instead of waiting for the full scan to finish.

## Current state

Both callers (`query.rs` and `check.rs`) always pass `collect=true`, so the streaming
path is currently unreachable. The path still compiles (it calls `render_match_text`) but
it was never re-wired after the report model migration.

## What streaming should do

Streaming only makes sense for line-oriented formats (text, gcc) — JSON/XML/YAML need
a complete document to be well-formed. The streaming path should:

1. Check `ctx.output_format`: if it's json/xml/yaml, silently force `collect=true`
   (or error early).
2. For text/gcc: stream batches to stdout as they arrive, same as before.
3. Expose a `--stream` flag (or make it the default for text/gcc on large inputs),
   so users can opt in explicitly.

## Fix sketch

- Add `--stream` flag to `QueryArgs` (or auto-enable when output format is text/gcc).
- Pass `collect = !ctx.stream || ctx.output_format.requires_collection()` to
  `query_files_batched`.
- The streaming branch in `query_files_batched` calls `render_match_text` per batch
  (already there), but should also handle gcc format.

## Priority

Medium. Real UX value for large codebases.
