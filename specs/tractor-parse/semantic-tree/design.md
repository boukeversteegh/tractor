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
Primary nodes are concrete *developer concepts*, not raw grammar
variants. `<variable>` is concrete (developers say "variable") even
though `const` / `let` / `var` are surface variants under it. `<call>`
is concrete even though Rust's `?` and JavaScript's `await` are surface
modifiers around it. Variants narrow a stable concept; they do not
replace it.

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

### 6. Broad-to-Narrow Query Refinement

A simple query should match the broadest concept a developer is likely
to mean. More specific variants should be expressed by adding
predicates or markers, not by reaching for a different element name.

Rule authoring is iterative: write a broad query, inspect the matches
in the current codebase, and narrow until the result set captures the
intended pattern. False positives are visible during authoring and can
be refined away. False negatives caused by an unintentionally narrow
tree shape are silent — they only appear later when a real-world case
drifts past the rule, undermining the whole point of writing a rule
once.

Good:

```xpath
//variable
//variable[const]
//variable[const][name='foo']
```

Bad:

```xpath
//const
```

The `//const` shape silently excludes `let` and `var` declarations
unless the user already knows to search for each separately. The same
pattern applies to expressions: `//call` should keep matching whether
the call is awaited, try-propagated, null-forgiven, or
optional-chained.

This goal interacts directly with Goal #5: the broadest natural
concept must be a *concrete developer concept* (`variable`, `call`,
`member`), not an abstract supertype (`expression`, `declaration`).
Surface variants narrow a stable concept; they do not replace it.

### 7. Source Reversibility

The semantic tree's text, when concatenated in document order and the
element tags stripped, should reproduce the original source. This is a
goal — not always achievable, because:

- Whitespace between tokens is often dropped during parsing.
- Modifiers, keywords, and punctuation that got lifted into markers
  have to survive as dangling text siblings to satisfy the goal.

Practical consequences:

- Markers that replace a source keyword (e.g. `<public/>`,
  `<static/>`, `<get/>`) keep the keyword text as a sibling in the
  parent. The parent's XPath string-value then contains the original
  token.
- Marker order follows source order. `public abstract static class`
  renders as `class[public and abstract and static]/ "public abstract static class" …`
  — a reader comparing tree to source shouldn't see shuffled
  keywords.
- Wrappers that get flattened (e.g. `<modifiers>`,
  `parenthesized_expression`) preserve their text content by lifting
  it into the parent, not dropping it.

This is closely related to Principle #8 (Renderability): renderability
is about reconstructing valid syntax, source reversibility is about
the textual content matching what the user wrote. Both push the
transform toward keeping source tokens reachable.

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

**Names don't need to be globally unique.** The principle is *concept ↔ name*,
not *name ↔ exactly-one-element*. The same name can appear at multiple roles
(e.g. as a structural container *and* as a marker on `<op>`) as long as parent
context disambiguates queries. `//op/receive` (the operator marker) and
`//case/receive` (Go's `receive_statement` clause) are unambiguous; only bare
`//receive` matches both, and that's expected — both nodes really are
"channel receive" at different syntactic levels. The design uses this same
pattern elsewhere (`<default>` as a switch arm vs. a parameter-default marker;
`<this>` as the bare expression vs. a `<call>` marker; `<not>` inside
`<logical>` vs. inside `<bitwise>`).

A name "collision" alone is not a Principle #5 violation. Reach for a rename
only when the two uses encode *different* concepts — at which point the issue
is misuse of one of the names, not the sharing.

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

### 14. Namespace Vocabulary

Most programming languages separate identifiers into a **value
namespace** (variables, functions, parameters, method names) and a
**type namespace** (classes, interfaces, structs, enums, type aliases,
generic parameters). Tree-sitter grammars already track this at the
token level — `identifier` vs `type_identifier`, `primitive_type`,
`predefined_type`, etc. — and the semantic tree preserves that
distinction by using two different element names:

- **`<name>`** — an identifier in the *value namespace*. Whether it
  is being declared or referenced, the element is `<name>`; role is
  inferred from tree position (see the `<name>` decision below).

- **`<type>`** — a reference to (or expression of) something in the
  *type namespace*. Every type-reference slot — parameter type,
  return type, base class, implemented interface, generic argument,
  trait bound, default type, field type, variable type — carries a
  `<type>` child. Simple types are text content
  (`<type>int</type>`); complex types add markers and children
  (`<type><generic/>List<type field="arguments">int</type></type>`).

Type *declarations* still use specific element names
(`<class>`, `<interface>`, `<struct>`, `<alias>`, `<enum>`, `<record>`,
and — only where the declared thing *is literally a type* — `<type>`
itself, as in Go's `type MyInt int`). Their identifier is a `<name>`
child, same as every other declaration:

```xml
<class><name>Foo</name>...</class>          <!-- class declaration -->
<struct><name>Hello</name>...</struct>      <!-- Go struct -->
<alias><name>Color</name><type>int</type></alias>

<param><name>x</name><type>int</type></param>     <!-- type reference -->
<returns><type>int</type></returns>                <!-- type reference -->
<base><type>Bar</type></base>                      <!-- type reference -->
<implements><type>IBaz</type></implements>         <!-- type reference -->
<generic><name>T</name><bound><type>Comparable</type></bound></generic>
```

Two axes, orthogonal:

- **Which namespace** the identifier belongs to → element name
  (`<name>` for values, `<type>` for types).
- **Declaration vs reference** → tree position (inside a declaration
  element vs inside an expression/reference wrapper).

#### Why this matters for queries

- `//type[.='Bar']` finds every use of `Bar` *as a type* — parameter
  type, return type, base class, interface implementation, generic
  argument. A uniform handle for all type references.
- `//name[.='foo']` finds every value-namespace identifier. Doesn't
  include type references; those are queried via `<type>`.
- Neither query crosses the other's namespace, matching how the
  language itself handles scoping.

#### Why this matters for avoiding the old bug

Earlier transforms conflated the two axes: some value-namespace
*references* were mis-classified as `<type>` because the
classification code didn't trust tree-sitter's kinds and tried to
infer type-ness from position. The rule now: trust tree-sitter's
distinction. `type_identifier`/`primitive_type`/etc. → `<type>`;
`identifier` → `<name>`. No context-dependent reinterpretation.

**Rationale:** Supports Goal #5 (developer's mental model — languages
use two namespaces, so the tree does too), Principle #5 (unified
concepts — one element per namespace regardless of context),
Principle #11 (specific names — `<type>` and `<name>` are concrete
concepts, not abstract supertypes).

### 15. Markers Live in Stable, Predictable Locations

Markers (`<public/>`, `<async/>`, `<const/>`, `<await/>`, `<try/>`,
`<nullable/>`, `<generic/>`, …) carry meaning by their presence on a
parent. For that to work, the parent has to be a stable, predictable
location — the same parent shape whether or not the marker is
present, and across all surface variants of the same concept.

Concrete consequence: **when a developer concept has variations,
that concept must have its own node** so the variations can attach
as markers without disturbing the concept's identity in the tree.

```xml
<!-- WRONG: surface variant becomes the parent.
     Now //variable misses const, and //const misses let/var. -->
<const><name>x</name></const>
<let><name>y</name></let>

<!-- RIGHT: stable concept node, variant as marker.
     //variable matches both, //variable[const] narrows. -->
<variable><const/><name>x</name></variable>
<variable><let/><name>y</name></variable>
```

```xml
<!-- WRONG: modifier wrapper steals the call's parent slot. -->
<try><call>foo()</call></try>      <!-- Rust foo()? -->
<await><call>foo()</call></await>  <!-- JS await foo() -->

<!-- RIGHT: stable host for the expression position, modifier on the host. -->
<expression><call>foo()</call><try/></expression>
<expression><await/><call>foo()</call></expression>
```

Two failure modes this rules out:

1. **Variant-as-parent.** Promoting a surface variant
   (`<const>`, `<try>`, `<await>`) to the parent name fragments the
   concept across multiple element names. Broad queries
   (`//variable`, `//call`) silently miss whichever variant the
   author didn't think to enumerate — the dangerous failure mode
   identified in Goal #6.

2. **Markers on text-only leaves.** Adding `<try/>` directly under
   `<name>foo</name>` regresses the leaf's JSON shape from `"foo"`
   to `{try: true, text: "foo"}` (Principle #13). A stable parent
   one level up gives the marker somewhere safe to live.

Companion principles:

- **Principle #7** (modifiers as empty elements) defines the marker
  shape; this principle says where the markers may *attach*.
- **Principle #11** (specific names over type hierarchies) keeps
  the operand inside the stable parent specifically named —
  `<variable><const/>`, `<expression><call>` — so concept queries
  (`//variable`, `//call`) stay broad without re-introducing an
  abstract hierarchy wrapper.
- **Principle #13** (annotation follows node shape) explains *why*
  markers can't just go on any node; this principle explains *where
  they go instead*.

Concrete realizations of this principle (each in [Decisions](#decisions)):
the `<variable>` declaration shape (Principle #9 spelled out for
declaration-kind variants); the `<expression>` host for expression
modifiers; multi-discriminator `<type>` with `<nullable/>`,
`<generic/>`, `<array/>` markers.

### 16. Optimize for Repeated Patterns

Tractor rules are written for repeating codebase patterns, not for
one-off expressions. The tree shape should optimize for that case.

A team's rule typically targets shapes like:

- "every public controller method must have an authorization
  attribute"
- "no constructor parameter named `httpClient` without dependency
  injection"
- "consecutive `xot.with_*` calls in the same body should be chained"

These rules are valuable because they catch the same mistake many
times across the codebase. They are not written for a deeply nested
one-off expression that happens to appear once.

When designing tree shape, prefer **stable repeated query surfaces
over compact rendering of rare deep expression trees.** A more
verbose tree with predictable shape is worth more than a compact
tree whose shape shifts under surface variations, because the
verbosity cost is paid once during inspection while the shape cost
is paid every time a rule under-scopes silently.

This is why uniform expression hosts are acceptable even though they
make deeply nested expression trees more verbose — those trees are
rarely the direct target of durable rules.

#### Exception: fluent DSLs

Some complex expressions *are* rule-worthy because they are repeated
DSL shapes — LINQ, query builders, validation builders, routing
DSLs, dependency-injection registration chains, test setup builders.
A rule like "queries with more than 3 `Include` calls where each
include path is two or more levels deep must call `.AsSplitQuery()`"
targets a complex expression, but the expression is a *recurring*
shape, not a one-off.

Even here, the design preference is the same: consistency matters
more than compactness. DSL chains are queryable precisely *because*
their repeated shape is predictable. A stable expression host shape
across calls, members, arguments, lambdas, and modifiers is what
makes the rule expressible at all.

**Cites:** Goal #5 (mental model — rule authors think in terms of
"this pattern, repeated"), Goal #6 (broad-to-narrow refinement
depends on stable broad shapes), Principle #15 (stable marker
locations are what make the repeated query surface stable).

### 17. Avoid Compound Node Names

Avoid compound (multi-word, underscored) node names. Single-word
names keep queries short, predictable, and easy to remember. A
compound like `string_literal` or `function_declaration` almost
always signals one of three latent restructurings:

1. **Variation expressible as a marker.** The compound encodes a
   sub-kind that should attach as an empty marker on the broader
   concept (Principles #7, #15).
   `string_literal` → `<string><literal/>...</string>`,
   queryable as `//string[literal]`.

2. **Context already supplied by tree position.** The compound
   adds a contextual qualifier that the parent already provides
   (Principle #11).
   `function_parameter` → `<function>/<parameter>`, queryable as
   `//function/parameter`. No need to repeat `function_` in the
   child name.

3. **AST jargon that can be dropped entirely.** The suffix
   describes the grammar machinery (declaration, expression,
   statement, clause, list) rather than the developer concept.
   `function_declaration` → `<function>`,
   `if_statement` → `<if>`, `parameter_list` → flat `<parameter>`
   siblings (Principle #12).

**Allowed exceptions.** A compound is acceptable only when no
single-word name conveys the concept, AND none of the three
restructurings above applies. The bar is high — the obvious test
case is `else_if`, where the concept is genuinely the *combination*
of two keywords and neither half alone names it. Rare; expect to
justify each one individually.

```xml
<!-- WRONG: AST jargon as a node name -->
<function_declaration><name>foo</name>...</function_declaration>

<!-- WRONG: contextual qualifier duplicating the parent -->
<function><function_parameter><name>x</name></function_parameter></function>

<!-- WRONG: variation baked into the name -->
<string_literal>"hi"</string_literal>

<!-- RIGHT: drop the suffix, lift the variant, trust position -->
<function><name>foo</name><parameter><name>x</name></parameter></function>
<string><literal/>"hi"</string>
```

The mechanical gate for this principle is the
`no_underscore_in_node_names_except_whitelist` invariant in
`tractor/tests/tree_invariants.rs`. It walks every language's rule
table and fails on any `Rule::Passthrough` whose snake_case kind
contains an underscore not on `ALLOWED_UNDERSCORE_NAMES` — catching
drift at the table layer before it surfaces in fixture output.

**Cites:** Goal #1 (intuitive queries — single-word names are
easier to type and remember), Goal #4 (minimal query complexity),
Principle #5 (unified concepts), Principle #7 (modifiers as
markers), Principle #11 (specific names, not type hierarchies),
Principle #12 (flat lists over wrapper elements).

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

### Identifiers are never element names

Nodes are always lowercase (Principle #3). Identifiers — user-defined
OR language-built-in — carry distinguishing capitalization (`List` vs
`list`, `Dictionary` vs `dict`), so promoting an identifier to a node
name would either lose the case distinction or break the
all-lowercase rule.

This applies uniformly: `int`, `double`, `List`, `HashMap`, `Foo` are
all values inside a `<name>` element, never `<int/>` or `<List/>`
markers. Users don't need to remember which types are "well-known
enough" to be promoted — the rule is simple and cross-language
consistent. Queries use `//type[name='int']` / `//type[name='List']`
uniformly.

**Exception — additive markers for very unique built-ins.** A small
set of built-ins that represent a language *concept* (not a data
type) may carry an additional marker *alongside* the name, never
replacing it. The canonical case is `void`:

```xml
<type><void/><name>void</name></type>
```

JSON sees `{ "name": "void", "void": true }` — the name stays for
data consumers, the marker is a query shortcut (`//type[void]`).
Reserved for constructs that are return-only or otherwise
structurally special (Kotlin's `unit`, Rust's `!` never-type would
qualify); not a backdoor for adding markers to every popular
built-in.

**Cites:** Principle #3 (Always Lowercase), Principle #7 (Modifiers
as Empty Elements — markers must stay empty, which is why we can't
put the keyword text inside the marker).

### Expression positions get an `<expression>` host

Concrete realization of Principle #15 for expression positions.

Every expression position is wrapped in an `<expression>` host
element. Closed-set expression modifiers — `<await/>`, `<try/>`
(Rust `?`), `<non_null/>` (TS `foo!`), `<conditional/>` (`?.`),
`<deref/>`, `<ref/>`, etc. — attach as empty markers on the host.
The operand keeps its specific named element (`<call>`, `<member>`,
`<binary>`, `<name>`, …) inside the host.

```xml
<!-- WRONG: per-modifier wrapper steals identity from the operand -->
<try><call>foo()</call></try>            <!-- Rust foo()? -->
<await><call>foo()</call></await>        <!-- JS await foo() -->

<!-- RIGHT: uniform host, modifier as marker, operand keeps its name -->
<expression><call>foo()</call><try/></expression>
<expression><await/><call>foo()</call></expression>
```

The host appears at *every* expression position, even when no
modifier is present. Bare expressions like `return y` become
`<return><expression><name>y</name></expression></return>`. This
verbosity is the price of uniform shape; the benefit is that two
identities are preserved at once:

- **Operand identity.** `//call` finds every call regardless of
  whether it is awaited, try-propagated, null-forgiven, or
  conditionally-accessed. The call element name doesn't change
  under surface variation.
- **Position identity.** A modified expression and an unmodified
  expression have the same parent shape. Two adjacent expression
  statements are siblings under `<body>` whether or not either has
  a `?` or `await`.

#### The motivating example

Two consecutive fluent calls should be expressible as siblings:

```rust
xot.with_a(node)?;
xot.with_b();
```

A rule that targets "two adjacent `xot.with_*` calls in the same
body" must keep working when one of them gains a `?`. With the
expression host, both statements are `<expression>` siblings of
`<body>`, and the rule reads naturally:

```xpath
//body/expression[
  call/callee/member[object/expression/name='xot' and starts-with(name, 'with_')]
]
/following-sibling::expression[1][
  call/callee/member[object/expression/name='xot' and starts-with(name, 'with_')]
]
```

Without the host, the `?`-suffixed call would have a different
parent (`<try>`) than its plain neighbour (`<body>`), and they would
never appear as siblings — the rule would silently miss every mixed
case.

#### Rejected alternatives

- **Keep modifier-specific wrappers** (`<try>`, `<await>`,
  `<non_null>` as parents). Closest to tree-sitter, minimal
  transform work, but each new modifier becomes a special case in
  every position-sensitive query. Forces defensive disjunctions like
  `self::call or self::try or self::await`. Rejected: violates
  Principle #15.
- **Markers directly on the operand** (`<call><try/>...</call>`).
  Works for complex operands, but breaks for text-only leaves
  (`<name>foo</name>` would have to grow children, regressing JSON
  shape — Principle #13). Also ambiguous in chained expressions
  about which operand owns the modifier. Rejected as the general
  model.
- **Host only when a modifier is present.** Modified and unmodified
  expressions still have different parent shapes — the sibling
  problem from the motivating example is unsolved. Rejected: only a
  partial fix.
- **Reuse role nodes as hosts** (`<argument>`, `<return>` host the
  marker directly). Avoids one wrapper, but expression modifiers end
  up scattered across many host types and there is no single
  expression-level query surface. Rejected as the primary design;
  remains viable as a localized optimization.
- **Virtual-node / transparent XPath** (keep `<try><call>` in the
  tree, make the engine treat it as transparent). Breaks the
  promise that the displayed tree is the queried tree. Rejected for
  the core model; could be revisited as optional query sugar.

#### What stays as a named wrapper element

Not every node inside an expression position becomes a marker.
Named wrappers stay where the wrapper *introduces structure or
carries data*, not just where it annotates:

- **Open-set operators with structural grouping**: `<binary>`,
  `<unary>`, `<assign>`, `<ternary>`. Their identity *is* the
  operator and they carry left/right/op children. Reducing them to
  `<expression><binary/>` would lose structural grouping.
- **Control-flow and structural constructs**: `<if>`, `<while>`,
  `<for>`, `<return>`, `<function>`, `<class>`, statement-form
  `<try>` (distinct from Rust's expression-level `?`). These
  introduce new structure, not annotations.
- **Constructs that carry a target as data**: `<cast>` keeps a
  `<type>` child; the cast is a construct, not a closed-set
  keyword.

#### Use concrete nodes vs. expression hosts

Two complementary query surfaces; users learn the distinction by use:

```xpath
//call          // "where do calls occur, anywhere?"     — concept query
//member        // "where do member accesses occur?"     — concept query
//binary        // "where do binary expressions occur?"  — concept query
```

```xpath
//body/expression[call]       // "what occupies expression-statement position
                              //  where the operand is a call?"  — position query
//argument/expression[call]   // "which arguments are calls?"
//return/expression[call]     // "which returns return a call?"
```

Modifiers attach as predicates on the host:

```xpath
//expression[await]
//expression[try]
//body/expression[call][try]
```

#### Audit candidates

Currently per-modifier wrappers, to migrate to host + marker:

- Rust: `try_expression` → `<try/>`; `await_expression` → `<await/>`;
  `reference_type` (`<ref>` for `&T`) — borderline, see follow-up.
- JS / TS / Python / C#: `await_expression` → `<await/>`.
- TypeScript: `non_null_expression` → `<non_null/>`.
- C#: `<conditional/>` (`?.`) is already a marker on `<member>`;
  align surrounding shape.
- C-family `cast` expressions stay as `<cast>` with a `<type>`
  child (constructs, not annotations).

C# already preserves `expression_statement` as `<expression>`;
TypeScript and Rust currently drop it. Migration unifies the
existing precedent across all languages.

**Cites:** Principle #15 (stable marker locations), Principle #11
(operand keeps its specific name inside the host), Principle #13
(host removes the temptation to put markers on text-only leaves),
Principle #7 (modifier markers extend the declaration pattern to
expressions), Goal #6 (broad-to-narrow — `//call` stays broad),
Principle #16 (verbosity in deep one-off expressions is acceptable).

**Deferred / open question:** the more radical "kind as marker"
shape — `<expression><call/><callee>foo</callee>...</expression>`
without a named `<call>` element — works for multi-discriminator
elements (already used by `<type>`) but flattens structural grouping
for primary-kind elements like calls and members. Tracked as a
separate experiment; not part of this decision.

*See child specs for language-specific and feature-specific decisions.*
