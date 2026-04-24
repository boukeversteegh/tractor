---
title: Semantic Tree Transformations
priority: 1
---

Index of per-language transformation decisions. Each language's
transform pass turns tree-sitter's raw syntax tree into tractor's
semantic tree by renaming, flattening, wrapping, marking, and
restructuring nodes. This document is a map; the per-language files
under `transformations/` carry the detail.

## How to read a per-language file

Each file follows the same shape:

1. **Summary** — one paragraph on the language's overall shape
   (any language-specific quirks worth knowing up front).
2. **Element names** — table of tree-sitter kinds → semantic element
   names, with rationale referencing the relevant principle/goal
   from [design.md](design.md).
3. **Structural transforms** — flattens, marker insertions,
   contextual rewrites that aren't pure renames.
4. **Language-specific decisions** — choices that depart from the
   common pattern, with explicit rationale.
5. **Open questions / flagged items** — places where the naming is
   still unsettled and awaiting user input.

## Languages

- [Java](transformations/java.md)
- [TypeScript / JavaScript](transformations/typescript.md)
- [Python](transformations/python.md)
- [C#](transformations/csharp.md)
- [Go](transformations/go.md)
- [Rust](transformations/rust.md)
- [Ruby](transformations/ruby.md)
- [PHP](transformations/php.md)

## Cross-cutting conventions

A handful of conventions apply across every programming-language
transform. They're documented here to avoid repeating them in every
per-language file.

### Field wrapping → semantic element

The builder records tree-sitter's field name as a `field="X"`
attribute on the child element. The per-language `FIELD_WRAPPINGS`
table (in `languages/mod.rs`) maps a tree-sitter field name to a
semantic wrapper element name. The common set:

| Field | Wrapper |
|---|---|
| `name` | `<name>` |
| `value` | `<value>` |
| `left` | `<left>` |
| `right` | `<right>` |
| `body` | `<body>` |
| `condition` | `<condition>` |
| `consequence` | `<consequence>` |
| `alternative` | `<alternative>` |

Per-language additions:

| Language | Field | Wrapper | Rationale |
|---|---|---|---|
| TS/JS | `return_type` | `<returns>` | Canonicalise to match C# (Principle #5). |
| Rust | `return_type` | `<returns>` | Same. |
| Go | `result` | `<returns>` | Same concept, Go-specific field name. |
| C# | `returns` | `<returns>` | Already canonical. |
| TS/JS | `function` | `<callee>` | Distinguish call target from function declaration (avoid `<function>` collision). |
| TS/JS | `object` / `property` | `<object>` / `<property>` | Member expression roles. |

### Flat lists (Principle #12)

Purely-grouping wrappers get dropped, and their children become
siblings of the enclosing element, carrying `field="<plural>"`.
Covered uniformly across languages:

- `parameter_list` / `formal_parameters` / `parameters` → children become siblings with `field="parameters"`.
- `argument_list` / `arguments` → children become siblings with `field="arguments"`.
- `type_arguments` / `type_argument_list` → children become siblings with `field="arguments"` (inside a generic `<type>`).
- `type_parameter_list` / `type_parameters` → children become siblings with `field="generics"`.
- `attribute_list` (C#) → `field="attributes"`.
- `accessor_list` (C#) → `field="accessors"`.

### Identifier handling (Principle #14)

All languages now trust tree-sitter's token distinction:

- `identifier`, `property_identifier`, `shorthand_field_identifier`,
  `field_identifier` → `<name>` (value namespace).
- `type_identifier`, `primitive_type`, `predefined_type`,
  `integral_type`, `floating_point_type`, `boolean_type`,
  `void_type` → `<type>` (type namespace).

C# is the historical outlier; its classifier has been simplified
to match.

### Interface defaults (spec-level)

Members declared inside an interface default to `<public/>` rather
than the enclosing class/struct default. C# and Java both enforce
this (C# spec §18.4, Java spec §9.4). See each language's decision
file for mechanics.

### Generic type references

Pattern established by C#, now applied across TS/JS, Java, Rust,
Python:

```xml
<type>
  <generic/>           <!-- marker: this type is generic -->
  Name                 <!-- the generic's name, as text -->
  <type field="arguments">Arg1</type>
  <type field="arguments">Arg2</type>
</type>
```

### Return types

`<returns>` wrapper containing one `<type>` (single-return
languages) or a sequence of `<type>` siblings (Go multi-return).

### Conditional shape (`<if>` / `<else_if>` / `<else>`)

A conditional is rendered flat: the root `<if>` holds the primary
condition and then-branch, followed by zero or more `<else_if>`
sibling branches, followed by an optional `<else>` default.

```
if[condition][then][else_if[condition][then]][else_if[condition][then]][else]
```

XML form for a 4-branch chain:

```xml
<if>
  <condition>x > 0</condition>
  <then>return "positive"</then>
  <else_if>
    <condition>x == 0</condition>
    <then>return "zero"</then>
  </else_if>
  <else_if>
    <condition>x < -10</condition>
    <then>return "very negative"</then>
  </else_if>
  <else>return "negative"</else>
</if>
```

Cites: Goal #5 (match the mental model — a JS dev reads `else if (…)`
as a continued conditional branch, not "an `else` whose body happens
to be an `if`"), Principle #12 (don't keep structural wrappers that
add no semantic meaning — the nested `else_clause[if[…]]` chain is
one such wrapper), Principle #1 (use language keywords — `else`).

**Naming notes:**
- `<then>` replaces the old `<consequence>` field wrapper.
  "Consequence" is functional-language jargon; no mainstream
  imperative developer says it.
- `<else_if>` uses underscore because XML element names cannot
  contain spaces and XPath parses `-` as minus (naming-conventions
  spec). Written out in full rather than as `elif` per Principle #2
  (full names over abbreviations).
- `<else>` replaces the old `<alternative>` field wrapper — again,
  the language's own keyword.

**Per-language mechanics:**

- **Python** — tree-sitter already emits `elif_clause` as a flat
  sibling of `if_statement`; rename to `<else_if>`. Python's
  `else_clause` renames to `<else>`. `consequence` field → `<then>`.
- **Ruby** — `elsif_clause` → `<else_if>`; `else_clause` → `<else>`.
- **JS / TS / Java / C# / Go / Rust** — tree-sitter produces a
  nested chain: `if_statement` whose `alternative` field is an
  `else_clause` whose sole child is another `if_statement`. The
  transform collapses this chain in-place: the nested `if`'s
  condition and then-branch are lifted as an `<else_if>` sibling
  of the outer `<if>`, recursively.

### Return types

### Trailing comments — adopt into predecessor (proposed)

Tree-sitter emits every comment as a top-level sibling of its
enclosing block. For free-floating / leading comments this is
correct — the comment precedes the thing it annotates and a
sibling is the right structural position.

**Trailing** comments (those on the same line as the end of the
preceding statement / declaration) are different: they annotate
the statement that comes *before*, and the reader thinks of them
as "attached" to that statement. The current structural shape
leaves them as a sibling, which forces queries that care about
the attached comment to rely on sibling-position scans.

**Current shape**:

```
<method public>
  <returns><type>…</type></returns>
  <name>perimeter</name>
  "();"
</method>
<comment>// implicitly public</comment>
```

**Proposed**: classify a comment as `<trailing/>` (same-line-as-
predecessor, which C# already does) AND *adopt it into the
predecessor element as a final child*:

```
<method public>
  <returns><type>…</type></returns>
  <name>perimeter</name>
  "();"
  <comment trailing>// implicitly public</comment>
</method>
```

Benefits:

- `//method[@public]/comment` reliably finds every trailing
  comment on a public method, with no sibling-index gymnastics.
- A statement's XPath string-value round-trips the trailing
  comment as part of the statement's own text.
- Aligns visual code layout with structural layout — readers
  think of the comment as part of the statement, so the tree
  mirrors that mental model.

Open questions for implementation:

- **Leading block comments** that precede a declaration — stay as
  a preceding sibling (current C# behaviour) or adopt into the
  declaration as the first child? The two forms are distinguishable
  via `<leading/>` markers, but the structural home of the comment
  is another decision.
- **Floating comments** (separator comments, no adjacent code):
  stay as siblings under the enclosing block. Nowhere else
  sensible to put them.
- **Multi-line trailing**: a line comment followed by more line
  comments on the next lines — the C# grouping pass already merges
  adjacent `//` comments; the adopted trailing form inherits that
  merging.
- **Grouping scope**: "predecessor" means immediately-preceding
  element sibling under the same parent. If the preceding sibling
  is a text token (e.g. `;`), adoption happens into the element
  that owns that text.

Cross-cutting — applies to every programming language once
implemented. C#'s `is_inline_node` / `is_leading_comment` helpers
already compute the classification; the adoption step is
additional.

### Multi-line comment grouping — prefix-stripped `<line>` children (proposed)

C#'s current grouping pass merges consecutive `//` line comments
on adjacent lines into one `<comment>` element, with the joined
text as a single string (prefixes included):

```
<comment>// first line
// second line
// third line</comment>
```

That preserves source text but the consumer has to strip the
`//` prefixes themselves and split on newlines to recover the
comment body as structured data. JSON serialisation produces a
single blob string rather than an array of lines.

**Proposed**: keep the `//` / `#` / `--` prefixes as dangling text
between `<line>` element children. Each `<line>` holds the
prefix-stripped text for one line.

```
<comment>
  "// "
  <line field="lines">first line</line>
  "// "
  <line field="lines">second line</line>
  "// "
  <line field="lines">third line</line>
</comment>
```

- Source reconstruction: concatenating all descendant text still
  yields `// first line\n// second line\n// third line` — the
  prefix text stays in place, the `<line>` element's string value
  is its inner text, no characters lost.
- Query: `//comment/line` returns the clean line bodies directly,
  no per-language prefix knowledge required. `//comment[line[.='TODO']]`
  finds every block with a line equal to `TODO`.
- JSON: `field="lines"` on each `<line>` promotes to an array
  under the comment: `{"comment": {"lines": ["first line", "second line", ...]}}`.
- Single-line comments: one `<line>` child (consistent shape —
  readers don't need to special-case one-versus-many).

Cross-language coverage:

- `//` — JavaScript, TypeScript, Rust, Go, Java, C#, PHP, Swift.
- `#` — Python, Ruby, PHP (rare), shell.
- `--` — SQL (including T-SQL), Lua.

Each language's prefix is known; the grouping pass strips it
uniformly. Block comments (`/* ... */`, `"""…"""`) don't get
split into lines — they stay as one text leaf or one `<line>`
since the reader already sees them as a unit.

Open questions:

- **Docstring-flavoured block comments** (Rust `///`, Java `/**`)
  carry structure (parameter tags, return tags) inside. Keeping
  them as one `<line>` leaves that structure hidden. Defer —
  treat as a separate "doc-comment shape" cycle.
- **Interaction with trailing-comment adoption** — a multi-line
  trailing comment adopted into its predecessor still has the
  `<line>` children; nothing special. A single-line trailing
  comment (most common) is `<comment trailing><line>…</line></comment>`.
- **Grouping window** — current C# pass only merges when lines
  are strictly adjacent (no blank line between). Keep that rule.

Cross-cutting, post-dates the trailing-comment adoption work so
the adopted form inherits the `<line>` substructure automatically.

## Relationship to `transform-rules/`

The older `transform-rules/` folder documents generic, pattern-level
transformations (lift-modifiers, flatten-declaration-lists, etc.)
that apply across languages. `transformations/` documents the
per-language *decisions* — which specific tree-sitter kinds are
affected, which names are chosen, and why.
