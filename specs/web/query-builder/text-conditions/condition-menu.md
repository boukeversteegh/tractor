---
title: Text Condition Menu
priority: 2
---

Dedicated popup menu for adding text-based conditions to nodes.

## Menu Structure

- **Header**: Shows the text being matched (truncated to 20 chars)
- **Buttons**: Contextual matching options based on selection

## Appearance

- Positioned near the text content element
- Uses same styling as node context menu
- Closes on blur or after selecting an option

## Special Characters

Single quotes in text are escaped as `''` for XPath compatibility:

```
Text: "It's working"
Condition: .='It''s working'
```
