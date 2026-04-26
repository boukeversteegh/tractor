---
title: Go Transformations
---

Per-node decisions for the Go transform
(`tractor/src/languages/go.rs`).

## Summary

Go has several grammar patterns that need special handling:

1. **`type_declaration` wrapper** — Go's `type X …` top-level
   declaration is wrapped by tree-sitter. We move the literal
   `type` keyword into the inner `type_spec` before flattening,
   so the keyword stays attached rather than floating as orphan
   text (see Go #10 design history).
2. **Overloaded `parameter_list`** — Go uses the same tree-sitter
   node for both formal parameters and multi-value return specs.
   Context (parent element) disambiguates.
3. **Overloaded `field_identifier`** — Go uses this kind for
   struct fields, method receivers, and method names on interfaces.
   Currently renamed to `<name>` to match the value-namespace
   convention.
4. **Exported/unexported markers** — based on the first character
   of the declared name (Go spec's visibility rule), the transform
   adds `<exported/>` or `<unexported/>`.

## Element names

| tree-sitter kind | semantic name | rationale |
|---|---|---|
| `source_file` | `<file>` | Short; matches Go's spec term. |
| `package_clause` | `<package>` | Language keyword. |
| `function_declaration` | `<function>` | Language keyword. |
| `method_declaration` | `<method>` | Developer mental model. |
| `method_elem` | `<method>` | Interface method; unified with declaration-level method. |
| `type_declaration` | flattened (after keyword move) | See structural transforms. |
| `type_spec` | `<type>` | Go's own vocabulary — `type X Y` declares a type. |
| `struct_type` | `<struct>` | Language keyword. |
| `interface_type` | `<interface>` | Language keyword. |
| `const_declaration` | `<const>` | Language keyword. |
| `var_declaration` | `<var>` | Language keyword. |
| `short_var_declaration` | `<variable>` with `<short/>` marker | Distinguishes `x := 42` from `var x = 42`. |
| `parameter_declaration` | `<param>` | Short; matches other languages. |
| `pointer_type` | `<pointer>` | Language concept. |
| `slice_type` | `<slice>` | Language concept. |
| `map_type` | `<map>` | Language keyword. |
| `channel_type` | `<chan>` | Language keyword. |
| `return_statement` | `<return>` | Language keyword. |
| `if_statement` | `<if>` | Language keyword. |
| `else_clause` | `<else>` | Language keyword; chain collapsed — see below. |
| `for_statement` | `<for>` | Language keyword. |
| `range_clause` | `<range>` | Language keyword. |
| `switch_statement` | `<switch>` | Language keyword. |
| `case_clause` | `<case>` | Language keyword. |
| `default_case` | `<default>` | Language keyword. |
| `defer_statement` | `<defer>` | Language keyword. |
| `go_statement` | `<go>` | Language keyword. |
| `select_statement` | `<select>` | Language keyword. |
| `call_expression` | `<call>` | Matches other languages. |
| `selector_expression` | `<member>` | Matches C#/Java/TS. |
| `index_expression` | `<index>` | Language concept. |
| `composite_literal` | `<literal>` | Language-neutral term. |
| `binary_expression` | `<binary>` | Consistent. |
| `unary_expression` | `<unary>` | Consistent. |
| `interpreted_string_literal` | `<string>` | Language concept. |
| `raw_string_literal` | `<string>` with `<raw/>` marker | Exhaustive marker (Principle #9 partial — raw is non-default; plain strings stay bare). |
| `int_literal` | `<int>` | Language keyword. |
| `float_literal` | `<float>` | Language concept. |
| `true`, `false` | `<true>`, `<false>` | Language keywords. |
| `nil` | `<nil>` | Language keyword. |
| `field_declaration` | `<field>` | Matches other languages. |
| `field_identifier` | `<name>` | Namespace vocabulary (Principle #14). Previously `<field>` which created a collision with `<field>` declaration elements. |
| `package_identifier` | `<name>` | Namespace vocabulary. |
| `import_declaration` | `<import>` | Language keyword. |

## Structural transforms

### Flatten (Principle #12)

- `expression_statement` → Skip.
- `block` — structural.
- `field_declaration_list` — the `{ field; field; }` inside a
  struct; drop so fields become direct children.
- `expression_list` — comma-separated expression group (e.g.
  `return x, y`); drop so individual expressions are siblings.
- `import_spec` — wrapper inside an `import ( … )` block; drop so
  each import path is a direct child of `<import>`.
- `interpreted_string_literal_content` — the content-inside-quotes
  node; flatten to inline text into the enclosing `<string>`.

### `type_declaration` + keyword preservation

Tree-sitter's shape:

```
type_declaration
  "type"                   (literal keyword text)
  type_spec                (or multiple, in `type ( … )` block form)
    name: ...
    type: struct_type | interface_type | ...
```

The transform calls `move_type_keyword_into_spec(decl)` to
relocate the literal `"type"` text into the inner `type_spec`,
then flattens the outer wrapper. Result: the keyword stays
attached to the `<type>` element (useful for the renderer / text
view) rather than floating as an orphan sibling at file level.

### Flat lists with context awareness

- `parameter_list` — dual use:
  - If parent is `<returns>` (via the `result`→`returns` field
    canonicalisation): call `collapse_return_param_list`, which
    rewrites each `parameter_declaration` to just its inner
    type. Result: `<returns><type>int</type><type>error</type></returns>`
    reads as a sequence of types, not a sequence of params.
  - Otherwise (formal parameters): distribute `field="parameters"`.
  - In both cases, flatten the wrapper.
- `argument_list` → children get `field="arguments"`.

### Return type canonicalisation

Go tree-sitter uses `field="result"` for return types (single or
multi). The builder's `canonical_field_name` maps `result` →
`returns`, producing a `<returns>` wrapper consistent with every
other language.

### Export markers

`function_declaration`, `method_declaration`, and `type_spec`
get `<exported/>` or `<unexported/>` based on the first
character of the declared name (Go spec — capital-letter prefix
means exported from the package). Not a syntactic modifier in
the source, so these markers don't have source locations.

### Short variable declarations

```xml
<variable><short/>...</variable>         <!-- from `x := 42` -->
<variable>...</variable>                 <!-- from `var x = 42` -->
```

The `<short/>` marker is an exhaustive indicator of the short form
(Principle #9); the standard `var` form is unmarked because
there are multiple declaration forms (`var`, `const`) that share
the `<variable>` shape.

### Raw string marker

`r"..."` (raw strings) — the transform renames `raw_string_literal`
to `<string>` and prepends a `<raw/>` marker. Mirrors the pattern
used in Rust.

## Language-specific decisions

### `<method>` for both declaration and interface member

Tree-sitter distinguishes `method_declaration` (a concrete
implementation) from `method_elem` (an interface method spec).
Both are "a method" in the developer's mental model (Goal #5) —
the difference (has-body vs no-body) is visible from the
`<body>` child's presence. Unifying to `<method>` simplifies
queries.

### `<field_identifier>` → `<name>` (not `<field>`)

A field_identifier in Go can appear in several places:
- As the name of a struct field declaration.
- As the property being accessed in a selector expression (`obj.Field`).
- As the name of an interface method.

All of these are identifiers in the value namespace (Principle
#14), so they all become `<name>`. An earlier draft renamed
field_identifier to `<field>`, which collided with the `<field>`
declaration element — that's why the rename to `<name>` was made.

### `interpreted_string_literal_content` flatten

The content-inside-quotes node adds no useful nesting — a
`<string>` should just contain its text. Flattening inlines it.
Applied uniformly.

### Keeping `<type>` for Go's type definitions

Go's `type X int` creates a new *defined type* (per the Go spec;
distinct from the alias form `type X = int`). Because the thing
being declared *is literally a type* in Go's vocabulary, using
`<type>` as the declaration element name matches the developer's
speech (`type` is the actual keyword). Principle #14's rule —
"type declarations have their own element name" — is satisfied
because `<type>` *is* a specific element; it just happens to also
be Go's spec term. Disambiguated from type *references* by having
a `<name>` child.

### Conditional shape (`<if>` / `<else_if>` / `<else>`)

Go's `if_statement` grammar uses the same nested `else_clause`
shape as the other C-like languages. The transform:

- `GO_FIELD_WRAPPINGS` maps `consequence` → `<then>` and
  `alternative` → `<else>`; the `else_clause` kind renames to
  `<else>` via `map_element_name`.
- The post-walk `collapse_else_if_chain` (registered for Go in
  `languages/mod.rs`) unwraps the redundant `<else><else>` and
  lifts each inner `<if>`'s condition/then into an `<else_if>`
  sibling, recursively.

See the cross-cutting "Conditional shape" convention in the index
[`transformations.md`](../transformations.md).


## Open questions / flagged items

### Imports — grouping per spec (needs redesign)

Currently `import_spec_list` AND `import_spec` both Flatten, producing:

```
<import>
  "import ("
  <string>"context"</string>
  <string>"errors"</string>
  <string>"io"</string>
  <name>myio</name>
  <string>"io"</string>       <!-- which path goes with "myio"? -->
  <name>.</name>
  <string>"strings"</string>  <!-- which path goes with "."? -->
  <name>_</name>
  <string>"net/http/pprof"</string>
  ")"
</import>
```

Every alias / dot / blank import is a logical pair `(name, path)`
but the names and paths are adjacent siblings of `<import>`, with
no structural grouping. `//import/string` returns every path
without indicating which alias applies, so queries like "find
every blank import" or "find all dot imports for package X"
require relying on document order rather than structure.

**Proposed shape — make each spec a queryable unit:**

```
<import>
  <spec><string>"context"</string></spec>
  <spec><string>"errors"</string></spec>
  <spec><name>myio</name><string>"io"</string></spec>
  <spec><name>.</name><string>"strings"</string></spec>
  <spec><name>_</name><string>"net/http/pprof"</string></spec>
</import>
```

Or (alternative), keep the name-is-path-wrapper by inverting:

```
<import>
  <string>"context"</string>
  <import alias="myio"><string>"io"</string></import>
  <import dot><string>"strings"</string></import>
  <import blank><string>"net/http/pprof"</string></import>
</import>
```

The `<spec>` form is more consistent with other languages'
import-spec handling. The alternative form uses markers
(`<dot/>` / `<blank/>`) and attributes; markers compose better
with the shape-marker convention used elsewhere.

**TODO:** decide between the two forms, then:
1. Stop flattening `import_spec` for Go — keep as `<spec>` (or
   inner `<import>` wrapper).
2. Add `<dot/>` / `<blank/>` / `<alias/>` markers for the three
   non-plain forms (queries like `//import[blank]/string`).
3. Handle the single-import case `import "context"` consistently
   (no spec wrapper, or always wrapped — pick one).

Related: the blueprint + sample fixtures and `update_snapshots`
will surface via the snapshot diff.

### Const / var blocks — same grouping problem as imports

`const_spec` and `var_spec` are also currently flattened, producing
the same shape pathology. Example from the Go blueprint:

```
<const>
  "const ("
  <name>StatusIdle</name>
  "="
  <value><iota>iota</iota></value>
  <name>StatusRunning</name>    <!-- implicit iota continuation -->
  <name>StatusDone</name>
  <name>_</name>                <!-- blank identifier (discard) -->
  <name>StatusError</name>
  ")"
</const>
```

Every spec in a `const (…)` or `var (…)` block is a logical
`name [= value]` unit. In Go specifically, `const` blocks with
`iota` have *implicit continuation*: after the first `= iota`, each
subsequent name inherits the expression (and `iota` auto-increments).
The current shape loses that relationship entirely — a reader /
query can see the names and some values, but not which name gets
which value.

**Proposed shape** — the same `<spec>` grouping as imports:

```
<const>
  <spec><name>StatusIdle</name> = <value><iota/></value></spec>
  <spec><name>StatusRunning</name></spec>    <!-- implicit iota -->
  <spec><name>StatusDone</name></spec>
  <spec><name>_</name></spec>
  <spec><name>StatusError</name></spec>
</const>
```

Queries that benefit: `//const/spec[iota]` (every spec with an
explicit `iota`), `//const/spec[not(value)]` (every spec relying on
implicit continuation), `//var/spec[name='cfg']/value` (value for a
specific name without relying on sibling indexing).

**TODO (grouped with the imports redesign above):**
1. Stop flattening `const_spec` / `var_spec`; keep as `<spec>`.
2. Consider whether `<spec>` generalises across imports + const +
   var (same element name, same role), or whether each family gets
   a distinct wrapper (`<import>`/`<spec>`).
3. If we stay with flattening, at minimum emit a `<continuation/>`
   marker on specs that inherit the previous expression, so the
   implicit-iota case is queryable.

### Var blocks with tuple assignment

Same flattening problem, extra pathology — Go's `var` blocks can
mix four different shapes:

```go
var (
    ErrNotFound = errors.New("not found")  // name = value
    globalCount int                         // name type (no value)
    name, age   = "alice", 30               // names , = values ,
)
```

Currently renders as a flat soup of `<name>`, `<type>`, and value
children under `<var>` with commas and `=` as text siblings — the
multi-assignment form loses the name↔value positional pairing
entirely.

The `<spec>` grouping proposed above handles the first two forms
cleanly. The tuple-assignment form needs either:

- **Option A — one spec per declared name**: split
  `name, age = "alice", 30` into two `<spec>`s, re-pairing each
  name with its corresponding value. The source text stays intact
  as dangling siblings of the `<var>` so text-preservation holds,
  but the XML shape shows the intent:

  ```
  <spec><name>name</name><value><string>"alice"</string></value></spec>
  <spec><name>age</name><value><int>30</int></value></spec>
  ```

- **Option B — one spec with multi-name + multi-value**: keep the
  source shape verbatim, with the spec containing lists:

  ```
  <spec>
    <name>name</name>, <name>age</name> =
    <value><string>"alice"</string></value>,
    <value><int>30</int></value>
  </spec>
  ```

Option A is more queryable (every value has exactly one name
sibling) but loses the syntactic grouping. Option B preserves the
Go source shape but requires sibling-position indexing to recover
the pairing. Pick when the redesign lands.

## Comments

Go uses the shared `CommentClassifier`
(`tractor/src/languages/comments.rs`) with `["//"]` as the line
prefix. `comment` (Go's tree-sitter emits a single kind for both
`//` and `/* */`) renames to `<comment>` and gets a `<trailing/>`
or `<leading/>` marker per the cross-cutting rules (see
[`transformations.md`](../transformations.md) — *Comments*).
Adjacent `//` comments merge into a single `<comment>`.
