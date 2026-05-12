# Trees and Renderings — Role Distinction

**Status:** Draft for iteration. **Partially promoted to [`specs/tractor-parse/tree/renderings.md`](../specs/tractor-parse/tree/renderings.md) on 2026-05-13** — the stable structural pieces (layer ownership, what-the-tree-is-authoritative-for, the six node-encoding categories, the Bucket A/B/C re-bucketing, the four per-domain tree types, three stable goals). Forward-leaning content — the §3 projection contracts, §2's marker-exhaustiveness / cross-language-uniformity / lossless-reconstructibility goals, and the example walkthroughs — stays here until S5 / S11 implementation validates them. Each piece graduates from this draft into the spec when its corresponding code stabilises.

**Purpose.** Today's design docs in `specs/tractor-parse/tree/design.md`, `specs/tractor-parse/dual-view/data-branch/*.md`, and `specs/codexpath/cli/output-options/json-format/*.md` write principles in terms of XML element shapes (`<class>`, `<body>`, `list="X"`, etc.). After the tree migration (S3 / S4 / S5), `SyntaxTree` / `DataTree` / `SqlTree` (and `DocumentTree` once Markdown lifts out) are the canonical typed structures — XML and JSON are derived views. This doc proposes how to disentangle the two: which principles describe the tree (the data) and which describe each rendering of it.

**How to use this doc.** Mark up inline with your comments. Use any markup that's clear (`<!-- COMMENT: ... -->`, `>>` quotation, or just edit/add prose). When the design feels stable, we lift relevant pieces into `specs/` and retire / rewrite the affected source-of-truth docs.

---

## 1. The core distinction

| Layer                                              | Role                                                                                                                                                                                                                                                                                                                                                  | Owns                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| -------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Tree** (`SyntaxTree` / `DataTree` / `SqlTree`)   | Canonical typed structure of source code. Single source of truth for "what is this construct, and what are its parts." Carries source ranges + spans for tracing. Mutation operates here. It ensures that transformations happen consistently at all levels, it drives the semantic transformation rules, and the type system enforces invariants.   | Concept identity (`SyntaxTree::Class`, `SyntaxTree::Function`, `SyntaxTree::Body`); node structure (named fields, `Vec<SyntaxTree>` lists); markers (typed enums for mutually-exclusive variants); source ranges. In other words it describes the tree nodes and their hierarchy, it determines what is expressible in queries, and is authoritative as to what the structure of queries will look like, but the specific syntax needed for each query language is not determined by the tree. For example, whether to encode a boolean value as an XML attribute or as an empty marker element is XML's decision. |
| **XML rendering** (`to_xot`)                       | Mechanical projection of tree to a queryable XML tree. Optimised for XPath ergonomics.                                                                                                                                                                                                                                                                | Element name = node type name; text recovery via gap-fill; attribute encoding (`@line`, `@column`, `@list`, `@key`); the `<expression>` host insertion (if kept).                                                                                                                                                                                                                                                                                                                                                                                                                      |
| **JSON rendering** (`to_data` → `data_to_json`)    | Mechanical projection of tree to JSON. Optimised for JSON-tool ergonomics (jq, downstream readers).                                                                                                                                                                                                                                                   | JSON key naming = field name; scalar-vs-object decisions; array shape; `$type` metadata identifying the node type.                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| **Source rendering** (`tree::render`)              | Mechanical projection of tree back to source text (anchored or canonical).                                                                                                                                                                                                                                                                            | Whitespace / formatting / language-specific syntax.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |

**Mechanical means:** renderers don't make policy decisions about shape. If `<body>` is in the queryable shape, it's because there's a `SyntaxTree::Body` node type. If a `<block/>` marker appears, it's because the tree carries a typed enum whose variant maps to that name. `to_xot` does not insert wrappers without a tree antecedent, nor drop wrappers based on their name.

### 1.5 What the tree is authoritative for, and what it isn't

To make the boundary unambiguous:

| The tree is authoritative for | The tree is NOT authoritative for |
|---|---|
| Tree node types and hierarchy (what nodes exist, what their parts are) | XML element naming convention (lowercase, hyphenation rules) |
| Field structure (named typed fields, lists vs singletons) | Whether to encode booleans as XML attributes vs marker elements |
| Marker enums and their values | Whether JSON renders a marker as `{public: true}` or `{visibility: "public"}` |
| What is queryable (every queryable thing has a tree-node field) | Query syntax of any specific engine (XPath / JQ / future) |
| Source ranges and spans for tracing | Per-renderer attributes for source location (`@line`, `@column` etc. are XML's call) |
| Mutation surface — the tree is the only mutable representation | How a renderer chooses to splice / regenerate output after mutation |
| Driving cross-format transformations (tree → tree rewrites apply to all renderings simultaneously) | Per-renderer canonical-form decisions (e.g. how to indent, how to break lines) |

---

## 2. The tree's goals (the new top-level)

Reframing today's `semantic-tree/design.md` "Design Goals" to be tree-shape-centric — renderings inherit these.

1. **Concept faithfulness.** Every tree node matches a developer concept (`class`, `function`, `if`, `binary`). No abstract supertypes (`expression`, `declaration`) at the baseline. Surface variants narrow a stable concept; they don't replace it. *(Today's Goal #5; Principle #11.)*
2. **Cross-language uniformity.** The same concept produces the same tree shape across languages, modulo intentional syntax markers (`BodyForm::Block` for brace-bodied, no marker for colon/indent). Cross-language queries Just Work. *(Today's Goal #4; Principle #5.)*
3. **Source reversibility.** `tree.range().slice(source)` plus the recursive structure recovers the original text. Markers replacing keywords keep the keyword as gap text. Mutation rewrites bytes through the node's range. *(Today's Goal #7.)*
4. **Cardinality independence.** A node's structural shape doesn't depend on how many children it has. One method or twelve, the parent's field layout is identical. *(Today's `cardinality-independence.md`.)*
5. **Marker exhaustiveness via typed enums.** Where mutually-exclusive surface forms exist, the node carries an `Option<EnumKind>` (or `EnumKind` with a default). The marker name is *derived* from the enum variant — never a free-form string. Adding a form is one enum variant; the renderer follows mechanically. *(Today's Principle #9, refined.)*
6. **Lossless reconstructibility through renderings.** Every rendering must allow reconstruction of the original tree (i.e. trace back any matched subtree to source). Internal projection plumbing (`$inline`, `$skip`, etc.) does NOT leak. `$type` IS legitimate metadata identifying the tree-node type. **Decided 2026-05-11:** always emit `$type` on every JSON object node. The "omit when parent property name matches singular type" optimisation is on the table but deferred — start with always-emit for simplicity and unambiguous round-trippability. Goal: the JSON shape is theoretically describable by a TypeScript interface — every field has a known type, every child is identifiable.

<!-- COMMENT: ... -->

---

## 3. Projection contracts (mechanical, not policy)

Each renderer walks tree nodes by uniform rules. No special-cased behaviour keyed by node-type name; every rule below is generic over the tree's enum structure.

### 3.1 XML / `to_xot`

- `TreeNode::Foo { children, field_a, field_b, ... }` → `<foo>{field_a}{field_b}{children}</foo>`. Element name = node-type name (snake_case via strum), no exceptions.
- **Semantic enum fields** (named for the concept they encode, not "marker"). E.g. `visibility: Visibility` with variants `Public` / `Private` / `Protected`, or `form: Option<BodyForm>` with variants `Block` / `Pass`. The XML renderer emits each present enum *value* as an empty-element child: `Visibility::Public` → `<public/>`, `BodyForm::Block` → `<block/>`. The XML doesn't see the field name (`visibility`); only the enum-value name. `Vec<EnumKind>` fields emit one marker per element, preserving source order. Enums in `Option<T>` simply emit nothing when None.
- Structural fields (`Binary { left, op, right }`) emit as named wrapper elements (`<left>`, `<op>`, `<right>`). The field name IS the wrapper element name; no XML-only insertions.
- Gap text between source-derived siblings emits to preserve byte-by-byte recovery (existing `render_with_gaps` mechanism).
- Source location attributes (`line` / `column` / `end_line` / `end_column` / `range`) derive from the node's `Span` and `ByteRange`.

**~~Open question A~~ → Resolved.** Keep `SyntaxTree::Expression` as a real tree node (option A1).

> **Rationale (your call):** the tree represents the queryable structure; conceptually queries should be the same between XPath and JQ. If we need an expression container for value-positions to host markers (`non_null`, `await`), it belongs in the tree — not a typed wrapper or a render-time insertion. A2 (typed field wrapper) and A3 (drop entirely) rejected: A3 because markers must modify conceptual objects, not primitive values; A2 because there's no real benefit over just having the node.

<!-- COMMENT on Open question A: ... -->

### 3.2 JSON / `to_data` → `data_to_json`

- `TreeNode::Foo { children, field_a, field_b, ... }` → `{ "$type": "foo", field_a: …, field_b: …, children: [...] }`. Key name = field name (with optional rename if Rust-keyword conflict like `then_` → `then`).
- **`$type` rule (decided 2026-05-11):** always emit `"$type"` for now. The "omit when parent property name matches singular type" optimisation is deferred — see Q6, now closed-as-deferred. Always-emit is simpler, unambiguous, and round-trippable.
- **Semantic enum fields — B2 (string-style, decided 2026-05-11):** `visibility: Visibility::Public` → `{ "visibility": "public" }`. The field name appears as the JSON key; the enum value's snake_case name is the string value. Optional fields with `None` → omitted entirely. `Vec<EnumKind>` → array of strings.
  - Trade-off accepted: query asymmetry vs XPath (`//class[public]` ↔ JSON `class.visibility === "public"`). The XPath syntax stays — JSON path is its own dialect.
- Anonymous-vec-only nodes (`Tuple`, `List`, `Set`) → JSON array. Otherwise → JSON object.
- Scalar-leaf nodes → bare JSON scalar.
- `$inline` / `$skip` / projection plumbing names DO NOT appear in JSON. Those are internal to `to_data`'s rules, not data.

### 3.3 Source / `tree::render`

- **Anchored mode** (default): slices `tree.range()` from input verbatim — byte-identical roundtrip.
- **Canonical mode**: per-language formatter logic that consumes tree nodes directly (no XML/JSON in the loop). Used when re-emitting after structural mutation, when source isn't available, or when canonical formatting is desired.

### 3.4 Examples — tree ↔ XML ↔ JSON

Concrete cases showing the conversion contracts side-by-side. (Source-text rendering omitted; that's per-language formatting and lives in `tree::render`.) Examples use the actual Rust type `SyntaxTree::*` since these are programming-language constructs; the abstract rules in §3.1–3.2 use the generic `TreeNode::*` placeholder.

#### Example 1 — public class with one field

**Tree**
```rust
SyntaxTree::Class {
    visibility: Visibility::Public,
    name: Box::new(SyntaxTree::Name { range: 6..9, ... }),  // source: "Foo"
    body: Box::new(SyntaxTree::Body {
        form: Some(BodyForm::Block),
        children: vec![
            SyntaxTree::Variable {
                visibility: Visibility::Private,
                type_ann: Some(Box::new(SyntaxTree::Name { range: 18..21, ... })),  // "int"
                name: Box::new(SyntaxTree::Name { range: 22..23, ... }),            // "x"
                value: None,
            },
        ],
    }),
}
```

**XML** (`<public/>` / `<private/>` / `<block/>` are enum-value markers; `<name>` is a field wrapper; `<body>` and `<variable>` are node elements):
```xml
<class>
  <public/>
  <name>Foo</name>
  <body>
    <block/>
    <variable>
      <private/>
      <type><name>int</name></type>
      <name>x</name>
    </variable>
  </body>
</class>
```

**JSON** (B2 string encoding for enum fields; `$type` always emitted, per 2026-05-11 decisions):
```json
{
  "$type": "class",
  "visibility": "public",
  "name": "Foo",
  "body": {
    "$type": "body",
    "form": "block",
    "children": [
      {
        "$type": "variable",
        "visibility": "private",
        "type": "int",
        "name": "x"
      }
    ]
  }
}
```

Notes: `name` and `type` fields dereference scalar `Name` to a bare string (TS-interface: `name: string`). Enum fields use the field name as the JSON key with the enum value as a snake_case string. `$type` always emitted on every object node — the parent-field omit-rule (Q6) is deferred.

#### Example 2 — function with two parameters

**Tree**
```rust
SyntaxTree::Function {
    visibility: Visibility::Public,
    name: ...,                 // "add"
    parameters: vec![
        SyntaxTree::Parameter { name: ...("a"), type_ann: ...("int"), default: None },
        SyntaxTree::Parameter { name: ...("b"), type_ann: ...("int"), default: None },
    ],
    returns: Some(...),         // "int"
    body: ...,
}
```

**XML** (per Bucket-B Principle #12: list field is flat — no `<parameters>` wrapper; each `<parameter>` carries `list="parameters"` for cardinality independence):
```xml
<function>
  <public/>
  <name>add</name>
  <parameter list="parameters"><name>a</name><type><name>int</name></type></parameter>
  <parameter list="parameters"><name>b</name><type><name>int</name></type></parameter>
  <returns><type><name>int</name></type></returns>
  <body>...</body>
</function>
```

**JSON** (explicit container `parameters: [...]`; `$type` always emitted; enum fields B2-string):
```json
{
  "$type": "function",
  "visibility": "public",
  "name": "add",
  "parameters": [
    {"$type": "parameter", "name": "a", "type": "int"},
    {"$type": "parameter", "name": "b", "type": "int"}
  ],
  "returns": {"$type": "returns", "type": "int"},
  "body": {...}
}
```

#### Example 3 — body with heterogeneous statements

**Tree**
```rust
SyntaxTree::Body {
    form: None,                 // colon/indent (Python) or unmarked
    children: vec![
        SyntaxTree::Variable {...},
        SyntaxTree::If {...},
        SyntaxTree::Return {...},
    ],
}
```

**XML** (statements as siblings under `<body>`; same shape as Example 1 minus `<block/>`):
```xml
<body>
  <variable>...</variable>
  <if>...</if>
  <return>...</return>
</body>
```

**JSON** (`$type` always emitted; heterogeneous-children is the canonical case where it's load-bearing):
```json
{
  "$type": "body",
  "children": [
    {"$type": "variable", ...},
    {"$type": "if", ...},
    {"$type": "return", ...}
  ]
}
```

#### Example 4 — binary expression `a + b`

**Tree**
```rust
SyntaxTree::Binary {
    left: Box::new(SyntaxTree::Expression {
        inner: Box::new(SyntaxTree::Name { range: 0..1, ... }),  // "a"
    }),
    op: BinaryOp::Plus,
    op_text: "+".to_string(),
    op_range: 2..3,
    right: Box::new(SyntaxTree::Expression {
        inner: Box::new(SyntaxTree::Name { range: 4..5, ... }),  // "b"
    }),
}
```

**XML** (`<expression>` host present; `<op>` carries text + a marker for the op kind):
```xml
<binary>
  <left><expression><name>a</name></expression></left>
  <op>+<plus/></op>
  <right><expression><name>b</name></expression></right>
</binary>
```

**JSON** (`$type` always emitted; the `op` field is a node containing `text` + an enum `kind: "plus"`):
```json
{
  "$type": "binary",
  "left": {"$type": "expression", "inner": {"$type": "name", "text": "a"}},
  "op": {"$type": "op", "text": "+", "kind": "plus"},
  "right": {"$type": "expression", "inner": {"$type": "name", "text": "b"}}
}
```

#### Example 5 — scalar leaves

| Tree | XML | JSON |
|---|---|---|
| `SyntaxTree::Name { range: 0..3 }` (text "Foo") | `<name>Foo</name>` (when standalone); when inside a `<name>` field wrapper, the wrapper IS the name element so the inner becomes raw text | `"Foo"` (bare scalar; parent field's TS type is `string`) |
| `SyntaxTree::Int { range: 0..2 }` (text "42") | `<int>42</int>` | `42` (bare number) |
| `SyntaxTree::String { range: 0..5 }` (text `"hi"` with quotes) | `<string>hi</string>` (escapes resolved) | `"hi"` |
| `SyntaxTree::Null` | `<null/>` | `null` |

#### Example 6 — list of attributes (decorators)

**Tree** (suppose `decorators: Vec<SyntaxTree>` directly contains expression nodes after S11-Z3):
```rust
SyntaxTree::Function {
    decorators: vec![
        SyntaxTree::Call { callee: ...("app.route"), arguments: vec![...] },
    ],
    ...
}
```

**XML** (#12 flat: each decorator emits as a sibling with `list="decorators"`):
```xml
<function>
  <call list="decorators"><name>app.route</name>...</call>
  <name>handler</name>
  ...
</function>
```

**JSON** (explicit container; `$type` always emitted on every item):
```json
{
  "$type": "function",
  "decorators": [
    {"$type": "call", "callee": "app.route", "arguments": [...]}
  ],
  "name": "handler",
  ...
}
```

(After S11-Z3 drops the `Decorator` wrapper, items in `decorators` are the inner expression types directly — `call`, `name`, etc. — heterogeneous, so `$type` is load-bearing here.)

<!-- COMMENT on examples: ... -->

---

## 4. What tree nodes must encode

Based on the rules above, a node's typed shape carries everything a renderer needs. Specifically:

1. **Concept identity** — the node-type name itself (`Class`, `Function`, `Binary`).
2. **Sub-variant kind** — for nodes whose surface form has mutually-exclusive variations, an `Option<EnumKind>` field whose variants name the markers (e.g. `BodyForm::{Block, Pass}`).
3. **Independent surface flags** — for nodes with multiple non-exclusive markers, a `Vec<EnumKind>` field where each enum variant maps to one marker.
4. **Named typed fields** — single-value relationships (`condition: Box<SyntaxTree>`, `value: Option<Box<SyntaxTree>>`).
5. **List fields** — multi-value relationships (`children: Vec<SyntaxTree>`, `parameters: Vec<SyntaxTree>`). The field name is the JSON key and the XML element-name plurality.
6. **Source ranges** — `range: ByteRange`, `span: Span`. Always present.

Anything that doesn't fit one of these six categories is suspicious — it likely encodes either a Rust-API ergonomic (drop or convert) or a legacy XML quirk (move to renderer-only).

<!-- COMMENT: ... -->

---

## 5. Re-bucketing the existing principles

Today's `semantic-tree/design.md` has 7 goals + 19 principles + 16 decisions. Each falls into one of three buckets.

### Bucket A — Tree structure principles

| Today's principle                                       | What it says about the tree                                                                                                                                                                                        |
| ------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| #5 Unified Concepts                                     | Same concept = same tree node type (e.g. `SyntaxTree::Class` everywhere)                                                                                                                                           |
| #6 Preserve Language Idioms                             | What node types exist                                                                                                                                                                                              |
| #9 Exhaustive Markers for Mutually Exclusive Variations | **Refined:** typed enums on the tree (one enum per mutex group). Booleans don't work — XML has no name for the "false" marker.                                                                                     |
| #11 Specific Names Over Type Hierarchies                | Node-type naming                                                                                                                                                                                                   |
| #13 Annotation Follows Node Shape                       | Markers attach to the node they describe                                                                                                                                                                           |
| #15 Markers Live in Stable, Predictable Locations       | Markers as enum fields on the parent node                                                                                                                                                                          |
| #17 Avoid Compound Node Names                           | Node-type naming (no camelcase either)                                                                                                                                                                             |
| #18 Name Relationships After the Operator               | Node-type naming                                                                                                                                                                                                   |
| #19 Wrap Role-Mixed Text-Leaves                         | Tree field structure for ambiguous text positions                                                                                                                                                                  |

(The tree-side counterpart of #12 — list fields are `Vec<SyntaxTree>` directly under the parent, no grouping-wrapper node-type variant — is covered mechanically by §4 rule #5; it doesn't need its own Bucket A entry. The XML and JSON projections of that fact live in Bucket B and Bucket C respectively.)

### Bucket B — XML projection principles (move under "XML Rendering")

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

### Bucket C — Goals that span layers (stay at top)

| Today's goal | Inherited by |
|---|---|
| #1 Intuitive Queries | Any query interface (XPath, JQ, future) |
| #2 Readable Tree Structure | Any rendering |
| #4 Minimal Query Complexity | Any query interface |
| #5 Match Developer's Mental Model | Primarily the tree; renderings inherit |
| #7 Source Reversibility | Tree carries the ranges; renderings recover via gap-fill or anchoring |

<!-- COMMENT on the bucketing: ... -->

---

## 6. Migration plan (small steps, each its own commit)

1. **Add `specs/tractor-parse/tree-and-renderings.md`** with the role distinction (this doc, polished). Reference it from `tree/design.md`'s preamble.
2. **Annotate `tree/design.md` in place.** Add an `[A]` / `[B]` / `[C]` tag next to each existing goal/principle. No content move yet.
3. **Annotate `dual-view/data-branch/*.md` and `output-options/json-format/*.md`.** Mark which paragraphs are tree-shape-talking and which are XML/JSON projection-talking. Many paragraphs say "X becomes `<x>`" but really mean "X is `SyntaxTree::X` and renders as `<x>`."
4. **Promote the projection contracts** (§3 above) to a stable section in `tree-and-renderings.md`. Reference from `to_xot.rs`, `to_data.rs`, `data_to_json.rs`, `tree/render/*.rs` doc comments.
5. **Per-node-type doc-comment uplift.** Each `SyntaxTree::Variant` doc currently says "renders as `<variant>...`" — split into "tree shape: ..." and "XML rendering: ..." so the tree docstring is true regardless of which renderer is being read.
6. **Retire XML-specific examples in tree-shape principles.** For each Bucket-A principle whose example is XML (`<method><parameter list="parameters">…`), add a parallel JSON example so the principle reads as "data shape" not "XML shape."

<!-- COMMENT on migration plan: ... -->

---

## 7. Connections to other design docs

After the 2026-05-12 doc-consolidation commit (`fbed305a`) and the in-flight terminology sweep (S12), the relevant doc landscape:

**Pipeline & architecture (`docs/`):**
- **`docs/pipeline-architecture.md`** — canonical pipeline reference (rewritten 2026-05-12; tree-terminology scrub in progress). Describes the data-processing pipeline (CLI → parse → query → render).
- **`docs/transform-validation-architecture.md`** — regression archetypes + NodeRole content (rewritten 2026-05-12).
- **`docs/design-projection-pipeline.md`** — articulates the three projection architectures (A: per-format renderers, B: universal DataTree, C: hybrid) and recommends C. This proposal aligns with C and refines the tree side of it. Heavy old-vocabulary load; largest doc in the S12 sweep.
- **`docs/design-transform-redesign-exploration.md`** — ADOPTED-banner historical doc explaining why we moved from imperative xot mutation to typed trees. Body intentionally frozen for accuracy; banner notes the rename mapping.

**Specs (`specs/tractor-parse/`):**
- **`specs/tractor-parse/tree/design.md`** — the principle catalogue this proposal re-buckets (§5).
- **`specs/tractor-parse/tree/transformations.md`** — describes per-language CST→semantic transforms; in the new framing, these are CST→tree lowering rules (the tree is the post-transform state).
- **`specs/tractor-parse/tree/chain-inversion.md`** — moved into the spec dir from `docs/` on 2026-05-12. Spec for the `SyntaxTree::Access` left-deep chain shape. Cross-link from `design.md`.
- **`specs/tractor-parse/dual-view/data-branch/*.md`** — describes the data-tree XML shape; in the new framing, those are `DataTree → XML` projection rules.
- **`specs/codexpath/cli/output-options/json-format/*.md`** — describes JSON shape; in the new framing, those are `tree → JSON` projection rules.

**Deleted on 2026-05-12 (so this doc doesn't reference them, but readers tracking the design history should know):**
- `docs/design-ir-to-dataIr-projection.md` — superseded by `design-projection-pipeline.md`.
- `docs/data-multi-view-impl-plan.md` — impl plan for a shipped feature.

<!-- COMMENT on connections: ... -->

---

## 9. Terminology — per-domain tree types (shipped in code; doc scrub in flight)

**Decision.** The "IR" framing leaked an implementation term ("intermediate representation") into the API. Replaced with **per-domain tree types**, each containing **nodes**.

### 9.1 The four tree types

| Tree | Sources | Replaces |
|---|---|---|
| **`SyntaxTree`** | Python, C#, Java, TS/JS, Rust, Go, Ruby, PHP | `Ir` |
| **`DataTree`** | JSON, YAML, TOML, INI, `.env` | `DataIr` (Markdown moves out) |
| **`SqlTree`** | T-SQL (more SQL dialects later) | `SqlIr` |
| **`DocumentTree`** | Markdown (now), HTML (later); possibly RST/AsciiDoc | new — extracted from `DataIr`'s Markdown lowering |

Each tree is a Rust enum; each variant is a "node". `SyntaxTree::Class { ... }`, `DocumentTree::Heading { ... }`, etc.

### 9.2 Sweep status

The "IR" term has been removed from **code** and is being removed from **comments and docs**. Scope:

- **Code:** ✅ shipped. Type renames `Ir → SyntaxTree`, `DataIr → DataTree`, `SqlIr → SqlTree`. New `DocumentTree` planned for Markdown. Module: `crate::ir → crate::tree`. Submodules: `tree::syntax`, `tree::data`, `tree::sql`, `tree::document` (or flat — TBD). Supporting renames: `IrFamily → TreeKind`, `lower_ir_* → lower_*`, `to_xot::render_ir_* → render_tree_*`, `data_ir → data_tree`, file names `ir_*.rs → tree_*.rs`.
- **Vocabulary in docs:** "IR" → "tree" or specific tree type; "IR variant" → "tree node"; "IR shape" → "tree structure"; "IR pipeline" → "tree pipeline"; "intermediate representation" disappears. "slot" (when meaning a named property on a node) → "field" or "property".
- **Doc-scrub status:**
  - `docs/design-ir-and-renderings.md` — ✅ this doc (scrubbed 2026-05-12 alongside the slot→field tightening).
  - `docs/pipeline-architecture.md` — partially scrubbed; some IR mentions still in code-comment references.
  - `docs/transform-validation-architecture.md` — light touch needed; a handful of IR mentions.
  - `docs/design-projection-pipeline.md` — heaviest single doc; ~105 IR mentions. Largely becomes a tree-projection design doc after sweep.
  - `docs/design-transform-redesign-exploration.md` — ADOPTED-banner historical doc. Body frozen for historical accuracy; banner notes the rename mapping.
  - `specs/tractor-parse/tree/*` — directory renamed; subspecs being scrubbed; later passes may split into `syntax-tree.md` / `data-tree.md` / `sql-tree.md` / `document-tree.md` subdivisions.
  - `specs/tractor-parse/dual-view/*`.
  - `specs/codexpath/cli/output-options/json-format/*`.
  - `specs/cli-output-design.md`.
- TODO.md scrubbed.

Tracked as **S12** in TODO.md.

---

## 10. Open questions

| Q | Status |
|---|---|
| Q1. Layer distinction in §1 / §1.5 | open — assumed acceptable until you flag otherwise |
| Q2. `<expression>` host | ✅ resolved: A1, keep `SyntaxTree::Expression` |
| Q3. Marker enums strictness | open — minor; `Vec<EnumKind>` acceptable for non-exclusive flag groups |
| Q4. Final doc location | open: (a) new file, (b) rewrite design.md, (c) fork into multiple |
| Q5. Bucket completeness | open — #12 to move to Bucket B; otherwise no gaps flagged |
| Q6. `$type` omission rule precision | ⏸ deferred (2026-05-11) — always emit for now |
| Q7. JSON enum encoding | ✅ resolved 2026-05-11: B2 string-style (`"visibility": "public"`) |
| Q8. Terminology rename | ✅ resolved 2026-05-11; code rename shipped (see S12) |
| Q9. `IrFamily` rename | ✅ resolved 2026-05-12: `TreeKind` with variants `Syntax` / `Data` / `Sql` / `Document` |
| Q10. `specs/tractor-parse/tree/` directory | ✅ resolved 2026-05-12: rename to `specs/tractor-parse/tree/`. Subspecs can be reorganised underneath (`syntax-tree.md`, `data-tree.md`, etc.) as the sweep proceeds. |
| Q11. Scrub `docs/design-transform-redesign-exploration.md`? | ✅ resolved 2026-05-12: leave body frozen for historical accuracy; add a terminology-note banner at top mapping `Ir → SyntaxTree`, `DataIr → DataTree`, `SqlIr → SqlTree`. |
| Q12. Markdown tree name (provisional answer pending your call) | ✅ resolved 2026-05-12: `DocumentTree` (shared with future HTML) |
| Q13. "slot" vocabulary | ✅ resolved 2026-05-12: drop "slot" in favour of "field" / "property" — conceptually it's just a named property on a node. |

<!-- COMMENT here: ... -->

---

## 11. History

Log of changes driven by inline `%%` comments and chat feedback. Each entry names the source of the input, the change applied, and (where relevant) the section affected. The intent is that no user-authored comment is silently dropped; if you read this list you can trace why each piece of the doc looks the way it does.

### 2026-05-13

- **Partial promotion to specs**: created [`specs/tractor-parse/tree/renderings.md`](../specs/tractor-parse/tree/renderings.md) with the stable structural pieces — §1 layer ownership table, §1.5 authoritative-for table, §4 six node-encoding categories, §5 Bucket A/B/C re-bucketing of existing principles, §9 four per-domain tree types, and three of §2's six goals (concept faithfulness, source reversibility, cardinality independence). Source: chat decision (2026-05-13) that the structural pieces are stable enough to commit even while the projection contracts and forward-leaning goals remain draft. The draft retains everything else; each piece graduates from here to the spec when its code stabilises. Companion cross-link added to `tree/design.md` preamble.

### 2026-05-12

- **§1 table — XML/JSON encoding example folded into prose** (was `%% for example whether to encode a boolean value as an attribute or an empty marker element is XML's decision %%`). Source: user inline `%%` comment in the Tree row's *Owns* column. The example now reads as a regular sentence in the row text.
- **§1 closing — Claude-authored scaffolding removed** (was `%% claude: incorporating your feedback into the table prose. Cleaner phrasing of the Tree row to roll into §1.5 below: … %%`). Source: Claude's draft staging note. The consolidated phrasing it described lives in §1.5; the scaffolding is no longer needed.
- **§5 Bucket A header — rename note removed** (was `%% claude: renamed per your suggestion. See §9 below for the Tree / TreeNode terminology — now shipped in code. %%`). Source: Claude's draft note. The rename is now shipped (§9.2); the heading needs no annotation.
- **§5 Bucket A row #5 (Unified Concepts) — example folded** (was `%% i.e. `SyntaxTree::Class` everywhere %%`). Source: user inline `%%` clarification. Now reads "Same concept = same tree node type (e.g. `SyntaxTree::Class` everywhere)".
- **§5 Bucket A row #9 (Exhaustive Markers) — rationale folded** (was `%% i.e. avoid booleans because then xml doesn't know what to call the 'false' marker`). Source: user inline `%%` rationale. Now reads "Booleans don't work — XML has no name for the 'false' marker."
- **§5 Bucket A row #17 (Avoid Compound Node Names) — clarification folded** (was `%% this means also no camelcase %%`). Source: user inline `%%` clarification. Now reads "Node-type naming (no camelcase either)".
- **§5 — Principle #12 (Flat Lists Over Wrapper Elements) moved from Bucket A to Bucket B**. Source: user inline `%%` note: *"12 is an XML concern. list wrappers that contain unique lists are not encoded as a wrapping element. so class/attribute instead of class/attributes/attribute. in json the container name is explicit, and the element types are implicit: class.attributes = [{...attribute data},]"*. The XML projection of "no wrapper element" + the JSON projection of "explicit container key" are now both stated in the #12 Bucket B row. A parenthetical under Bucket A notes that the tree-side counterpart (list fields are `Vec<SyntaxTree>` directly, no grouping-wrapper node-type) is covered mechanically by §4 rule #5 and doesn't need its own principle entry.
- **Both `%%` comments below the Bucket A table — removed** (the user note above, and the Claude agreement note `%% claude: agreed — moving #12 to Bucket B in the next iteration … %%`). Source: pair of `%%` comments. Both fully addressed by the #12 move above; their text is no longer needed standalone.
- **Whole-doc — "slot" terminology dropped in favour of "field" / "property"**. Source: user chat feedback ("i also dont like the term slot. to me conceptually its just a field or property on a node"). Applied throughout: `slot_a / slot_b` placeholders → `field_a / field_b`; "named slot" / "typed slot" → "named field" / "typed field" / "field"; "slot wrapper" → "field wrapper element" (where it survives at all); "IR slot" → "tree-node field". Q13 added to §10.
- **Whole-doc — "IR" terminology replaced with "tree" / per-domain tree types**. Source: user chat confirmation that the code rename has shipped (`Ir → SyntaxTree`, `DataIr → DataTree`, `SqlIr → SqlTree`; `crate::ir → crate::tree`; module path `ir::source → tree::render`). Section §9.2 status updated from "planned/in-flight" to "shipped" for the code portion; doc-scrub list reflects per-doc status. Title updated from "IR and Renderings — Role Distinction" to "Trees and Renderings — Role Distinction"; filename intentionally kept as `design-ir-and-renderings.md`.

### 2026-05-11

- **Q6 (`$type` rule) — closed-as-deferred**: always emit `$type` on every JSON object node. The "omit when parent property name matches singular type" optimisation is deferred. Source: chat decision.
- **Q7 (JSON enum encoding) — resolved B2**: `{ "visibility": "public" }` (field name = JSON key, enum value = snake_case string). Source: chat decision.
- **Q8 (terminology rename) — resolved**: rename `Ir → SyntaxTree`, `DataIr → DataTree`, `SqlIr → SqlTree`. Source: chat decision. Code rename completed 2026-05-12.

*Older entries (decisions absorbed before this history block was added) live as inline notes in §10.*

---

*End of draft. Mark up freely; we'll iterate. New inline `%%` comments get a corresponding entry here when addressed.*
