---
title: Supported Languages
priority: 1
---

Dual-view output is produced for data-structure languages where the content
has a natural key-value or document structure suited to direct XPath navigation.

Currently supported:
- **JSON** (`json`)
- **YAML** (`yaml`, `yml`)

Other languages (TypeScript, Python, C#, etc.) produce a single `<syntax>`
branch using their existing semantic transform. The dual-view system is
extensible to new data-structure formats (TOML, INI, ENV are candidates).
