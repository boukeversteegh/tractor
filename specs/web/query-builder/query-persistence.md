---
title: Query Persistence
priority: 2
---

Selection state persists across source code changes.

## Why It Works

Path-based selection uses structural paths like `class/method/body` rather than
unique node instance IDs. When source code changes:

- The AST is re-parsed
- Node instances get new IDs
- But structural paths remain valid
- Selection state maps to the same paths

## Persistence Rules

| Change Type | Selection Behavior |
|-------------|-------------------|
| Edit source code | Selection preserved |
| Change language | Selection cleared |
| Clear button | Selection cleared |

## Language Change

When switching languages (e.g., C# to Python), the selection is cleared because:
- AST structure differs completely between languages
- Path like `class_declaration/method_declaration` has no meaning in Python
