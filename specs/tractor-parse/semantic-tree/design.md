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

### 5. Match the Developer's Mental Model

The baseline shape of the tree is what an average developer thinks about
their code when writing or reading it: *a function is a function, an
argument is an argument, a variable is a variable*. That vocabulary is
already in the developer's head, so it's what the tree surfaces by
default — no translation layer between source and query.

Onboarding is the primary optimisation. Common cases must be easy.
Concrete is simpler than abstract; abstractions build on top of concrete
things; therefore concrete is primary and abstractions are layered on.

**The yardstick for "how precise":** would a developer describe this
distinction in natural language when talking about the code?

- A `function` is a function whether its body is a block or a lambda
  expression. Developers don't routinely say "lambda-bodied function"
  vs "block-bodied function" — same word in their head.
- A `call` is a call whether the callee is a plain name or a member
  expression. Same word.
- A `variable` is a variable whether the initializer is a literal or a
  call.
- `Foo` in `class Foo {}` and `Foo` in `new Foo()` are the same
  identifier in the developer's head — the role differs but the
  vocabulary is "Foo".

Both directions away from the baseline are served **additively**, never
by taxing the baseline:

- **More precise than the baseline** (distinguish sub-variants)
  → empty-element markers on the complex node (`<function><async/>`,
  `<function><lambda/>` if we wanted to flag lambda-bodied functions).
  Free when the target is a complex node.
- **More abstract than the baseline** (group across kinds)
  → adjacent tooling: tractor-bound XPath variables (`$declaration`,
  `$reference`), attributes dropped in default JSON output
  (`<name ref="…">`), side-channel metadata from a resolver pass,
  saved queries.

Neither direction reshapes the tree that every consumer reads.

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

**Rationale:** Directly serves Goal #5 (developer's mental model —
"I'm writing a binary expression", not "I'm writing an expression of
kind binary"). Also supports Goal #1 (intuitive queries — `//binary`
beats `//expression[binary]`), #2 (readable tree — less nesting), and
#4 (minimal query complexity — no extra predicates needed). Broader
queries ("all expressions") are served additively via variables or
unions (Principle #13).

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

**Rationale:** Directly serves Goal #5 (developer's mental model — a
developer thinks "the function has parameters a and b", not "the
function has a parameters container that has a params list"). Also
supports Goals #1 (intuitive queries), #2 (readable tree — less
nesting), and #4 (minimal query complexity). The wrapper element never
represented a thing in the source code — the parens and commas of the
list live as sibling text nodes either way — so removing it loses no
renderer-relevant information.

### 13. Annotation Follows Node Shape

Adding a child element to a node has very different cost depending on
whether the node is already complex or is a text-only leaf:

| Node shape | Cost of adding `<marker/>` child | JSON consequence |
|---|---|---|
| **Complex** (has existing children) — `<method>`, `<class>`, `<variable>` | Free — adds one boolean-valued property to an existing object | `{..., "public": true, ...}` |
| **Text-only leaf** — `<name>foo</name>`, `<type>int</type>` | Changes the leaf's shape entirely | `"name": "foo"` becomes `{marker: true, text: "foo"}` |

This is a property of the format mapping, not an opinion: any design
choice must respect it.

#### How to add semantic information

The menu of mechanisms, ranked by tree cost:

| Mechanism | Tree cost | JSON cost | Good for |
|---|---|---|---|
| **Position** — role inferred from parent/ancestor | Zero | Zero | Distinctions that are always structurally determined |
| **Empty marker on complex node** (`<method><public/>`) | One element | One boolean property | Exhaustive modifier sets on declarations (access, async, static, literal/comprehension, raw) |
| **Element rename / split** (`<name>` vs `<ref>`) | Zero | Zero | Truly primary binary distinctions where both halves deserve separate names |
| **Attribute** (`<name is="…">`) | One attribute | Dropped in default JSON | Advanced / cross-cutting metadata; resolver pointers |
| **External annotation map** (side-channel from a resolver pass) | Zero | Zero | Scope-resolved info, cross-file references |

#### Decision tree for future additions

When a new semantic distinction is proposed, ask in order:

1. Is the distinction always determined by tree position?
   → use **position** (no tree change).
2. Is the target a **complex node**?
   → use an **empty marker**, freely.
3. Is the target a **text-only leaf**?
   → prefer **position** where structurally determined; fall back to an
   **attribute** or an **external annotation map** for advanced cases.
   Avoid adding markers to leaves.
4. Is the distinction binary and always present, with both halves
   meaningful as standalone element names?
   → an **element rename/split** may be justified.

#### Why no marker on `<type>` or `<name>`

Both are text-only leaves in their dominant uses. Adding child markers
would change `"name": "foo"` into `{marker: true, text: "foo"}` —
regressing the single most-consumed leaf field across every downstream
consumer. Their roles (declaration vs reference, type-reference vs
type-parameter-name) are inferred from tree position; cross-cutting
queries are served by the additive mechanisms above.

**Rationale:** This principle is a direct consequence of Goal #5
(match the developer's mental model; keep the baseline free of advanced-
case complexity). Every additive mechanism listed serves precision or
abstraction needs without taxing the baseline tree.

---

## Decisions

Specific choices derived from the principles above. Each decision references
the principle(s) that justify it.

### Identifiers are a single `<name>` element

A developer reads `Foo` in `class Foo {}` and `Foo` in `new Foo()` as
the same identifier — the *role* differs (declaration vs reference)
but they're both "Foo" at the mental-model level.

The tree reflects that: `<name>Foo</name>` in both positions. Role is
inferred from parent element; source code conveys role by position
in exactly the same way.

```xml
<!-- Declaration: position (direct child of a declaration element) -->
<class><name>Foo</name></class>
<variable><let/><name>x</name></variable>
<param><name>a</name></param>

<!-- Reference: position (inside an expression wrapper) -->
<binary>
  <left><name>a</name></left>
  <right><name>b</name></right>
</binary>
<call><callee><name>print</name></callee>...</call>
```

Cross-cutting "every declaration of foo regardless of kind" is served
additively (Principle #13) — by `$declaration` (planned XPath variable),
or `<name ref="…">` attributes from a future resolver pass. The default
tree stays flat.

**Cites:** Goal #5 (developer's mental model), Principle #11 (no
`<reference>` supertype wrapper), Principle #13 (leaf-node shape —
avoid markers on `<name>`).

**Rejected alternatives:**

- **Empty child markers** (`<name><bind/>Foo</name>` / `<name><use/>x</name>`):
  regressed the JSON leaf shape. Violates Principle #13.
- **Element split** (`<name>` for declarations, `<ref>` for references):
  doubled the vocabulary for a distinction the tree shape already makes;
  the short `<ref>` element was also ambiguous with other uses (issue #73).

*See child specs for language-specific and feature-specific decisions.*
