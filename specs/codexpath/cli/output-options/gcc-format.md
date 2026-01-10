---
title: GCC Format
priority: 1
---

Output in GCC-style error format: file:line:col: error: message

Includes source code snippets:
- Single line: shows line with caret underline
- 2-6 lines: shows all with line markers (>)
- 7+ lines: shows first 2, ellipsis, last 2

Ideal for CI integration and IDE error navigation.
