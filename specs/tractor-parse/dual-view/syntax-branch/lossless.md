---
title: Lossless Structure
priority: 1
---

The syntax branch preserves all structural information from the source:

- Every key and value is an explicit node with its own span
- Whitespace and formatting are reflected in span positions
- String content preserves the original encoding (escape sequences are not decoded)
- All nodes carry `start` and `end` attributes pointing to their source location

This makes the syntax branch the source of truth for precise source tracing
and structural analysis.
