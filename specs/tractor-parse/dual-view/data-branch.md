---
title: Data Branch
type: group
priority: 1
---

The `<data>` branch projects the source content into query-friendly XML
optimized for intuitive XPath navigation. Object keys become element names,
scalars become text content, and arrays become repeated sibling elements.

The data branch contains decoded values with no trace of the source format's
syntax (no quotes, no escape sequences). Source span attributes on data nodes
point to the value portion of each property, enabling `-o source` to extract
the original source text.
