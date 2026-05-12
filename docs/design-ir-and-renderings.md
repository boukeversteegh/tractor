# IR and Renderings — Role Distinction

**Status:** Draft for iteration. Not yet committed to specs.

**Purpose.** Today's design docs in `specs/tractor-parse/tree/design.md`, `specs/tractor-parse/dual-view/data-branch/*.md`, and `specs/codexpath/cli/output-options/json-format/*.md` write principles in terms of XML element shapes (`<class>`, `<body>`, `list="X"`, etc.). After the IR migration (S3 / S4 / S5), `Ir`/`DataIr`/`SqlIr` are the canonical typed structures — XML and JSON are derived views. This doc proposes how to disentangle the two: which principles describe the IR (the data) and which describe each rendering of it.

**How to use this doc.** Mark up inline with your comments. Use any markup that's clear (`<!-- COMMENT: ... -->`, `>>` quotation, or just edit/add prose). When the design feels stable, we lift relevant pieces into `specs/` and retire / rewrite the affected source-of-truth docs.

---

## 1. The core distinction

| Layer                                           | Role                                                                                                                                                                                                                                                                                                                             | Owns                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| ----------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **IR** (`Ir` / `DataIr` / `SqlIr`)              | Canonical typed structure of source code. Single source of truth for "what is this construct, and what are its parts." Carries source ranges + spans for tracing. Mutation operates here. It ensures that transformations happen consistently at all levels, it drivers the semantic transformation rules, and the type system ) | Concept identity (`Ir::Class`, `Ir::Function`, `Ir::Body`); slot structure (named fields, `Vec<Ir>` lists); markers (typed enums for mutually-exclusive variants); source ranges. ! in other words it describes the tree nodes and their hierarchy, it determines what is expressible in queries, and authorative as to what the structure of queries will look like, but the specific syntax needed for each query language is not determined by IR. %% for example whether to encode a boolean value as an attribute or an empty marker element is XML's decision %% |
| **XML rendering** (`to_xot`)                    | Mechanical projection of IR to a queryable XML tree. Optimised for XPath ergonomics.                                                                                                                                                                                                                                             | Element name = IR variant name; text recovery via gap-fill; attribute encoding (`@line`, `@column`, `@list`, `@key`); the `<expression>` host insertion (if kept).                                                                                                                                                                                                                                                                                                                                                                                                     |
| **JSON rendering** (`to_data` → `data_to_json`) | Mechanical projection of IR to JSON. Optimised for JSON-tool ergonomics (jq, downstream readers).                                                                                                                                                                                                                                | JSON key naming = IR field name; scalar-vs-object decisions; array shape; no `$type` metadata.                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| **Source rendering** (`ir::source`)             | Mechanical projection of IR back to source text (anchored or canonical).                                                                                                                                                                                                                                                         | Whitespace / formatting / language-specific syntax.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |

**Mechanical means:** renderers don't make policy decisions about shape. If `<body>` is in the queryable shape, it's because there's an `Ir::Body` variant. If a `<block/>` marker appears, it's because the IR carries a typed enum whose variant maps to that name. `to_xot` does not insert wrappers without an IR antecedent, nor drop wrappers based on their name.

%% claude: incorporating your feedback into the table prose. Cleaner phrasing of the IR row to roll into §1.5 below: "IR is authoritative for tree structure (what nodes exist, what slots they have, how they nest) and for the queryable shape (what queries can express). It is NOT authoritative for query syntax — whether a boolean attribute encodes as `<class public='true'/>` or as a marker child `<class><public/></class>` is XML's choice; whether `parameters` shows as a JSON array or a numbered object is JSON's choice. Renderers OWN those encoding decisions, but every encoding must derive mechanically from an IR field; renderers don't invent shape." %%

### 1.5 What IR is authoritative for, and what it isn't

To make the boundary unambiguous:

| IR is authoritative for | IR is NOT authoritative for |
|---|---|
| Tree node types and hierarchy (what nodes exist, what their parts are) | XML element naming convention (lowercase, hyphenation rules) |
| Slot structure (named typed fields, lists vs singletons) | Whether to encode booleans as XML attributes vs marker elements |
| Marker enums and their values | Whether JSON renders a marker as `{public: true}` or `{visibility: "public"}` |
| What is queryable (every queryable thing has an IR slot) | Query syntax of any specific engine (XPath / JQ / future) |
| Source ranges and spans for tracing | Per-renderer attributes for source location (`@line`, `@column` etc. are XML's call) |
| Mutation surface — IR is the only mutable representation | How a renderer chooses to splice / regenerate output after mutation |
| Driving cross-format transformations (IR → IR rewrites apply to all renderings simultaneously) | Per-renderer canonical-form decisions (e.g. how to indent, how to break lines) |

---

## 2. IR's goals (the new top-level)

Reframing today's `semantic-tree/design.md` "Design Goals" to be IR-shape-centric — renderings inherit these.

1. **Concept faithfulness.** Every IR variant matches a developer concept (`class`, `function`, `if`, `binary`). No abstract supertypes (`expression`, `declaration`) at the baseline. Surface variants narrow a stable concept; they don't replace it. *(Today's Goal #5; Principle #11.)*
2. **Cross-language uniformity.** The same concept produces the same IR shape across languages, modulo intentional syntax markers (`BodyForm::Block` for brace-bodied, no marker for colon/indent). Cross-language queries Just Work. *(Today's Goal #4; Principle #5.)*
3. **Source reversibility.** `ir.range().slice(source)` plus the recursive structure recovers the original text. Markers replacing keywords keep the keyword as gap text. Mutation rewrites bytes through the IR's range. *(Today's Goal #7.)*
4. **Cardinality independence.** A variant's structural shape doesn't depend on how many children it has. One method or twelve, the parent's slot layout is identical. *(Today's `cardinality-independence.md`.)*
5. **Marker exhaustiveness via typed enums.** Where mutually-exclusive surface forms exist, the variant carries an `Option<EnumKind>` (or `EnumKind` with a default). The marker name is *derived* from the enum variant — never a free-form string. Adding a form is one enum variant; the renderer follows mechanically. *(Today's Principle #9, refined.)*
6. **Lossless reconstructibility through renderings.** Every rendering must allow reconstruction of the original IR (i.e. trace back any matched subtree to source). Internal projection plumbing (`$inline`, `$skip`, etc.) does NOT leak. `$type` IS legitimate metadata identifying the tree-node type. **Decided 2026-05-11:** always emit `$type` on every JSON object node. The "omit when parent property name matches singular type" optimisation is on the table but deferred — start with always-emit for simplicity and unambiguous round-trippability. Goal: the JSON shape is theoretically describable by a TypeScript interface — every slot has a known type, every child is identifiable.

<!-- COMMENT: ... -->

---

## 3. Projection contracts (mechanical, not policy)

Each renderer walks IR variants by uniform rules. No special-cased behaviour keyed by variant name; every rule below is generic over the IR's enum structure.

### 3.1 XML / `to_xot`

- `TreeNode::Foo { children, slot_a, slot_b, ... }` → `<foo>{slot_a}{slot_b}{children}</foo>`. Element name = node-type name (snake_case via strum), no exceptions.
- **Semantic enum fields** (named for the concept they encode, not "marker"). E.g. `visibility: Visibility` with variants `Public` / `Private` / `Protected`, or `form: Option<BodyForm>` with variants `Block` / `Pass`. The XML renderer emits each present enum *value* as an empty-element child: `Visibility::Public` → `<public/>`, `BodyForm::Block` → `<block/>`. The XML doesn't see the field name (`visibility`); only the enum-value name. `Vec<EnumKind>` fields emit one marker per element, preserving source order. Enums in `Option<T>` simply emit nothing when None.
- Slot fields with structural meaning (`Binary { left, op, right }`) emit as named wrapper elements (`<left>`, `<op>`, `<right>`). The IR field name IS the wrapper element name; no XML-only insertions.
- Gap text between source-derived siblings emits to preserve byte-by-byte recovery (existing `render_with_gaps` mechanism).
- Source location attributes (`line` / `column` / `end_line` / `end_column` / `range`) derive from the node's `Span` and `ByteRange`.

**~~Open question A~~ → Resolved.** Keep `TreeNode::Expression` as a real IR node (option A1).

> **Rationale (your call):** the IR represents the queryable structure; conceptually queries should be the same between XPath and JQ. If we need an expression container for value-positions to host markers (`non_null`, `await`), it belongs in the IR — not a typed wrapper or a render-time insertion. A2 (typed slot wrapper) and A3 (drop entirely) rejected: A3 because markers must modify conceptual objects, not primitive values; A2 because there's no real benefit over just having the variant.

<!-- COMMENT on Open question A: ... -->

### 3.2 JSON / `to_data` → `data_to_json`

- `TreeNode::Foo { children, slot_a, slot_b, ... }` → `{ "$type": "foo", slot_a: …, slot_b: …, children: [...] }`. Key name = field name (with optional rename if Rust-keyword conflict like `then_` → `then`).
- **`$type` rule (decided 2026-05-11):** always emit `"$type"` for now. The "omit when parent property name matches singular type" optimisation is deferred — see Q6, now closed-as-deferred. Always-emit is simpler, unambiguous, and round-trippable.
- **Semantic enum fields — B2 (string-style, decided 2026-05-11):** `visibility: Visibility::Public` → `{ "visibility": "public" }`. The IR field name appears as the JSON key; the enum value's snake_case name is the string value. Optional fields with `None` → omitted entirely. `Vec<EnumKind>` → array of strings.
  - Trade-off accepted: query asymmetry vs XPath (`//class[public]` ↔ JSON `class.visibility === "public"`). The XPath syntax stays — JSON path is its own dialect.
- Anonymous-vec-only nodes (`Tuple`, `List`, `Set`) → JSON array. Otherwise → JSON object.
- Scalar-leaf nodes → bare JSON scalar.
- `$inline` / `$skip` / projection plumbing names DO NOT appear in JSON. Those are internal to `to_data`'s rules, not data.

### 3.3 Source / `ir::source`

- **Anchored mode** (default): slices `ir.range()` from input verbatim — byte-identical roundtrip.
- **Canonical mode**: per-language formatter logic that consumes IR nodes directly (no XML/JSON in the loop). Used when re-emitting after structural mutation, when source isn't available, or when canonical formatting is desired.

### 3.4 Examples — IR ↔ XML ↔ JSON

Concrete cases showing the conversion contracts side-by-side. (Source-text rendering omitted; that's per-language formatting and lives in `ir::source`.) IR is shown in pseudocode (`TreeNode::*`) for readability; in actual Rust today the type is `Ir::*` (terminology rename proposal in §9).

#### Example 1 — public class with one field

**IR**
```rust
TreeNode::Class {
    visibility: Visibility::Public,
    name: Box::new(TreeNode::Name { range: 6..9, ... }),  // source: "Foo"
    body: Box::new(TreeNode::Body {
        form: Some(BodyForm::Block),
        children: vec![
            TreeNode::Variable {
                visibility: Visibility::Private,
                type_ann: Some(Box::new(TreeNode::Name { range: 18..21, ... })),  // "int"
                name: Box::new(TreeNode::Name { range: 22..23, ... }),            // "x"
                value: None,
            },
        ],
    }),
}
```

**XML** (`<public/>` / `<private/>` / `<block/>` are enum-value markers; `<name>` is a slot wrapper; `<body>` and `<variable>` are node elements):
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

Notes: `name` and `type` slots dereference scalar `Name` to a bare string (TS-interface: `name: string`). Enum fields use their IR field name as the JSON key with the enum value as a snake_case string. `$type` always emitted on every object node — the parent-slot omit-rule (Q6) is deferred.

#### Example 2 — function with two parameters

**IR**
```rust
TreeNode::Function {
    visibility: Visibility::Public,
    name: ...,                 // "add"
    parameters: vec![
        TreeNode::Parameter { name: ...("a"), type_ann: ...("int"), default: None },
        TreeNode::Parameter { name: ...("b"), type_ann: ...("int"), default: None },
    ],
    returns: Some(...),         // "int"
    body: ...,
}
```

**XML** (per Bucket-B Principle #12: list slot is flat — no `<parameters>` wrapper; each `<parameter>` carries `list="parameters"` for cardinality independence):
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

**IR**
```rust
TreeNode::Body {
    form: None,                 // colon/indent (Python) or unmarked
    children: vec![
        TreeNode::Variable {...},
        TreeNode::If {...},
        TreeNode::Return {...},
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

**IR**
```rust
TreeNode::Binary {
    left: Box::new(TreeNode::Expression {
        inner: Box::new(TreeNode::Name { range: 0..1, ... }),  // "a"
    }),
    op: BinaryOp::Plus,
    op_text: "+".to_string(),
    op_range: 2..3,
    right: Box::new(TreeNode::Expression {
        inner: Box::new(TreeNode::Name { range: 4..5, ... }),  // "b"
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

| IR | XML | JSON |
|---|---|---|
| `TreeNode::Name { range: 0..3 }` (text "Foo") | `<name>Foo</name>` (when standalone); when inside a `<name>` slot, the wrapper IS the name element so the inner becomes raw text | `"Foo"` (bare scalar; parent slot's TS type is `string`) |
| `TreeNode::Int { range: 0..2 }` (text "42") | `<int>42</int>` | `42` (bare number) |
| `TreeNode::String { range: 0..5 }` (text `"hi"` with quotes) | `<string>hi</string>` (escapes resolved) | `"hi"` |
| `TreeNode::Null` | `<null/>` | `null` |

#### Example 6 — list of attributes (decorators)

**IR** (suppose `decorators: Vec<TreeNode>` directly contains expression nodes after S11-Z3):
```rust
TreeNode::Function {
    decorators: vec![
        TreeNode::Call { callee: ...("app.route"), arguments: vec![...] },
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

## 4. What IR variants must encode

Based on the rules above, an IR variant's typed shape carries everything a renderer needs. Specifically:

1. **Concept identity** — the variant name itself (`Class`, `Function`, `Binary`).
2. **Sub-variant kind** — for variants whose surface form has mutually-exclusive variations, an `Option<EnumKind>` field whose variants name the markers (e.g. `BodyForm::{Block, Pass}`).
3. **Independent surface flags** — for variants with multiple non-exclusive markers, a `Vec<EnumKind>` field where each enum variant maps to one marker.
4. **Named typed slots** — single-value relationships (`condition: Box<Ir>`, `value: Option<Box<Ir>>`).
5. **List slots** — multi-value relationships (`children: Vec<Ir>`, `parameters: Vec<Ir>`). The field name is the JSON key and the XML element-name plurality.
6. **Source ranges** — `range: ByteRange`, `span: Span`. Always present.

Anything that doesn't fit one of these six categories is suspicious — it likely encodes either a Rust-API ergonomic (drop or convert) or a legacy XML quirk (move to renderer-only).

<!-- COMMENT: ... -->

---

## 5. Re-bucketing the existing principles

Today's `semantic-tree/design.md` has 7 goals + 19 principles + 16 decisions. Each falls into one of three buckets.

### Bucket A — Tree structure principles %% claude: renamed per your suggestion. See §9 below for the Tree / TreeNode terminology proposal. %%

| Today's principle                                       | What it says about IR                                                                                                                                                                                              |
| ------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| #5 Unified Concepts                                     | Same concept = same IR variant %% I find the term "IR Variant" not clear. This is referring to the node type. IR should be renamed to Tree, which consists of TreeNodes. so we have TreeNode::Class for example.%% |
| #6 Preserve Language Idioms                             | What variants exist %% tree nodes %%                                                                                                                                                                               |
| #9 Exhaustive Markers for Mutually Exclusive Variations | **Refined:** typed enums in IR (one enum per mutex group) %% i.e. avoid booleans because then xml doesn't know what to call the 'false' marker                                                                     |
| #11 Specific Names Over Type Hierarchies                | Variant naming                                                                                                                                                                                                     |
| #12 Flat Lists Over Wrapper Elements                    | IR has direct `Vec<Ir>` slots, no grouping wrappers                                                                                                                                                                |
| #13 Annotation Follows Node Shape                       | Markers attach to the variant they describe %% node %%                                                                                                                                                             |
| #15 Markers Live in Stable, Predictable Locations       | Markers as enum fields on the parent variant                                                                                                                                                                       |
| #17 Avoid Compound Node Names                           | Variant naming %% this means also no camelcase %%                                                                                                                                                                  |
| #18 Name Relationships After the Operator               | Variant naming                                                                                                                                                                                                     |
| #19 Wrap Role-Mixed Text-Leaves                         | IR slot structure for ambiguous text positions                                                                                                                                                                     |
%% 12 is an XML concern. list wrappers that contain unique lists are not encoded as a wrapping element. so class/attribute instead of class/attributes/attribute. in json the container name is explicit, and the element types are implicit: class.attributes = [{...attribute data},]

%% claude: agreed — moving #12 to Bucket B in the next iteration. The IR-side principle stays as "list slots are typed `Vec<TreeNode>` directly under the parent node, not under a grouping wrapper node" (which is a real IR principle: there's no `TreeNode::Parameters { items: Vec<TreeNode> }` wrapper, just `Function { parameters: Vec<TreeNode> }` directly). The XML-flat-encoding (`<parameter list="parameters">` siblings, no `<parameters>` wrapper) is the XML projection of that IR rule. The JSON encoding (`parameters: [...]` array under explicit key) is the JSON projection. Same IR, different renderings. Will split into two notes when the doc moves to specs. %%

### Bucket B — XML projection principles (move under "XML Rendering")

| Today's principle | What it says about XML projection |
|---|---|
| #1 Use Language Keywords | XML element name = IR variant name (which IS the language keyword by Principle #1's spirit) |
| #2 Full Names Over Abbreviations | XML naming convention (inherited from IR variant naming) |
| #3 Always Lowercase | XML element name convention |
| #4 Elements Over Attributes | XML encoding choice (when to use children vs attrs) |
| #7 Modifiers as Empty Elements | XML projection of the IR's marker enums |
| #8 Renderability | XML output reversibility (text recovery) |
| #10 Marker Source Locations | XML attribute encoding for marker source ranges |
| #14 Namespace Vocabulary | XML naming for compound concepts |
| #16 Optimize for Repeated Patterns | XML form for recurring patterns |

### Bucket C — Goals that span layers (stay at top)

| Today's goal | Inherited by |
|---|---|
| #1 Intuitive Queries | Any query interface (XPath, JQ, future) |
| #2 Readable Tree Structure | Any rendering |
| #4 Minimal Query Complexity | Any query interface |
| #5 Match Developer's Mental Model | Primarily IR; renderings inherit |
| #7 Source Reversibility | IR carries the ranges; renderings recover via gap-fill or anchoring |

<!-- COMMENT on the bucketing: ... -->

---

## 6. Migration plan (small steps, each its own commit)

1. **Add `specs/tractor-parse/ir-and-renderings.md`** with the role distinction (this doc, polished). Reference it from `semantic-tree/design.md`'s preamble.
2. **Annotate `semantic-tree/design.md` in place.** Add an `[A]` / `[B]` / `[C]` tag next to each existing goal/principle. No content move yet.
3. **Annotate `dual-view/data-branch/*.md` and `output-options/json-format/*.md`.** Mark which paragraphs are IR-shape-talking and which are XML/JSON projection-talking. Many paragraphs say "X becomes `<x>`" but really mean "X is `Ir::X` and renders as `<x>`."
4. **Promote the projection contracts** (§3 above) to a stable section in `ir-and-renderings.md`. Reference from `to_xot.rs`, `to_data.rs`, `data_to_json.rs`, `ir/source/*.rs` doc comments.
5. **Per-IR-variant doc-comment uplift.** Each `Ir::Variant` doc currently says "renders as `<variant>...`" — split into "IR shape: ..." and "XML rendering: ..." so the IR docstring is true regardless of which renderer is being read.
6. **Retire XML-specific examples in IR-shape principles.** For each Bucket-A principle whose example is XML (`<method><parameter list="parameters">…`), add a parallel JSON example so the principle reads as "data shape" not "XML shape."

<!-- COMMENT on migration plan: ... -->

---

## 7. Connections to other design docs

After the 2026-05-12 doc-consolidation commit (`fbed305a`), the relevant doc landscape:

**Pipeline & architecture (`docs/`):**
- **`docs/pipeline-architecture.md`** — canonical pipeline reference (rewritten 2026-05-12). Describes the data-processing pipeline (CLI → parse → query → render). The tree-types section there names `Ir` / `DataIr` / `SqlIr` and will need scrubbing under S12.
- **`docs/transform-validation-architecture.md`** — regression archetypes + NodeRole content (rewritten 2026-05-12). Contains a handful of IR references.
- **`docs/design-projection-pipeline.md`** — articulates the three projection architectures (A: per-format renderers, B: universal DataIr, C: hybrid) and recommends C. This proposal aligns with C and refines the tree side of it. Heavy IR vocabulary (~105 mentions); largest doc in the S12 sweep.
- **`docs/design-transform-redesign-exploration.md`** — ADOPTED-banner historical doc explaining why we moved from imperative xot mutation to typed trees. Body intentionally frozen for accuracy. **S12 policy question: scrub or leave alone?** (Q11 below.)

**Specs (`specs/tractor-parse/`):**
- **`specs/tractor-parse/tree/design.md`** — the principle catalogue this proposal re-buckets (§5).
- **`specs/tractor-parse/tree/transformations.md`** — describes per-language CST→semantic transforms; in the new framing, these are CST→tree lowering rules (the tree is the post-transform state).
- **`specs/tractor-parse/tree/chain-inversion.md`** — moved into the spec dir from `docs/` on 2026-05-12. Spec for the `Ir::Access` left-deep chain shape; will become `SyntaxTree::Access` under S12. Cross-link from `design.md`.
- **`specs/tractor-parse/dual-view/data-branch/*.md`** — describes the data-tree XML shape; in the new framing, those are `DataTree → XML` projection rules.
- **`specs/codexpath/cli/output-options/json-format/*.md`** — describes JSON shape; in the new framing, those are `tree → JSON` projection rules.

**Deleted on 2026-05-12 (so this doc doesn't reference them, but readers tracking the design history should know):**
- `docs/design-ir-to-dataIr-projection.md` — superseded by `design-projection-pipeline.md`.
- `docs/data-multi-view-impl-plan.md` — impl plan for a shipped feature.

<!-- COMMENT on connections: ... -->

---

## 9. Terminology — drop "IR", use per-domain tree types (decided 2026-05-11)

**Decision.** The "IR" framing leaks an implementation term ("intermediate representation") into the API. Replace with **per-domain tree types**, each containing **nodes**.

### 9.1 The four tree types

| Tree | Sources | Replaces |
|---|---|---|
| **`SyntaxTree`** | Python, C#, Java, TS/JS, Rust, Go, Ruby, PHP | `Ir` |
| **`DataTree`** | JSON, YAML, TOML, INI, `.env` | `DataIr` (Markdown moves out) |
| **`SqlTree`** | T-SQL (more SQL dialects later) | `SqlIr` |
| **`DocumentTree`** | Markdown (now), HTML (later); possibly RST/AsciiDoc | new — extracted from `DataIr`'s Markdown lowering |

Each tree is a Rust enum; each variant is a "node". `SyntaxTree::Class { ... }`, `DocumentTree::Heading { ... }`, etc.

### 9.2 Sweep

The "IR" term disappears from **code, comments, and docs**. Scope:

- Type renames: `Ir → SyntaxTree`, `DataIr → DataTree`, `SqlIr → SqlTree`. New `DocumentTree` extracted from the Markdown lowering currently in `DataIr`.
- Module: `crate::ir → crate::tree`. Submodules: `tree::syntax`, `tree::data`, `tree::sql`, `tree::document` (or flat — TBD).
- Supporting renames: `IrFamily → TreeKind` (variants `Syntax` / `Data` / `Sql` / `Document`), `lower_ir_* → lower_*`, `to_xot::render_ir_* → render_tree_*`, `data_ir → data_tree`, file names `ir_*.rs → tree_*.rs` or repositioned.
- Vocabulary in docs: "IR" → "tree" or specific tree type; "IR variant" → "tree node"; "IR shape" → "tree structure"; "IR pipeline" → "tree pipeline"; "intermediate representation" disappears.
- Doc-scrub targets (post-consolidation 2026-05-12 inventory):
  - `docs/pipeline-architecture.md` — 15 IR mentions; rewrite to use the new tree names.
  - `docs/transform-validation-architecture.md` — 4 IR mentions; light touch.
  - `docs/design-projection-pipeline.md` — 105 IR mentions; heaviest single doc. Will largely become a tree-projection design doc after sweep.
  - `docs/design-transform-redesign-exploration.md` — 156 IR mentions; ADOPTED-banner historical doc. **Q11: scrub or leave frozen?**
  - `specs/tractor-parse/tree/*` → renames to `specs/tractor-parse/tree/*` (Q10 resolved 2026-05-12). Subspecs scrubbed for IR vocabulary; later passes may split into `syntax-tree.md` / `data-tree.md` / `sql-tree.md` / `document-tree.md` subdivisions.
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
| Q8. Terminology rename | ✅ resolved 2026-05-11: rename now (see S12) |
| Q9. `IrFamily` rename | ✅ resolved 2026-05-12: `TreeKind` with variants `Syntax` / `Data` / `Sql` / `Document` |
| Q10. `specs/tractor-parse/tree/` directory | ✅ resolved 2026-05-12: rename to `specs/tractor-parse/tree/`. Subspecs can be reorganised underneath (`syntax-tree.md`, `data-tree.md`, etc.) as the sweep proceeds. |
| Q11. Scrub `docs/design-transform-redesign-exploration.md`? | ✅ resolved 2026-05-12: leave body frozen for historical accuracy; add a terminology-note banner at top mapping `Ir → SyntaxTree`, `DataIr → DataTree`, `SqlIr → SqlTree`. |
| Q12. Markdown tree name (provisional answer pending your call) | ✅ resolved 2026-05-12: `DocumentTree` (shared with future HTML) |

<!-- COMMENT here: ... -->

---

*End of draft. Mark up freely; we'll iterate.*
