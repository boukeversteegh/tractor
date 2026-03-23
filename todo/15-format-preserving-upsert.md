# Format-preserving upsert

## Goal

When `tractor set` inserts or updates nodes, the result should match the
formatting style of the surrounding source code — not a fixed default style.
This includes arbitrary formatting patterns people actually use:

- Minified / whitespace-free: `{"a":1,"b":2}`
- Compact single-line objects: `{ "a": 1, "b": 2 }`
- 2-space, 4-space, or tab indentation
- Mixed styles: small objects inline, large objects expanded
- Trailing commas, trailing newlines, CRLF vs LF

## Current state

- **Updates already work**: the splice approach replaces only the value bytes,
  so surrounding formatting is preserved exactly.
- **Inserts re-render the parent subtree**, using `detect_render_options` which
  is a global heuristic (first indented line in the file). This breaks for
  minified JSON (expands to multi-line) and doesn't adapt to local context.
- Tests confirm: 2-space, 4-space, tab, CRLF all detected correctly for
  expanded files. Minified insert is a known failure case.

## Desired behavior

Inserted or updated nodes should adopt the formatting conventions visible in
their immediate context — the parent node and its existing children. If the
parent object is written inline, a new property should be inserted inline. If
siblings use 4-space indent, the new node should too. The system should handle
any formatting pattern without needing to enumerate styles upfront.

## Possible approaches

1. **Detect from parent span**: check whether the parent node's source span
   is single-line (render compact) or multi-line (detect indent from children).
   Simple, covers the main cases.

2. **Copy sibling whitespace**: look at the literal whitespace before/after
   existing sibling nodes in the original source and replicate it for the
   new node. Handles arbitrary patterns without the renderer needing to
   understand them.

3. **Hybrid**: use the renderer for structure, but inject whitespace tokens
   sampled from the original source context.
