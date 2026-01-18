---
title: Visual Query Builder
priority: 1
---

Interactive visual tool for building XPath queries by clicking on AST nodes in a tree view. Users select nodes, set targets, and add conditions to construct queries without writing XPath syntax directly.

## Key Concepts

- **Path-based selection**: Nodes are identified by their structural path (e.g., `class/method/body`), not unique IDs. Selecting a path selects ALL matching nodes in the tree.
- **Target node**: The node that will be returned by the query. Can be explicit (user-selected) or auto-detected via LCA algorithm.
- **Conditions**: XPath predicates added to nodes to filter matches (e.g., text content matching).
- **Query persistence**: Selections persist across source code edits since paths are structural.

## User Interaction Flow

1. User clicks a node pill to select/deselect it
2. Selected nodes appear in the generated XPath query
3. Right-click opens context menu for target setting
4. Clicking text content opens condition menu
5. Query updates in real-time as selections change
