---
title: Tree Navigation
priority: 2
---

Navigation and display controls for the AST tree view.

## Expand/Collapse

- Nodes with children show expand/collapse button (▼/▶)
- Leaf nodes show placeholder for alignment
- Default: Nodes at depth < 3 are auto-expanded

## Source-to-Tree Navigation

Clicking in the source editor:

1. Finds the deepest AST node at cursor position
2. Expands all ancestor nodes to reveal it
3. Scrolls the node into view
4. Highlights the node as focused
5. Switches to Builder tab if on XML tab

## Expansion State

- Managed via `expandedNodeIds` Set
- Uses unique node IDs (not paths) since expansion is instance-specific
- Preserved during source edits
