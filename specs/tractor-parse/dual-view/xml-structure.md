---
title: XML Structure
priority: 1
---

Dual-view languages produce two sibling branches under `<File>`:

```xml
<Files>
  <File path="config.json">
    <syntax> ...lossless AST... </syntax>
    <data> ...query-friendly projection... </data>
  </File>
</Files>
```

Both branches are derived from the same TreeSitter parse tree. The syntax
branch is transformed in place; the data branch is built from a clone of the
raw tree with a separate transform applied.

Single-document files have their content directly under `<syntax>` and `<data>`.
Multi-document YAML files wrap each document in a `<document>` element under
both branches.
