---
title: Glob Pattern Support
priority: 1
---

Expand glob patterns in file arguments:
- `*` matches any characters within a single path component
- `**` matches zero or more path components (recursive)

Unsupported syntax: `?` (single-character wildcard) and `[...]`
(character classes) are rejected at compile time when used in a glob
pattern (i.e. alongside `*`). In a non-glob pattern (no `*`), they
are treated as literal filename characters with a warning.

Example: `"**/*.cs"` matches all C# files recursively
