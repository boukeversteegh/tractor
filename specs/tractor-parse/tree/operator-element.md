---
title: Operator Element
priority: 2
status: implemented
refs:
  - /tractor/src/xot_transform.rs # prepend_op_element, add_operator_markers, is_operator_marker
  - /tractor/src/languages/typescript.rs # extract_operator
  - /tractor/src/languages/csharp.rs # extract_operator
  - /tractor/src/languages/rust_lang.rs # extract_operator
  - /tractor/src/languages/python.rs # extract_operator
  - /tractor/src/languages/java.rs # extract_operator
  - /tractor/src/languages/go.rs # extract_operator
  - /tractor/src/languages/tsql.rs # extract_operator
---

Operators in expressions are represented as `<op>` child elements with semantic
marker children that classify the operator, plus the original token as text.

## Structure

Semantic marker elements are added inside `<op>` as **flat siblings** —
no nested marker categories. Each marker is an empty empty element;
together they form a presence-flag set on `<op>`. The raw token is
preserved as text content of `<op>` for source-text round-trip.

```xml
<binary>
  <op><equals/><strict/>===</op>
  <left>x</left>
  <right>0</right>
</binary>
```

### Query patterns

Both text matching and semantic matching work; markers are queried as
predicates on `<op>` directly.

```xpath
//binary[op='===']                          (exact token match)
//binary[op[equals]]                        (any equality: == or ===)
//binary[op[equals and strict]]             (strict equality only)
//binary[op[compare]]                       (any comparison: < > <= >=)
//binary[op[compare and equal]]          (>= or <=)
//binary[op[compare and less and equal]] (only <=)
//binary[op[logical and and]]               (logical and: && or 'and')
//assign[op[assign]]                        (any compound assignment)
//assign[op[assign and logical and and]]    (only &&=)
```

Marker order on `<op>` is "primary, children, nested-name,
nested-children" (see Operator Taxonomy below) but since markers are
presence-flags the order is semantically irrelevant — XPath
`[a and b]` predicates use them as a set.

### Why flat, not nested?

Earlier iterations nested markers (`<op><equals><strict/></equals></op>`),
which forced asymmetric queries: `op[equals]` for a one-marker op,
`op/equals[strict]` for a two-marker op. The flat form gives one
uniform shape regardless of marker count, and `op[equals]` finds
both `==` and `===`. See Principle #12 — flat siblings over wrapper
elements; the same rationale applies to marker categories as to
list containers.

### Graceful degradation

Operators without a semantic marker get no marker — just `<op>` with text
content. The taxonomy can be built incrementally: common operators get markers
first, obscure ones later (or never).

```xml
<!-- Semantic marker available -->
<op><plus/>+</op>

<!-- No marker — still works, just no semantic query -->
<op>=</op>
```

## Operator Taxonomy

Operators grouped by semantic family. Each family is a marker element that may
contain sub-markers for variations.

### Equality

| Token(s)       | Markers on `<op>`              | Languages          |
|----------------|--------------------------------|--------------------|
| `==`           | `<equals/>`                    | All                |
| `===`          | `<equals/><strict/>`           | JS, TS, PHP        |
| `!=`           | `<inequality/>`                | All                |
| `!==`          | `<inequality/><strict/>`       | JS, TS, PHP        |

### Comparison

| Token(s)       | Markers on `<op>`                       | Languages |
|----------------|-----------------------------------------|-----------|
| `<`            | `<compare/><less/>`                     | All       |
| `>`            | `<compare/><greater/>`                  | All       |
| `<=`           | `<compare/><less/><equal/>`          | All       |
| `>=`           | `<compare/><greater/><equal/>`       | All       |

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

| Token(s)          | Markers on `<op>`         | Languages       |
|-------------------|---------------------------|-----------------|
| `&&` / `and`      | `<logical/><and/>`        | All             |
| `\|\|` / `or`     | `<logical/><or/>`         | All             |
| `!` / `not`       | `<logical/><not/>`        | All (unary)     |
| `??`              | `<nullish/>`   | JS, TS, C#      |

Query: `//binary[op[logical]]` — all logical operations.

### Bitwise

| Token(s)       | Markers on `<op>`                 | Languages |
|----------------|-----------------------------------|-----------|
| `&`            | `<bitwise/><and/>`                | All       |
| `\|`           | `<bitwise/><or/>`                 | All       |
| `^`            | `<bitwise/><xor/>`                | All       |
| `~`            | `<bitwise/><not/>`                | All (unary) |
| `<<`           | `<shift/><left/>`                 | All       |
| `>>`           | `<shift/><right/>`                | All       |
| `>>>`          | `<shift/><right/><unsigned/>`     | JS, TS    |

### Assignment

| Token(s)       | Markers on `<op>`                  | Languages |
|----------------|------------------------------------|-----------|
| `=`            | *(none)*                           | All       |
| `+=`           | `<assign/><plus/>`                 | All       |
| `-=`           | `<assign/><minus/>`                | All       |
| `&&=`          | `<assign/><logical/><and/>`        | JS, TS    |
| `&=`           | `<assign/><bitwise/><and/>`        | All       |
| `<<=`          | `<assign/><shift/><left/>`         | All       |
| (etc.)         | `<assign/><OP-family/><FLAG/>`     | All       |

Bare `=` receives **no marker**. Assignment semantics are carried by the parent
element (`<assign>`, `<variable>`, etc.), and `=` is ambiguous across languages:
in SQL it means equality in comparisons (`WHERE x = 1`) and assignment in SET
clauses (`SET x = 1`). Rather than misclassify, we leave `=` unmarked —
the parent element already disambiguates.

Compound assignments (`+=`, `&&=`, etc.) carry an `<assign/>` marker plus the
operator family's markers as flat siblings on `<op>`. The compound nature
is queryable via `//assign[op[assign]]`; the specific compound is
queryable by adding more marker predicates: `//assign[op[assign and logical and and]]`
finds only `&&=`.

Query: `//assign[op[assign]]` — all compound assignments.

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

- Shared classification function `add_operator_markers` in `xot_transform.rs`
  maps operator text to semantic markers
- Each language's `extract_operator` calls `prepend_op_element` which handles
  marker insertion and text content in one step
- `is_operator_marker` helper used by all language `syntax_category` functions
  to map marker elements to `SyntaxCategory::Operator` for highlighting
- Languages share the same taxonomy; the marker means the same *syntactic*
  concept even when runtime semantics differ (e.g. `==` in JS vs Java)
- Operators that don't fit a family get no marker; `<op>` with just text
  still works for queries and rendering
