---
title: Type Element Nesting
priority: 1
---

Types (return types, parameter types, variable types) are wrapped in a `<type>` element.

This provides consistent structure for both simple and complex types:

```xml
<!-- Simple type -->
<param>
  <name>count</name>
  <type><name>int</name></type>
</param>

<!-- Generic type -->
<param>
  <name>items</name>
  <type>
    <generic>
      <name>List</name>
      <type><name>string</name></type>
    </generic>
  </type>
</param>

<!-- Method return type -->
<method>
  <name>GetItems</name>
  <type>
    <generic>
      <name>List</name>
      <type><name>T</name></type>
    </generic>
  </type>
</method>
```

XPath queries:
- `//method[type/name='void']` - find void methods
- `//method[type/generic/name='List']` - find methods returning List<T>
- `//param[type/name='string']` - find string parameters

Nested generics are supported:
```xml
<type>
  <generic>
    <name>Dictionary</name>
    <type><name>string</name></type>
    <type>
      <generic>
        <name>List</name>
        <type><name>int</name></type>
      </generic>
    </type>
  </generic>
</type>
```
