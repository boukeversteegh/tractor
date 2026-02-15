---
title: Object Key-to-Element Mapping
priority: 1
---

Each object key becomes an XML element whose name is derived from the key.
The element's content represents the value.

```json
{"user": {"name": "John", "age": 30}}
```

```xml
<data>
  <user>
    <name>John</name>
    <age>30</age>
  </user>
</data>
```

Queryable as: `//data/user/name` returns `John`.

Nested objects produce nested elements, enabling natural path-based navigation
that mirrors the data structure.
