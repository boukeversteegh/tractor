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
`line`/`column`/`end_line`/`end_column` source locations pointing to that keyword. Markers that are inferred (no
corresponding source token) omit the location.

```xml
<variable>
  <const line="1" column="1" end_line="1" end_column="6"/>
  <name line="1" column="7" end_line="1" end_column="8">x</name>
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

### 12. Flat Lists Over Wrapper Elements

When a wrapper exists only to group a homogeneous list of child
elements, drop the wrapper. The children become direct siblings of the
enclosing element. Each child carries a `field="<plural>"` attribute so
non-XML serializers (JSON, YAML) can reconstruct the logical group.

```xml
<!-- WRONG: double-wrapped parameter list -->
<method>
  <name>add</name>
  <parameters>
    <params>
      <param><name>a</name></param>
      <param><name>b</name></param>
    </params>
  </parameters>
</method>

<!-- RIGHT: flat siblings, grouped by field attribute -->
<method>
  <name>add</name>
  <parameter field="parameters"><name>a</name></parameter>
  <parameter field="parameters"><name>b</name></parameter>
</method>
```

Applies to purely-grouping wrappers: `parameters`, `arguments`,
`attributes`, `accessors`, `generics` (type parameter/argument lists).
Does **not** apply to wrappers that carry their own meaning — `body`,
`block`, `type` — since those represent distinct concepts, not just
"N things of the same kind".

Queries get shorter and read naturally:
- `//method[parameter]` — "method that has any parameter"
- `//method[not(parameter)]` — "zero-parameter method"
- `//method/parameter[1]` — "first parameter"
- `//call[count(argument)=3]` — "three-argument call"

JSON/YAML output stays sensible: the `field="parameters"` attribute
tells the serializer to collect same-field siblings into a `parameters`
array, so scalar-vs-array ambiguity is resolved deterministically.

**Rationale:** Supports Design Goal #1 (intuitive queries), #2
(readable tree — one less level of nesting), and #4 (minimal query
complexity). The wrapper element never represented a thing in the
source code — the parens and commas of the list live as sibling text
nodes either way — so removing it loses no renderer-relevant
information.

### 13. Identifiers are `<name>`; declaration vs reference via markers

An identifier in the tree is always `<name>` — whether it labels something
being declared (`<class><name>Foo</name>`) or refers to something already
in scope (`<binary><left><name>a</name>`). Likewise, a type reference or
inline type expression is always `<type>`.

Tree position already distinguishes a *declaration site* from a *reference
site* (source code works the same way — `Foo` after `class` is a
declaration; `Foo` in an expression is a reference). But because
reference resolution is a common cross-cutting query need, every `<name>`
carries an exhaustive marker (Principle #9) indicating its role:

```xml
<!-- Declaration: name introduces a binding into scope -->
<class><name><bind/>Foo</name></class>
<variable><let/><name><bind/>x</name></variable>
<param><name><bind/>a</name></param>

<!-- Reference: name refers to an already-bound identifier -->
<binary>
  <left><name><use/>a</name></left>
  <right><name><use/>b</name></right>
</binary>
<call><function><name><use/>print</name></function>...</call>
```

The markers are `<bind/>` and `<use/>` — short, symmetric, and established
PL-theory vocabulary (name binding, def-use chains).

#### Query ergonomics

Position-based queries don't need to mention the markers — they're only
useful when the query spans declaration kinds:

| Intent | Query |
|---|---|
| All identifier occurrences | `//name` |
| All declarations of `foo` (any kind) | `//name[bind][.='foo']` |
| All references to `foo` (any kind) | `//name[use][.='foo']` |
| Variable declarations named `foo` | `//variable[name='foo']` (marker redundant) |
| Uses of a variable that's declared as `x` | `//name[use][.='x']` |

Prefer the element name (`<variable>`, `<function>`, `<class>`) when the
query is about a specific kind of declaration. Use `[bind]`/`[use]`
markers when the query is about identifier roles across kinds, or when
filtering role on its own.

#### Why no marker on `<type>`

A `<type>` is always a reference to a type — even in a type alias
declaration, the declared *name* lives in a `<name>` element inside the
alias, and the `<type>` expression on the right-hand side is always a
reference to an existing type. So type declarations and references are
not a mutually exclusive variant pair the same way identifier
declarations and references are; no marker needed.

#### Edge cases

Some source constructs introduce a binding *and* reference an existing
one in the same syntactic position. Mark them with both:

```xml
<!-- Destructuring: const {a} = obj
     `a` declares a new local AND references obj.a. Both roles apply. -->
<name><bind/><use/>a</name>
```

#### Advanced: attribute escape hatch

For users who prefer category-level selection without the marker
predicate, the same role information is also available as `is="…"`
on declaration elements (class, function, variable, param, …):
`//*[@is='declaration']/name`. This violates the usual Elements-Over-
Attributes rule and is reserved for advanced cross-cutting queries; the
marker form is the documented default.

**Rationale:** Supports Design Goal #1 (intuitive queries — reference
resolution becomes `//name[use][.='x']`), #2 (readable tree — one element
name per identifier, not a vocabulary of decl/ref variants), and #4
(minimal query complexity — `//name` unions both roles in one query).
Applies Principle #9 (exhaustive markers) to identifier role.

---

## Decisions

Specific choices derived from the principles above. Each decision references
the principle(s) that justify it.

*See child specs for language-specific and feature-specific decisions.*
