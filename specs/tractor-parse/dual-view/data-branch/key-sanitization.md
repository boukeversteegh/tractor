---
title: Key Name Sanitization
priority: 1
---

JSON/YAML keys may contain characters invalid in XML element names. Keys are
sanitized to produce valid element names:

- Valid XML name characters (`a-z`, `A-Z`, `0-9`, `_`, `-`, `.`) are preserved
- Invalid characters are replaced with `_`
- If the first character is not a letter or `_`, a `_` prefix is added

Examples:

| Original key     | Element name      |
|------------------|-------------------|
| `name`           | `name`            |
| `first name`     | `first_name`      |
| `123`            | `_123`            |
| `foo-bar`        | `foo-bar`         |
| `a:b`            | `a_b`             |

### Original key preservation

When a key requires sanitization, the original key text is stored in a `key`
attribute on the element:

```xml
<first_name key="first name">John</first_name>
```

This allows querying by original key: `//*[@key='first name']`.

Keys that don't need sanitization have no `key` attribute.
