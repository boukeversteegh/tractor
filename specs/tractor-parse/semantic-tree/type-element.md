---
title: Type Element Structure
priority: 1
refs:
  - design.md#5-unified-concepts
  - design.md#4-elements-over-attributes
---

All type references use a `<type>` element wrapper. This enables unified queries
for "all types" regardless of whether they are simple, generic, nullable, or array.

## Principle Applied

**Unified Concepts**: A query for all types should be `//type`, not
`//type | //generic | //array | //nullable_type`.

## Structure

### Simple Types

```xml
<parameter>
  <type>int</type>
  <name>count</name>
</parameter>
```

### Nullable Types

Nullable types contain the base type text plus a `<nullable/>` marker element:

```xml
<parameter>
  <type>
    Guid
    <nullable/>
  </type>
  <name>id</name>
</parameter>
```

Query for nullable types: `//type[nullable]`

### Generic Types

Generic types contain a `<generic/>` marker and type arguments:

```xml
<parameter>
  <type>
    <generic/>
    List
    <arguments>
      <type>User</type>
    </arguments>
  </type>
  <name>users</name>
</parameter>
```

Query for generic types: `//type[generic]`
Query for List types: `//type[generic][contains(., 'List')]`

### Array Types

Array types contain an `<array/>` marker and optionally rank information:

```xml
<parameter>
  <type>
    <array/>
    int
  </type>
  <name>numbers</name>
</parameter>
```

Query for array types: `//type[array]`

### Nested Generics

```xml
<!-- Dictionary<string, List<int>> -->
<type>
  <generic/>
  Dictionary
  <arguments>
    <type>string</type>
    <type>
      <generic/>
      List
      <arguments>
        <type>int</type>
      </arguments>
    </type>
  </arguments>
</type>
```

## XPath Examples

| Query | Finds |
|-------|-------|
| `//type` | All types |
| `//type[nullable]` | Nullable types |
| `//type[generic]` | Generic types |
| `//type[array]` | Array types |
| `//type[not(generic or array or nullable)]` | Simple types |
| `//parameter/type` | All parameter types |
| `//method/returns/type[generic]` | Methods returning generic types |
