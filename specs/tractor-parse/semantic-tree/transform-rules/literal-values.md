---
title: Literal Values
priority: 2
refs:
  - ../design.md#6-preserve-language-idioms
  - ../element-naming.md
  - ../../dual-view/syntax-branch/vocabulary.md
---

Literal value nodes use **wrapper elements** where the element name is the type
and the text content is the value:

```xml
<bool>true</bool>
<bool>false</bool>
<number>42</number>
<string>hello</string>
<null>null</null>
```

## Decision

**Wrappers** (type wraps value) over **markers** (type flag + sibling text).

Wrapper approach:
```xml
<value><bool>true</bool></value>
```

Marker approach:
```xml
<value><bool/>true</value>
```

Both support the same core queries and both preserve enough information for
a code generator to reconstruct valid source. The wrapper approach is chosen
for consistency with:
- The syntax vocabulary for JSON/YAML (`<value><string>John</string></value>`)
- C# type elements (`<type>int</type>`)
- The existing rename table (`boolean_literal` -> `bool`, `integer_literal` -> `int`)

This is a consistency decision, not a strong technical preference. It may be
revisited if practical query differences justify a change.

## Query comparison

| Query | Wrapper | Marker |
|-------|---------|--------|
| Find boolean values | `//bool` | `//bool` (matches the marker) |
| Find true assignments | `//value/bool[.='true']` | `//value[bool][.='true']` |
| Test value is true | `[value='true']` | `[value='true']` (same, string value passes through) |
| Get value text | `value/bool/text()` | `value/text()` |
| Find all literals by type | `//bool \| //number` | same |

The main practical difference is text extraction: with wrappers, the literal
text is one level deeper (`value/bool`). With markers, it's directly on the
parent (`value`). Both support `[value='true']` predicates identically because
XPath string value traverses child elements.

## Singleton wrapper lifting

Singleton wrappers like `<value>`, `<returns>`, `<body>` typically contain
exactly one semantic child. A post-transform pass adds `field="{element_name}"`
to the first element child of these wrappers, enabling the JSON serializer to
lift the child as a direct property instead of wrapping it in a `children` array.

Before lifting:
```json
"value": { "children": [{ "bool": "true" }] }
```

After lifting:
```json
"value": { "bool": "true" }
```

The default singleton list covers: `value`, `left`, `right`, `condition`,
`consequence`, `alternative`, `returns`, `body`. Each language can override
this list. Children that already have a `field` attribute are not modified.

## Cross-language mapping

| Language | Raw tree-sitter node | Semantic element |
|----------|---------------------|------------------|
| C# | `boolean_literal` | `bool` |
| C# | `integer_literal` | `int` |
| C# | `real_literal` | `number` |
| C# | `null_literal` | `null` |
| TypeScript | `true` | `bool` |
| TypeScript | `false` | `bool` |
| TypeScript | `number` | `number` |
| TypeScript | `null` | `null` |
| Python | `true` | `bool` |
| Python | `false` | `bool` |
| Python | `integer` | `number` |
| Python | `float` | `number` |
| Python | `none` | `null` |
