---
title: Native Text Selection
priority: 2
---

Support for native browser text selection within text content nodes.

## Interaction

1. Click and drag to select text within a text node
2. Release mouse to open contextual condition menu
3. Menu shows only relevant options based on selection position

## Selection Detection

After mouseup, analyze the selection:

- **Full text**: Selection equals complete text content
- **Start selection**: Selection begins at index 0
- **End selection**: Selection ends at last character
- **Middle selection**: Selection is somewhere in between

## Contextual Menu Options

Based on selection position, show only relevant options:

| Selection Type | Available Options |
|----------------|-------------------|
| Full text | Exactly |
| Start only | Contains, Starts with |
| End only | Contains, Ends with |
| Start + End (partial) | Contains, Starts with, Ends with |
| Middle only | Contains |

## Toggle Behavior

- Click text with menu open → closes menu
- Click text with menu closed → opens menu with full text
- Select text → opens menu with selected substring
