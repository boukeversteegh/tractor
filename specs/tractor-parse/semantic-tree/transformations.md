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

### Comments — classification and grouping

Comments live as siblings of the code they relate to. Three
structural concerns apply uniformly across programming languages.

**Leading / trailing classification (landed).** Each `<comment>`
may carry one of two attachment markers:

- `<trailing/>` — the comment sits on the same line as the end of
  the preceding sibling. It annotates that statement.
- `<leading/>` — the comment (or a merged group of line comments)
  is immediately followed by a non-comment sibling on the very
  next line. It annotates the next declaration.
- (no marker) — free-floating / standalone comment.

```xml
<field><type>int</type><name>x</name></field>
<comment trailing>// trailing</comment>

<comment leading>// describes Foo
// continued</comment>
<class><name>Foo</name>…</class>

<comment>// floating section divider</comment>
```

**Adjacent line-comment grouping (landed).** Consecutive line
comments (same prefix, no blank-line gap between them) merge into
a single `<comment>` element with multiline text content; the
merged node is then classified as a single unit.

Implementation: shared `CommentClassifier` in
`tractor/src/languages/comments.rs`, parameterised by line-comment
prefix list (`["//"]`, `["#"]`, or both for PHP). Each language's
`transform.rs` delegates. Coverage: C#, Java, TypeScript / JS,
Rust, Go, Python, Ruby, PHP. tsql inherits when comments become
relevant.

**Trailing comment adoption — deferred.** Even with the marker, a
trailing comment lives as a *sibling* of its predecessor, not a
child. Adopting it into the predecessor as a final child would
make `//method[public]/comment` find trailing comments without
sibling-index gymnastics, but raises an unsettled question for the
leading counterpart (does a leading comment become the first child
of the next declaration?). Held until the leading-anchor question
is explored. See [`semantic-tree-open-questions.md`](../../../todo/semantic-tree-open-questions.md).

**Multi-line `<line>` children — deferred.** A merged group is
currently one `<comment>` with multiline text (prefixes embedded).
A future evolution would split the prefix-stripped body into
`<line>` children with prefix text preserved as siblings, so JSON
serialises as `{"comment": {"lines": [...]}}`. Held until trailing
adoption is decided so the two evolve together.

### Tree-sitter kind catalogue

Each language exposes `KINDS: &[KindEntry]` in its `semantic`
module — the authoritative list of every tree-sitter kind the
transform recognises and how it is handled. `KindHandling`
variants:

| Variant | Meaning |
|---|---|
| `Rename(target)` | Pure rename: kind X → semantic name Y, no marker. |
| `RenameWithMarker(target, marker)` | Rename + prepend `<marker/>`. |
| `Custom` | Imperative dispatch arm in `transform.rs` (structural transform, conditional logic). |
| `CustomThenRename(target)` / `CustomThenRenameWithMarker(t, m)` | Imperative work followed by a deferred rename via the catalogue. |
| `Flatten` | Wrapper dropped, children promoted to siblings (Principle #12). |
| `PassThrough` | Kind passes through unchanged. |

Per-language `map_element_name(kind)` is a 3-line delegate that
reads back from `KINDS` — the catalogue is the single source of
truth, no duplication between catalogue and dispatcher.

The lint test `tractor/tests/kind_catalogue.rs` parses each
blueprint fixture, walks the raw tree-sitter parse, and asserts
every distinct kind appears in the catalogue. Tree-sitter grammar
upgrades that introduce new kinds fail this lint with a clear
pointer to the language's `semantic.rs`.

## Relationship to `transform-rules/`

The older `transform-rules/` folder documents generic, pattern-level
transformations (lift-modifiers, flatten-declaration-lists, etc.)
that apply across languages. `transformations/` documents the
per-language *decisions* — which specific tree-sitter kinds are
affected, which names are chosen, and why.
