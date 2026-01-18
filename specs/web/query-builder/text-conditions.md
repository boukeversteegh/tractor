---
title: Text Conditions
priority: 1
---

Add XPath predicates to filter nodes based on their text content.

## Condition Types

- **Exactly**: `.='text'` - Exact match
- **Contains**: `contains(.,'text')` - Substring match
- **Starts with**: `starts-with(.,'text')` - Prefix match
- **Ends with**: `ends-with(.,'text')` - Suffix match

## Text Selection Integration

Conditions can use:
- Full text content (click without selecting)
- Selected substring (native browser text selection)
