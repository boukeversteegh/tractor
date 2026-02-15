---
title: Syntax Branch
type: group
priority: 1
---

The `<syntax>` branch contains a lossless AST with a normalized vocabulary
shared across JSON and YAML. It preserves the full syntactic structure needed
for precise source tracing and structural queries.

The syntax branch uses the same semantic transform approach as programming
languages: TreeSitter node types are renamed to a unified vocabulary.
