---
title: Array Representation
priority: 1
---

Arrays are represented as repeated sibling elements sharing the parent key's
element name. This enables natural XPath indexing.

```json
{"tags": ["math", "science"]}
```

```xml
<data>
  <tags>math</tags>
  <tags>science</tags>
</data>
```

Queryable as:
- `//data/tags[1]` returns `math`
- `//data/tags[2]` returns `science`
- `//data/tags` returns both

### Anonymous arrays

When an array appears without a named parent key (top-level array, or array
nested directly inside another array), each item is wrapped in an `<item>`
element:

```json
[{"name": "Alice"}, {"name": "Bob"}]
```

```xml
<data>
  <item><name>Alice</name></item>
  <item><name>Bob</name></item>
</data>
```
