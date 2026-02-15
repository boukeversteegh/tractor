---
title: Dual-View XML
type: group
---

For data-structure languages (JSON, YAML), tractor produces two branches under
each `<File>` element: a `<syntax>` branch preserving the full AST structure,
and a `<data>` branch projecting the content into query-friendly XML where keys
become element names and values become text content.

This enables two complementary querying styles:
- **Syntax**: `//syntax//property[key/string='name']` — navigate the parse tree
- **Data**: `//data/user/name` — navigate the data like a document

Both branches carry source span attributes, so `-o source` works from either.
Languages without dual-view support (TypeScript, Python, etc.) produce a single
`<syntax>` branch with their existing semantic transform.
