---
title: Transformation Rules
type: group
priority: 1
---

Systematic rules for transforming TreeSitter syntax tree to semantic tree.

The transformation is applied recursively to each node. Rules are evaluated
in order for each node type, producing a cleaner, more queryable XML structure.
