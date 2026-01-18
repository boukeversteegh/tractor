---
title: Target Detection
priority: 1
---

Algorithm for determining which node is the query target (the node returned by the XPath query).

## Target Types

- **Explicit target**: User manually set via context menu
- **Auto-detected target**: Computed using LCA (Lowest Common Ancestor) algorithm

## Target Resolution Order

1. If any node has `isTarget: true`, use that node
2. If only one node selected, it becomes the target
3. If multiple nodes selected, apply LCA algorithm
