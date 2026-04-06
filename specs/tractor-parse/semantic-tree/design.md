---
title: Semantic Tree Design
priority: 0
---

This document defines the design goals, guiding principles, and decisions for
transforming TreeSitter syntax trees into semantic XML.

## Design Goals

Outcomes we want to achieve, regardless of implementation.

### 1. Intuitive Queries

Queries should be intuitive to read and write. A developer should be able to
express "find all public static methods" as `//method[public][static]` without
consulting documentation. Zero mental translation between the question and the
query.

### 2. Readable Tree Structure

The XML tree should be easy to read for developers unfamiliar with parse trees.
When viewing the raw XML output, developers should see element names they
recognize from their programming language, not TreeSitter internals.

### 3. Discoverability

A developer exploring an unfamiliar codebase should be able to understand the
tree structure by inspection. Element names should be self-explanatory.

### 4. Minimal Query Complexity

Users should be able to find all instances of a concept (all types, all methods,
all parameters) with simple queries. Minimize the need for disjunctions like
`//type | //generic | //array`.

---

## Guiding Principles

Rules that help us make consistent decisions.

### 1. Use Language Keywords

Use element names that match keywords from the source language where possible.
Developers already know their language's keywords (`class`, `def`, `function`,
`if`, `return`). These terms require no learning.

**Rationale:** Supports Design Goal #2 (readable tree) and #3 (discoverability).

### 2. Full Names Over Abbreviations

Use complete, readable element names. Prefer `property` over `prop`, `parameter`
over `param`, `attribute` over `attr`.

**Rationale:** Supports Design Goal #2 (readable tree). Abbreviations save typing
but hurt readability and discoverability. Tab completion handles typing efficiency.

### 3. Always Lowercase

All element names are lowercase. No exceptions.

**Rationale:** Supports Design Goal #1 (intuitive queries). Developers never need
to guess capitalization. Easy to remember, easy to type.

### 4. Elements Over Attributes

Represent all queryable information as child elements, not XML attributes.
The only exception is the `kind` attribute which stores TreeSitter metadata
for debugging purposes.

**Rationale:** Supports Design Goal #1 (intuitive queries). Element predicates
(`//method[public]`) are simpler than attribute predicates (`//method[@public='true']`).
Consistent structure means developers never wonder "is this an attribute or element?"

### 5. Unified Concepts

Similar concepts should use the same element name regardless of context. Function
call arguments and attribute arguments are both `<argument>` inside `<arguments>`.
All type references are `<type>` whether simple, generic, or array.

**Rationale:** Supports Design Goal #4 (minimal query complexity). Finding "all
arguments" is `//argument`, not `//argument | //attribute_argument`.

### 6. Preserve Language Idioms

When a language has a well-known keyword or term, preserve it even if it's short.
Developers recognize their language's type keywords (`int`, `bool`, `str`, `def`).
Keep these familiar rather than expanding them.

**Rationale:** Supports Design Goal #2 (readable tree). Expanding `int` to `integer`
or `def` to `definition` would feel foreign to developers.

### 7. Modifiers as Empty Elements

Access modifiers and keywords like `static`, `async`, `readonly` become empty
child elements: `<public/>`, `<static/>`, `<async/>`.

**Rationale:** Supports Design Goal #1 (intuitive queries) and Principle #4
(elements over attributes). Queries read naturally: `//method[public][static]`.

### 8. Renderability

The transformed AST must be renderable back into valid source code. Every
structural distinction needed for correct syntax must be preserved in the
element names alone (without relying on `kind` attributes). If a transform
loses information that a renderer would need to reconstruct valid syntax,
the transform violates this principle and should be fixed.

**Rationale:** Enables code generation from the AST (e.g. C#→TypeScript).
A handcoded renderer achieved 100% round-trip fidelity once this principle
was applied.

### 9. Exhaustive Markers for Mutually Exclusive Variations

When lifted modifiers represent mutually exclusive choices, **all** variants
must have an explicit marker — don't use the absence of a marker as a default.

```xml
<!-- WRONG: absence of <const/> implicitly means let or var -->
<variable><name>x</name></variable>

<!-- RIGHT: always include one marker from the set -->
<variable><let/><name>x</name></variable>
<variable><const/><name>y</name></variable>
```

This ensures:
- **Queries are symmetric**: `//variable[const]` and `//variable[let]`
  work the same way — match on presence, never on absence.
- **Rendering is unambiguous**: switch on which marker is present.
- **No implicit knowledge needed**: no "unmarked default" to memorize.

Mutually exclusive sets currently identified:
- **Declaration kind**: `const`, `let`, `var`
- **Parameter optionality**: `required`, `optional`
- **Access modifiers**: `public`, `private`, `protected`, `internal`

**Rationale:** Supports Design Goal #1 (intuitive queries) and Principle #8
(renderability).

### 10. Marker Source Locations

Lifted modifier elements that correspond to a source keyword carry
`startLine`/`startCol`/`endLine`/`endCol` source locations pointing to that keyword. Markers that are inferred (no
corresponding source token) omit the location.

```xml
<variable>
  <const startLine="1" startCol="1" endLine="1" endCol="6"/>
  <name startLine="1" startCol="7" endLine="1" endCol="8">x</name>
</variable>
```

**Rationale:** Supports Principle #8 (renderability). Exemplar-based renderers
need source locations to learn correct gap patterns for keywords.

### 11. Specific Names Over Type Hierarchies

Use the most specific semantic name for each node. Don't encode type
hierarchies (is-a relationships) as wrapper elements. A `<binary>` is known
to be an expression by its position in the tree, not by wrapping it in
`<expression><binary/>`.

```xml
<!-- WRONG: encoding is-a hierarchy -->
<expression><binary/><op>+</op><left>a</left><right>b</right></expression>
<declaration><class/><name>Foo</name></declaration>
<member><method/><name>bar</name></member>

<!-- RIGHT: use the specific name directly -->
<binary><op>+</op><left>a</left><right>b</right></binary>
<class><name>Foo</name></class>
<method><name>bar</name></method>
```

The hierarchy is implicit from tree position — a `<binary>` inside a
`<method>` body is obviously an expression. Adding wrapper elements
increases nesting, query verbosity, and tree noise without enabling
useful queries ("find all expressions" is too broad to be practical).

**Rationale:** Supports Design Goal #1 (intuitive queries — `//binary`
beats `//expression[binary]`), #2 (readable tree — less nesting), and
#4 (minimal query complexity — no extra predicates needed).

---

## Decisions

Specific choices derived from the principles above. Each decision references
the principle(s) that justify it.

*See child specs for language-specific and feature-specific decisions.*
