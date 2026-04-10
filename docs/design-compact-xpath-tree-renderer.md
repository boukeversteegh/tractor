# Compact XPath Tree Renderer

## Goals

- Provide a compact, query-oriented tree view for `--format text`.
- Make the rendered tree easier to read than XML while still reflecting the real XML structure.
- Help users mentally form XPath queries from the displayed output.
- Preserve all query-relevant structure in the text view.
- Keep tree guides and indentation purely presentational.

## Non-goals

- Do not change the underlying XML tree model.
- Do not change XML, JSON, or YAML output formats.
- Do not invent semantic summaries that are not present in the source tree.
- Do not hide queryable structure unless it is covered by explicit collapse rules.
- Do not become a language-specific pretty-printer.

## Design Principles

### 1. Text mode is a view over the XML tree

The compact renderer exists only to improve readability in text mode. It must not redefine the canonical tree shape.

### 2. Semantic fidelity over prettiness

Every semantic segment shown in the compact view must correspond to a real XML node or a real text node. If there is a tradeoff between brevity and faithfulness, choose faithfulness.

### 3. Deterministic collapse rules

Collapsing is allowed only when it follows explicit structural rules. The renderer should not apply ad hoc heuristics that make output context-dependent or hard to predict.

### 4. XPath-oriented notation

The text form should look closer to how users think about XPath than how XML source is serialized.

That means:

- predicates use `[...]`
- linear paths use `/`
- leaf values use ` = "..."` rather than XML tags

### 5. Presentation chrome is not query syntax

Indentation, branch glyphs, and continuation guides are only for readability. They are not XPath and should never be treated as such in code or docs.

### 6. Separation of concerns

The compact tree renderer must remain separate from the XML renderer. Text-mode readability changes should not affect canonical XML serialization.

## Scope

The compact renderer is used only when text mode renders a tree field:

- `-f text` with `-v tree`
- text-mode tree fallback paths in check/test rendering

All non-text formats continue to use their existing format-specific serializers.

## Output Model

Each rendered line represents one of:

- an element node
- a collapsed linear path of element nodes
- an element/path with collapsed predicates
- an element/path with an inline leaf value
- a raw text literal child
- a truncation placeholder when `--depth` prevents descent

Example:

```text
variable[const]/
  ├─ name/type = "API_URL"
  └─ value/string/
      ├─ "\""
      ├─ string_fragment = "http://localhost:3000"
      └─ "\""
```

## Rendering Rules

### 1. Base segment

An element starts as its XML element name:

```text
variable
```

### 2. Predicate collapse

Predicates come from two sources:

- marker children
- visible attributes

Marker children are structurally empty child elements:

- element node
- no visible attributes
- no non-whitespace text
- no child elements

Markers and attributes collapse onto the same segment and are combined into a single predicate block joined by ` and `:

```text
method[public and static and @kind="method_declaration" and @line="19"]
```

This keeps the output compact while preserving full predicate information.

### 3. Linear path collapse

Parent/child chains collapse with `/` when:

- each node has exactly one non-marker child element
- the current node has no other visible children
- collapsing does not hide branching information

Examples:

```text
name/type = "API_URL"
returns/type = "void"
value/call[ref] = "x"
```

### 4. Leaf value collapse

If a node or collapsed path ends in a leaf text value, render it inline:

```text
type = "string"
comment = "// hello"
name/type = "fs"
```

Displayed values use JSON-style quoted strings so escaping is stable and unambiguous.

### 5. Child indicator

If an entry still has visible children after collapsing, append `/`:

```text
class[public]/
body/
```

### 6. Raw text children

Meaningful raw text nodes that remain visible render as quoted literals:

```text
"class"
"{"
"}"
```

Whitespace-only formatting text introduced by XML pretty-printing is ignored.

## Tree Layout

Tree guides are presentation-only.

Rules:

- first-level children start with two spaces
- branch markers are `├─` and `└─`
- ancestor columns keep `│` while later siblings still exist
- otherwise ancestor columns become spaces

Example:

```text
class[public]/
  ├─ "class"
  └─ body/
      ├─ "{"
      ├─ property[public]/
      │   └─ ... (6 children)
      ├─ method[public]/
      │   └─ ... (7 children)
      └─ "}"
```

## Depth Limiting

`--depth` still applies in text mode.

When the renderer reaches the depth limit for a node that still has visible descendants, it emits a truncation placeholder instead of descending further:

```text
body/
  └─ ... (15 children)
```

The displayed count is the number of descendant element nodes below the truncated entry.

## Metadata Rendering

When `--meta` is enabled, visible XML attributes are rendered as attribute predicates:

```text
method[@kind="method_declaration" and @line="19" and @column="9"]
```

Without `--meta`, location and other internal metadata attributes remain hidden in text mode.

## Design Decisions

### Decision 1: Collapse predicates into one block

Multiple markers and attributes are emitted inside one `[...]` block joined by ` and ``, rather than as repeated adjacent predicate blocks.

Before:

```text
method[@kind="method_declaration"][@line="19"][@column="9"]
```

After:

```text
method[@kind="method_declaration" and @line="19" and @column="9"]
```

This keeps the output shorter and closer to how users naturally think about XPath predicates.

### Decision 2: Use JSON-style string quoting for displayed values

Displayed values and raw text literals use JSON-style quoting and escaping. This gives the renderer a stable, language-neutral display form for quotes, backslashes, tabs, and newlines without inventing a custom escaping scheme.

### Decision 3: Depth truncation counts descendant elements

When `--depth` truncates a subtree, the placeholder reports the number of descendant element nodes below that point. This gives users a compact indication of omitted structure without exposing XML serialization details or raw child counts that mix text and elements.

## Guarantees

- Every semantic segment maps to real XML nodes or real text nodes.
- Predicate markers come only from actual empty child elements and visible attributes.
- Slash-separated segments come only from real parent/child chains.
- ` = "..."` appears only when the rendered path ends at a real leaf text value.
- Tree guides and indentation are display-only.

## Implementation Structure

The renderer is intentionally split from canonical XML serialization:

- XML renderer: canonical XML output for XML-oriented formats
- compact tree renderer: text-only XPath-oriented view over `XmlNode`

This keeps the text renderer flexible without making XML output unstable.
