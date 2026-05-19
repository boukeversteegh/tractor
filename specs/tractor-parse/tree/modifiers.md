---
title: Modifiers as Empty Elements
priority: 1
---

Modifiers (access levels, keywords like static/async) are represented as empty
child elements rather than attributes or text nodes.

This enables intuitive XPath predicates using element existence:

```xml
<method>
  <public/>
  <static/>
  <async/>
  <name>FetchDataAsync</name>
</method>
```

XPath queries become natural language:
- `//method[public]` - find public methods
- `//method[static]` - find static methods
- `//method[async]` - find async methods
- `//method[public][static]` - find public static methods
- `//class[not(public)]` - find non-public classes
- `//param[this]` - find extension method parameters

Supported modifiers:

**Access modifiers:**
- `public`, `private`, `protected`, `internal`

**Other modifiers:**
- `static`, `async`, `abstract`, `virtual`, `override`
- `sealed`, `readonly`, `const`, `partial`
- `this` (for C# extension method first parameter)

**Python-specific:**
- `async` (for async def)

The empty element approach is chosen over attributes because:
1. `//method[public]` is more readable than `//method[@public='true']`
2. No need to remember attribute value formats
3. Naturally supports `not()` for negation

Modifiers with a corresponding source keyword carry source locations, and
mutually exclusive sets always include one marker (never use absence as default).
See [design principles #9 and #10](design.md#9-exhaustive-markers-for-mutually-exclusive-variations).

## Why markers, and why they stay empty

When a grammar concept has a small, closed, well-known set of values,
we render it as an empty marker element named after the value —
`<public/>` rather than `<visibility>public</visibility>`. Reasons:

- **Queries are shorter**: `//method[public]` over
  `//method[visibility='public']`.
- **Vocabulary match**: every developer knows the words `public`,
  `private`, `static`, `async`, `get`, `set`, `pub`, `mut`. Far fewer
  know the grammar's own meta-names (`visibility_modifier`,
  `accessor_declaration_kind`, `modifier`). Naming the marker after
  the value lets queries read like the source.
- **Values are the primary signal**: the name of the property would
  carry no information a reader doesn't already get from the value.

The rule only applies when the set of values is closed and known
ahead of time. Open-ended values (an identifier, a numeric literal,
a path) keep the named-property form because the reader needs the
name to know what the string means.

## Consequence: markers stay empty, source keyword dangles as sibling

We serialize to JSON / YAML / XML by rendering the AST as-is — no
per-format rewriting. In JSON and YAML, an empty element `<public/>`
collapses to a boolean flag on the parent (`"public": true`); a
non-empty element `<public>public</public>` would collapse instead
to a property with a value (`"public": "public"`), which is exactly
the named-property shape we chose markers to avoid.

To preserve both properties at once — the JSON flag shape **and**
source-accurate XPath string-value so `-v value` returns the source
text unchanged — the transform lifts the source keyword out of the
marker and leaves it as a dangling text node in the parent:

```xml
<!-- tree-sitter input -->
<class_declaration>
  <modifier>public</modifier>
  class
  <name>Foo</name>
  …
</class_declaration>

<!-- tractor semantic tree -->
<class>
  <public/>public        <!-- empty marker + dangling source keyword -->
  class
  <name>Foo</name>
  …
</class>
```

Querying the class:

- `//class[public]` — marker flag, clean and short.
- XPath `string()` of the class walks every descendant text, including
  the dangling `"public"` sibling → returns `"public class Foo …"`.
- JSON `-p tree -f json` renders `<public/>` as `"public": true`;
  the dangling text next to a structural child is discarded, which
  matches the design intent: JSON consumers want the flag.
- Tree view shows the marker and the source keyword as separate
  siblings — slight visual redundancy, but matches how a reader
  thinks about `public` (both a queryable attribute and a literal
  source token).

This is a design choice, not a principle. The alternative —
`<public>public</public>` with renderers special-casing "text
equals element name" as a marker — is more compact in the tree
view but introduces a hidden contract between three renderers
that must agree forever on what counts as marker-shaped. We prefer
the dangling-sibling form because it keeps the rule local to one
place (the transform) and lets every renderer see the tree as it is.

### Which keywords get this treatment

Every source-backed marker: `<public/>` / `<private/>` / `<static/>` /
`<async/>` / `<pub/>` / `<mut/>` / `<get/>` / `<set/>` / `<this/>` etc.
Inferred markers (`<required/>`, `<optional/>`, `<literal/>`,
`<comprehension/>`, implicit `<package/>` on Java, implicit
`<private/>` on Rust — the marker that fires when no source keyword
was present) have no source text to preserve and therefore no
dangling sibling.

### Follow-up

The dangling text node currently lives next to the marker rather
than inside it — a reader who wants to jump from the marker to its
originating source token has to scan siblings. Not ideal, but not
a blocker for v1.
