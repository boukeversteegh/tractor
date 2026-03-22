# Language-Agnostic Patching Architecture

This document describes the architecture for modifying values in data files
(JSON, YAML, TOML, etc.) using XPath expressions, without requiring
language-specific splicing logic. The same algorithm handles both updates
(replacing existing values) and inserts (creating new structure).

## Core Principle

All knowledge of a language's syntax lives in two places:

1. **The parser** — which produces a data tree with source spans
2. **The renderer** — which produces syntactically valid source from a data tree

The patching algorithm itself is language-agnostic. It never inspects the
source text for braces, commas, quotes, or any other syntax. Instead, it
delegates to the renderer and uses re-parsing to locate the rendered output
within the generated source.

## Algorithm

Given a source string, a language, an XPath expression, and a value:

### Step 1: Parse and query

Parse the source into a data tree (with source location spans preserved).
Query the tree with the XPath expression.

- If the XPath matches a node, we are on the **update path**.
- If it does not match, we are on the **insert path**.

### Step 2: Identify the splice node

The *splice node* is the node whose source span will be replaced. Its
identity differs between update and insert:

- **Update**: The splice node is the matched node itself.
- **Insert**: Walk the XPath steps against the data tree to find the
  *deepest pre-existing ancestor* — the last node in the path that already
  exists. The splice node is this ancestor. The remaining XPath steps
  (those that did not match) define the structure that must be created.

In both cases, record the splice node's **original source span** (byte
range in the original source string).

### Step 3: Mutate the tree

Modify the data tree in memory:

- **Update**: Replace the splice node's value (its text content) with the
  new value.
- **Insert**: Create child nodes for each remaining XPath step, nesting
  them under the splice node. Set the leaf node's value to the new value.

### Step 4: Re-render

Render the **entire modified tree** back to source code using the
language's renderer. This produces a complete, syntactically valid source
string that contains the modification with correct formatting — quotes,
delimiters, escaping, indentation, and all other syntax concerns are
handled by the renderer.

### Step 5: Re-parse and re-query

Parse the re-rendered source into a new data tree. Query it with the
**same XPath expression**. The query must now match exactly the node that
was modified or inserted. Extract the **new source span** of this node
from the re-rendered source.

For the **insert path**, the splice node is the deepest pre-existing
ancestor, so re-query its XPath (not the leaf's) to find the re-rendered
span of the entire subtree that was modified.

### Step 6: Splice

Replace the **original source span** (from step 2) with the content at
the **new source span** (from step 5) extracted from the re-rendered
source.

```
result = original[..orig_start] + rerendered[new_start..new_end] + original[orig_end..]
```

The original file's formatting is preserved everywhere outside the splice
region. Inside the splice region, the renderer's formatting applies.

## Diagram

```
Original source          Data tree (xot)         Modified tree
 ┌──────────────┐       ┌──────────────┐       ┌──────────────┐
 │ { "name":    │ parse │ <File>       │ mutate│ <File>       │
 │   "Alice"   │──────>│  <name>      │──────>│  <name>      │
 │ }            │       │   Alice      │       │   Alice      │
 │              │       │  </name>     │       │  </name>     │
 │              │       │ </File>      │       │  <age>       │
 │              │       └──────────────┘       │   30         │
 │              │                               │  </age>      │
 │              │       record splice span:     │ </File>      │
 │              │       <File> at [1:1, 3:2]    └──────┬───────┘
 └──────────────┘                                      │
                                                 render│
                                                       ▼
 Spliced result          Re-parsed tree          Re-rendered source
 ┌──────────────┐       ┌──────────────┐       ┌──────────────┐
 │ { "name":    │splice │ <File>       │ parse │ {            │
 │   "Alice",  │<──────│  <name>      │<──────│   "name":    │
 │   "age": 30 │       │   Alice      │       │     "Alice", │
 │ }            │       │  </name>     │       │   "age": 30  │
 │              │       │  <age>       │       │ }            │
 │              │       │   30         │       └──────────────┘
 │              │       │  </age>      │
 └──────────────┘       │ </File>      │  query: find <File>
                        └──────────────┘  new span: [1:1, 4:2]
```

## Why re-render the full tree?

An alternative would be to render only the inserted fragment and splice it
in directly. That approach requires language-specific knowledge in the
patching logic:

- Where to insert (find closing delimiter in source)
- How to separate siblings (commas, newlines)
- How to format the fragment (indentation relative to context)
- How to strip wrapper syntax from the rendered fragment

By re-rendering the full tree, all of these concerns are handled by the
renderer, which already knows the language's rules. The patching algorithm
remains a simple span replacement.

## Why re-parse after rendering?

The renderer produces a complete source string, but we only want to splice
the *changed region* into the original source. We need to know the byte
range of the modified node in the rendered output. Re-parsing and
re-querying with the same XPath gives us this range via the source spans
that the parser attaches to every node.

## Splice node selection

The choice of splice node determines what gets replaced in the original
source and what byte range is extracted from the re-rendered source:

| Scenario | Splice node | What gets replaced |
|----------|------------|-------------------|
| Update scalar | The matched leaf node | Just the value text |
| Insert into existing parent | The parent node | Parent + all its children (original children preserved by renderer) |
| Insert with missing intermediates | Deepest existing ancestor | Ancestor + all its children (new nested structure included) |

The splice node is always the **deepest pre-existing node** whose subtree
contains the modification. This minimizes the replaced region while
ensuring the renderer produces all necessary syntax (delimiters, commas,
nesting) for the modified subtree.

## Formatting preservation

Outside the splice region, the original source is preserved byte-for-byte
(whitespace, comments, trailing commas, etc.).

Inside the splice region, the renderer controls formatting. To match the
file's existing style, the renderer can be configured with options detected
from the original source:

- **Indent string**: detected from existing indentation (spaces vs tabs, width)
- **Newline style**: detected from existing line endings (`\n` vs `\r\n`)
- **Indent level**: derived from the splice node's depth in the tree

These are passed to the renderer via `RenderOptions`.

## Extending to new languages

To support patching for a new language, only two things are needed:

1. A **parser** that produces a data tree with source spans (start/end
   line:column on each node) — this already exists for all supported
   languages via tree-sitter.
2. A **renderer** that converts a data tree back to source code — this
   must be implemented per language (currently exists for C# and JSON).

No changes to the patching algorithm are required.

## Prerequisites

The architecture depends on these properties of the pipeline:

- **Source spans**: The parser must attach start/end positions to every
  node in the data tree. These positions must be byte-accurate and
  consistent between parse runs on the same input.
- **Roundtrip fidelity**: Parsing followed by rendering must produce
  output that, when re-parsed, yields an equivalent tree. The rendered
  output need not be identical to the original source, but the tree
  structure and values must be preserved.
- **XPath stability**: The same XPath expression must match the same
  logical node in both the original tree and the re-parsed tree. Node
  names, attributes used in predicates, and tree structure must be
  consistent across parse-render-parse cycles.
