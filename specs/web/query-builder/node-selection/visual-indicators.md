---
title: Visual Selection Indicators
priority: 2
---

Visual feedback showing selection state of nodes in the tree view.

## Node Pill States

- **Unselected**: Default appearance
- **Selected**: Highlighted background color
- **Target**: Shows target marker icon (▶ for explicit, ▷ for auto-detected)
- **Has condition**: Shows asterisk (*) marker
- **Focused**: Highlighted when source click focuses this node

## CSS Classes

- `.selected` - Node is part of the selection
- `.target` - Node is the effective target
- `.auto-target` - Target was auto-detected (not explicit)
- `.has-condition` - Node has a condition attached
- `.focused` - Node is currently focused from source click
