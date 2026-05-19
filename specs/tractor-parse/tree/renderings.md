---
title: Trees and Renderings — Role Distinction
priority: 0
---

This spec defines the boundary between **the tree** (canonical typed structure) and **its renderings** (XML, JSON, source text). It is the structural complement to [`design.md`](design.md): `design.md` enumerates principles for tree node naming and shape; this doc says which principles describe the tree itself and which describe specific projections of it.

Promoted from the draft `docs/design-ir-and-renderings.md` (2026-05-13). Only the parts that already reflect current code reality are committed here. Forward-leaning content (projection rules that depend on `to_data` exhaustiveness, marker-exhaustiveness claims, lossless reconstructibility) remains in the draft until validated by implementation; see *Out of scope below* at the end.

---

## 1. Layer ownership

Tractor's data model splits along one axis: **what is the data** vs. **how does it get rendered into a specific format**.

| Layer | Role | Owns |
|---|---|---|
| **Tree** (`SyntaxTree` / `DataTree` / `SqlTree`, future `DocumentTree`) | Canonical typed structure of source code. Single source of truth for "what is this construct, and what are its parts." Carries source ranges + spans for tracing. Mutation operates here. It drives the semantic transformation rules; the type system enforces invariants. | Concept identity (`SyntaxTree::Class`, `SyntaxTree::Function`, `SyntaxTree::Body`); node structure (named fields, `Vec<SyntaxTree>` lists); markers (typed enums for mutually-exclusive variants); source ranges. The tree describes nodes and their hierarchy; it determines what queries can express. It is not authoritative for query syntax — whether a boolean encodes as an XML attribute or as an empty marker child is XML's call. |
| **XML rendering** (`to_xot`) | Mechanical projection of the tree to a queryable XML tree. Optimised for XPath ergonomics. | Element name = node-type name; text recovery via gap-fill; attribute encoding (`@line`, `@column`, `@list`, `@key`); the `<expression>` host insertion (when present). |
| **JSON rendering** (`to_data` → `data_to_json`) | Mechanical projection of the tree to JSON. Optimised for JSON-tool ergonomics (jq, downstream readers). | JSON key naming = field name; scalar-vs-object decisions; array shape; `$type` metadata identifying the node type. |
| **Source rendering** (`tree::render`) | Mechanical projection of the tree back to source text (anchored or canonical). | Whitespace / formatting / language-specific syntax. |

**Mechanical means:** renderers don't make policy decisions about shape. If `<body>` is in the queryable shape, it's because there's a `SyntaxTree::Body` node type. If a `<block/>` marker appears, it's because the tree carries a typed enum whose variant maps to that name. `to_xot` does not insert wrappers without a tree antecedent, nor drop wrappers based on their name.

### 1.1 What the tree is authoritative for, and what it isn't

| The tree IS authoritative for | The tree is NOT authoritative for |
|---|---|
| Tree node types and hierarchy — what nodes exist, what their parts are | XML element naming conventions (lowercase, hyphenation rules) |
| Field structure — named typed fields, lists vs singletons | Whether to encode booleans as XML attributes vs marker elements |
| Marker enums and their values | Whether JSON renders a marker as `{public: true}` or `{visibility: "public"}` |
| What is queryable — every queryable thing has a tree-node field | Query syntax of any specific engine (XPath / JQ / future) |
| Source ranges and spans for tracing | Per-renderer attributes for source location (`@line`, `@column` etc. are XML's call) |
| Mutation surface — the tree is the only mutable representation | How a renderer chooses to splice or regenerate output after mutation |
| Driving cross-format transformations — tree → tree rewrites apply to all renderings simultaneously | Per-renderer canonical-form decisions (indentation, line breaks) |

---

## 2. The four per-domain tree types

Each domain has its own typed tree:

| Tree | Sources | Notes |
|---|---|---|
| `SyntaxTree` | Python, C#, Java, TS/JS, Rust, Go, Ruby, PHP | The programming-language tree |
| `DataTree` | JSON, YAML, TOML, INI, `.env` | The data-language tree |
| `SqlTree` | T-SQL (more SQL dialects later) | Distinct tree because SQL constructs (clauses, predicates, joins) don't map naturally onto `SyntaxTree` variants |
| `DocumentTree` *(planned)* | Markdown (now in `DataTree`); HTML and other documents later | Extraction from `DataTree` is in flight |

Each tree is a Rust enum; each variant is a **node** (`SyntaxTree::Class`, `DocumentTree::Heading`, etc.).

The three IR families staying separate as types is intentional: a single unified enum would force every cross-language unification to also accommodate JSON / YAML / SQL shapes. The cost of keeping them separate only becomes problematic if dispatch, projection, and rendering are copy-pasted three ways; the registry (`LanguageOps.tree_kind`) and the shared `data_to_json` projection address those.

---

## 3. What tree nodes encode

A node's typed shape carries everything a renderer needs. Specifically:

1. **Concept identity** — the node-type name itself (`Class`, `Function`, `Binary`).
2. **Sub-variant kind** — for nodes whose surface form has mutually-exclusive variations, an `Option<EnumKind>` field whose variants name the markers (e.g. `BodyForm::{Block, Pass}`).
3. **Independent surface flags** — for nodes with multiple non-exclusive markers, a `Vec<EnumKind>` field where each enum variant maps to one marker.
4. **Named typed fields** — single-value relationships (`condition: Box<SyntaxTree>`, `value: Option<Box<SyntaxTree>>`).
5. **List fields** — multi-value relationships (`children: Vec<SyntaxTree>`, `parameters: Vec<SyntaxTree>`). The field name is the JSON key and the XML element-name plurality.
6. **Source ranges** — `range: ByteRange`, `span: Span`. Always present.

Anything that doesn't fit one of these six categories is suspicious — it likely encodes either a Rust-API ergonomic (drop or convert) or a legacy XML quirk (move to renderer-only).

---

## 4. Re-bucketing the existing principles

Today's [`design.md`](design.md) catalogue mixes principles that describe the tree itself with principles that describe how the tree projects to XML or JSON. Each principle falls into one of three buckets.

### Bucket A — Tree structure principles

Properties of the tree itself, independent of how any renderer projects it.

| Today's principle | What it says about the tree |
|---|---|
| #5 Unified Concepts | Same concept = same tree node type (e.g. `SyntaxTree::Class` everywhere) |
| #6 Preserve Language Idioms | What node types exist |
| #9 Exhaustive Markers for Mutually Exclusive Variations | **Refined:** typed enums on the tree (one enum per mutex group). Booleans don't work — XML has no name for the "false" marker. |
| #11 Specific Names Over Type Hierarchies | Node-type naming |
| #13 Annotation Follows Node Shape | Markers attach to the node they describe |
| #15 Markers Live in Stable, Predictable Locations | Markers as enum fields on the parent node |
| #17 Avoid Compound Node Names | Node-type naming (no camelcase either) |
| #18 Name Relationships After the Operator | Node-type naming |
| #19 Wrap Role-Mixed Text-Leaves | Tree field structure for ambiguous text positions |

(The tree-side counterpart of #12 — list fields are `Vec<SyntaxTree>` directly under the parent, with no grouping-wrapper node-type variant — is covered mechanically by §3 rule #5; it doesn't need a separate Bucket A entry. The XML and JSON projections of that fact live in Bucket B.)

### Bucket B — XML projection principles

Properties of how `to_xot` projects the tree into XML.

| Today's principle | What it says about XML projection |
|---|---|
| #1 Use Language Keywords | XML element name = node-type name (which IS the language keyword by Principle #1's spirit) |
| #2 Full Names Over Abbreviations | XML naming convention (inherited from node-type naming) |
| #3 Always Lowercase | XML element name convention |
| #4 Elements Over Attributes | XML encoding choice (when to use children vs attrs) |
| #7 Modifiers as Empty Elements | XML projection of the tree's marker enums |
| #8 Renderability | XML output reversibility (text recovery) |
| #10 Marker Source Locations | XML attribute encoding for marker source ranges |
| #12 Flat Lists Over Wrapper Elements | XML emits list items as flat siblings carrying `list="X"`, with no grouping wrapper element (`<parameter list="parameters">` siblings, not `<parameters><parameter/>…</parameters>`). The JSON projection is the inverse: explicit container key, implicit per-item type (`{parameters: [{…}, {…}]}`). |
| #14 Namespace Vocabulary | XML naming for compound concepts |
| #16 Optimize for Repeated Patterns | XML form for recurring patterns |

### Bucket C — Goals that span layers

These describe outcomes inherited by both the tree and every rendering of it.

| Today's goal | Inherited by |
|---|---|
| #1 Intuitive Queries | Any query interface (XPath, JQ, future) |
| #2 Readable Tree Structure | Any rendering |
| #4 Minimal Query Complexity | Any query interface |
| #5 Match Developer's Mental Model | Primarily the tree; renderings inherit |
| #7 Source Reversibility | Tree carries the ranges; renderings recover via gap-fill or anchoring |

---

## 5. Stable goals (subset)

Three of the six tree goals from the design draft already reflect current state and are committed here. The other three depend on coverage work that hasn't finished and remain in the draft.

1. **Concept faithfulness.** Every tree node matches a developer concept (`class`, `function`, `if`, `binary`). No abstract supertypes (`expression`, `declaration`) at the baseline. Surface variants narrow a stable concept; they don't replace it.
2. **Source reversibility.** `tree.range().slice(source)` plus the recursive structure recovers the original text. Markers replacing keywords keep the keyword as gap text. Mutation rewrites bytes through the node's range. Anchored-mode source rendering is implemented and byte-identical.
3. **Cardinality independence.** A node's structural shape doesn't depend on how many children it has. One method or twelve, the parent's field layout is identical.

---

## Out of scope (still in draft)

The following are still validated against in-flight implementation and live in [`docs/design-ir-and-renderings.md`](../../../docs/design-ir-and-renderings.md) until they firm up:

- Cross-language uniformity *as a tree goal*. Cross-language consistency tests still surface gaps; the claim "cross-language queries Just Work" is the target, not the current state.
- Marker exhaustiveness via typed enums *as a global guarantee*. Depends on `Visibility` / `BodyForm` / etc. enums being declared in every language's lowering.
- Lossless reconstructibility through renderings. Depends on `$type` actually being emitted by `data_to_json` for every node type — bound to S5A coverage completion.
- The §3 projection contracts in the draft (XML rules, JSON rules, source rules). Today's `to_data::project` is a per-variant pattern match; the contract describes the mechanical end state.
- Per-variant projection examples (Class / Function / Body / Binary walkthroughs).

When each of these stabilises in code, it graduates from the draft into this spec.
