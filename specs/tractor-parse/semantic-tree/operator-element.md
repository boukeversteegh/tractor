---
title: Operator Element
priority: 2
status: proposed
---

Operators in expressions are represented as `<op>` child elements with semantic
marker children that classify the operator, plus the original token as text.

## Current State

Operators are extracted from anonymous tree-sitter text into `<op>` elements
with the raw token as text content:

```xml
<binary>
  <op>===</op>
  <left>x</left>
  <right>0</right>
</binary>
```

This solves the "anonymous operator" problem (operators are queryable), but
the raw token is the only way to identify operator meaning. Queries like
"find all equality checks" require enumerating tokens:
`//binary[op='==' or op='===']`.

## Proposed Structure

Add semantic marker elements inside `<op>`. The raw token is preserved as
text content of `<op>` (not inside the marker), keeping markers pure
empty elements consistent with the modifier pattern.

```xml
<binary>
  <op><equals><strict/></equals>===</op>
  <left>x</left>
  <right>0</right>
</binary>
```

### Query patterns

Both text matching and semantic matching work:

```xpath
//binary[op='===']                  (exact token match — still works)
//binary[op[equals]]                (any equality: == or ===)
//binary[op[equals[strict]]]        (strict equality only)
//binary[op[compare[or-equal]]]     (>= or <=)
//binary[op[logical[and]]]          (logical and: && or 'and')
```

### Graceful degradation

Operators without a semantic marker fall back to the current behavior — just
`<op>` with text content. This means the taxonomy can be built incrementally:
common operators get markers first, obscure ones later (or never).

```xml
<!-- Semantic marker available -->
<op><plus/>+</op>

<!-- No marker yet — still works, just no semantic query -->
<op>&gt;&gt;&gt;</op>
```

## Operator Taxonomy

Operators grouped by semantic family. Each family is a marker element that may
contain sub-markers for variations.

### Equality

| Token(s)       | Marker                          | Languages          |
|----------------|---------------------------------|--------------------|
| `==`           | `<equals/>`                     | All                |
| `===`          | `<equals><strict/></equals>`    | JS, TS             |
| `!=`           | `<not-equals/>`                 | All                |
| `!==`          | `<not-equals><strict/></not-equals>` | JS, TS        |

### Comparison

| Token(s)       | Marker                                     | Languages |
|----------------|-----------------------------------------------|-----------|
| `<`            | `<compare><less/></compare>`               | All       |
| `>`            | `<compare><greater/></compare>`            | All       |
| `<=`           | `<compare><less/><or-equal/></compare>`    | All       |
| `>=`           | `<compare><greater/><or-equal/></compare>` | All       |

Query: `//binary[op[compare]]` — all comparisons.

### Arithmetic

| Token(s)       | Marker              | Languages              |
|----------------|---------------------|------------------------|
| `+`            | `<plus/>`           | All                    |
| `-`            | `<minus/>`          | All                    |
| `*`            | `<multiply/>`       | All                    |
| `/`            | `<divide/>`         | All                    |
| `%`            | `<modulo/>`         | All                    |
| `**`           | `<power/>`          | JS, TS, Python, Ruby   |

These are flat (no family wrapper) since arithmetic operators rarely need
grouping in queries.

### Logical

| Token(s)          | Marker                          | Languages       |
|-------------------|---------------------------------|-----------------|
| `&&` / `and`      | `<logical><and/></logical>`     | All             |
| `\|\|` / `or`     | `<logical><or/></logical>`      | All             |
| `!` / `not`       | `<logical><not/></logical>`     | All (unary)     |
| `??`              | `<nullish-coalescing/>`         | JS, TS, C#      |

Query: `//binary[op[logical]]` — all logical operations.

### Bitwise

| Token(s)       | Marker                            | Languages |
|----------------|-----------------------------------|-----------|
| `&`            | `<bitwise><and/></bitwise>`       | All       |
| `\|`           | `<bitwise><or/></bitwise>`        | All       |
| `^`            | `<bitwise><xor/></bitwise>`       | All       |
| `~`            | `<bitwise><not/></bitwise>`       | All (unary) |
| `<<`           | `<shift><left/></shift>`          | All       |
| `>>`           | `<shift><right/></shift>`         | All       |
| `>>>`          | `<shift><right/><unsigned/></shift>` | JS, TS |

### Assignment

| Token(s)       | Marker                     | Languages |
|----------------|----------------------------|-----------|
| `=`            | `<assign/>`                | All       |
| `+=`           | `<assign><plus/></assign>` | All       |
| `-=`           | `<assign><minus/></assign>`| All       |
| (etc.)         | `<assign><OP/></assign>`   | All       |

Compound assignment reuses the arithmetic/logical marker as a child of
`<assign>`, mirroring the language semantics ("+= is assignment with addition").

Query: `//expression[op[assign]]` — all assignments (simple and compound).

## Design Principles

This design follows:

- **#4 Elements Over Attributes**: Semantic meaning as child elements, not
  attribute values
- **#5 Unified Concepts**: `<equals>` groups `==` and `===` under one concept
- **#7 Modifiers as Empty Elements**: Operator markers are empty elements,
  queryable via predicates
- **#8 Renderability**: Original token preserved as text content for
  round-trip fidelity
- **#10 Marker Source Locations**: `<op>` carries the source location of the
  operator token

## Implementation Notes

- The `<op>` wrapper is already implemented in all languages
- Semantic markers are additive — they go inside existing `<op>` elements
- Languages can share the same taxonomy; the marker means the same *syntactic*
  concept even when runtime semantics differ (e.g. `==` in JS vs Java)
- Operators that don't fit a family get no marker; `<op>` with just text
  still works for queries and rendering
