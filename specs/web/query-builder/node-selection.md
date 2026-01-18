---
title: Node Selection
priority: 1
---

Mechanism for selecting AST nodes in the tree view to include them in the XPath query.

## Path-Based Keys

Nodes are identified by their structural path rather than unique instance IDs:

- Path format: `class/method/body/return`
- Clicking any node with path `class/method` selects ALL nodes at that path
- Enables pattern-based matching across multiple instances

## Selection State

Each selected path tracks:

- `selected`: Whether the path is included in the query
- `isTarget`: Whether this is the explicit query target
- `condition`: Optional XPath predicate for filtering

## Interactions

- **Left-click** on node pill: Toggle selection on/off
- **Right-click** on node pill: Open context menu
- Selection persists across source code changes (paths remain valid)
- Selection clears when language changes (AST structure differs)
