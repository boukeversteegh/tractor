---
title: Scalar Values
priority: 1
---

Scalars become text content on their containing element.

- **Strings**: Decoded text with escape sequences resolved. JSON `"hello\nworld"`
  becomes a text node containing an actual newline. No quotes in the XML content.
- **Numbers**: Literal text (`30`, `3.14`)
- **Booleans**: Literal text (`true`, `false`)
- **Null**: Literal text `null`

All scalar types are represented uniformly as text content without type markers.
The data branch optimizes for value extraction, not type introspection.

### Escape decoding

JSON escape sequences (`\n`, `\t`, `\"`, `\\`, `\uXXXX`) are fully decoded.
YAML double-quoted escapes and single-quoted `''` sequences are also decoded.
The data branch contains the actual character values, not the source encoding.
