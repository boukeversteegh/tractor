---
title: Value-Oriented Source Spans
priority: 1
---

All data branch nodes carry `start` and `end` attributes in `row:col` format
(1-indexed), enabling `-o source` to extract original source text.

Data node spans point to the **value** portion of a property, not the whole
key-value pair. This reflects the data branch's purpose: extracting values.

### Span targets

| Data node | Span points to |
|-----------|---------------|
| `//data/user/name` | The value `"John"` (including quotes in JSON) |
| `//data/user` | The entire object `{...}` |
| `//data/tags` (array item) | The individual array element |
| `<data>` root | The entire source document |

### Example

For JSON `{"name": "John"}`, the `<name>` element's span covers `"John"`
(columns 10-16), not `"name": "John"` (the whole pair).

This means `-o source` on `//data/name` returns `"John"` — the raw source
representation of the value, including any format-specific syntax like quotes.
