# Output Pipeline Architecture — Design

**Status:** Re-evaluated 2026-05-07 after user pushback exposed a
flawed premise. See "Premise correction" section below — the
"post-passes" framing was incorrect. No multi-pass IR
transformation exists. The real lever is IR shape, not
projection layering.

Original design (kept below for reference) addressed user
question:

> If Data is the universal in-between step, then logically XML
> should also be generated from the data layer. But then losing
> source tracing is absolutely not acceptable. Please design this
> carefully and don't directly implement.

This doc compares architectures, articulates the constraint
ranking, and recommends a path. **No implementation should follow
from this doc until the user approves the recommendation.**

---

## Premise correction (2026-05-07)

User asked: "does this only happen for XML? if it transforms the
IR, it would still be OK because json gets the same benefits, but
in general i thought we had gotten rid of multi pass processing".

Investigation: **there is no multi-pass IR processing.** Parse
builds the IR once; each renderer reads it independently.

- `Modifiers` struct on declarations is the typed canonical
  form. `to_xot.rs` expands it into `<public/>` empty marker
  children. `to_json.rs` projects it into flag pairs. The
  expansion happens AT RENDER TIME, not as an IR pass.
- "Marker collapse" only exists in tree-text rendering
  (`query_tree_renderer.rs`) where empty `<public/>` children
  fold into `class[public]` chip syntax. Render-time formatting,
  not an IR transform.
- Plural grouping in JSON output is a JSON-projection rule, not
  an IR transform.

The earlier framing in this doc ("IR shaped for XML rendering
needs where post-passes handle marker collapse and slot lifting")
was wrong. **There are no IR post-passes.** Apologies — sloppy
analysis on my part.

### What's actually happening

The iter 29-36 JSON heuristics trace to one root cause:

`Ir::SimpleStatement { element_name, children, modifiers,
extra_markers }` is a **generic catch-all** where `element_name`
doubles as a slot label. When you see `SimpleStatement{name="columns",
children=[<column>×N]}`:
- XML rendering loves it: `<columns><column/>...</columns>` is
  the natural shape.
- JSON projection sees the slot key "columns" outer AND the
  pluralization "columns" inner — hence the double-wrap and the
  iter-35 collapse heuristic.

Other JSON heuristics (synthetic-marker collapse, Skip-only
collapse, Inline transparency in singleton slots) trace the
same way: SimpleStatement is over-general and the renderer has
to disambiguate.

### Architectural lever options

**(I)** **Move JSON projection to `to_data.rs` (the original
proposal in this doc).** Heuristics survive but consolidate to
one file. JSON, YAML, JSONL share the projection. Cheap;
addresses the symptom; doesn't address the root cause.

**(II)** **Replace `SimpleStatement` with explicit typed IR
variants per construct.** `Ir::Insert { name, columns:
Vec<Ir>, values: Vec<Ir> }` instead of `SimpleStatement{
name="insert", children=[name_atom, columns_wrapper,
values_wrapper] }`. Both XML and JSON read typed slots directly.
Heuristics evaporate at the source. Cost: every construct
gets a typed IR variant; IR enum grows. Aligns with Principle
#11 ("specific names over abstract supertypes") which the non-
TSQL languages already follow (`Ir::Class`, `Ir::Function`,
`Ir::Property` are typed; `Ir::SimpleStatement` is the catch-all
for things not yet typed).

**(III)** **Both — type the IR AND build the projection.**
Typed IR makes JSON projection trivial; the projection module
exists to share serialization across formats. Both wins. More
work overall.

### Recommendation (revised after second-pass thinking)

The iter 41 commit recommended (II) (type all of TSQL). On
reflection that's wrong:

- TSQL has 45+ `simple_statement(...)` calls. Typing them as
  cross-language IR variants pollutes the enum — INSERT,
  SELECT, WHERE aren't cross-language concepts. They belong to
  SQL only.
- A separate `SqlIr` enum would be one alternative but adds a
  whole parallel rendering pipeline.
- The projection rules for `SimpleStatement → JSON` are
  well-defined. They're not "symptoms of bad IR design"; they're
  the projection contract for a deliberately-generic IR variant.

Pragmatic split:

- **Typed IR** for cross-language constructs (already in place
  — Class, Function, Binary, If, etc.). Each has a clean
  XML and JSON projection by virtue of typed slots.
- **`SimpleStatement` + projection** for language-specific
  constructs (TSQL clauses). The projection layer handles
  generic SimpleStatement; rules live in `to_data.rs` once,
  consumed by JSON / YAML / JSONL / TOML output.

So the original recommendation (hybrid C) stands: continue
slice 2 of the projection plan. Wire `--projection=v2` flag,
build out `to_data.rs` to cover all reachable variants, parity-
test, flip default, retire `to_json.rs`.

The iter 41 framing about "type the IR" was an over-correction
in response to the premise mistake. The premise mistake (that
"post-passes" exist) is real and acknowledged; the projection-
layer architecture is still right.

Slice 1 (`to_data.rs` + `Ir::Class`, iter 39) remains the
foundation. Slice 2 next.

---

## Original design (kept for reference)

[continued below]

---

## Constraints (ranked)

These are the architectural commitments tractor has made — every
candidate architecture must respect them.

1. **The IR is the canonical source of truth.** Mutation
   operates on IR. Source tracing (byte ranges, spans) lives on
   IR. All other representations are derived views.

2. **Source-text recovery is non-negotiable.** For any rendered
   XML, `string(rendered_root) == source[ir.range()]` — every
   source byte appears as text content somewhere in the rendered
   XML. This is the text-recovery invariant. Loss is "absolutely
   not acceptable" (user, this turn).

3. **XML is the queryable surface for XPath today.** Queries
   return Xot subtrees. Match results carry the IR (for
   root-document matches) so consumers can re-render in any
   format. Partial-match IR carrying is task #93, pending.

4. **Alternative query engines must remain cheap to add.** A
   future JQ-over-JSON or pattern-matcher-over-IR engine should
   be able to read the model and map matches back to IR positions
   via source ranges.

5. **Output formats are a long tail.** Today: XML, JSON, JSON-Lines,
   YAML, tree-text, GCC, GitHub. Tomorrow: TOML output for
   prog-lang IR, custom user formats, structured-codegen targets.
   The pipeline should make new formats cheap.

6. **Internal IR concepts must NOT leak as user-visible
   metadata.** No `$inline` / `$skip` / `$type": "expression"`
   keys in JSON. `$type`, `$children`, `$truncated` are reserved.

---

## The Question

Given the recent JSON renderer cleanup added ~9 special-case
heuristics to `to_json.rs::add_children` (iters 29-36), should
tractor:

- **(A)** Continue layering IR-aware projection on each format
  serializer (status quo). Each new format gets its own renderer
  with its own heuristics.

- **(B)** Add a universal structural IR (`DataIr` repurposed),
  route ALL outputs through it. One projection, trivial
  serializers per format.

- **(C)** Add `DataIr` projection for structured-data outputs
  only (JSON/YAML/JSON-Lines/TOML); keep XML's rich-render direct
  from IR. Two pipelines.

The user's pushback rules out a naive **(B)** — DataIr → XML
today (`data_to_xot.rs`) is **structure-only**, with `let _ =
source` and no gap-fill. Routing XML through DataIr would lose
the text-recovery invariant by construction. Fixing `data_to_xot`
to support gap-fill is non-trivial: it requires every `DataIr`
variant to faithfully represent every `Ir` variant's source
coverage, including pathological cases like `Ir::Skip`,
`Ir::Inline { list_name }`, and operator-text-with-marker
overlap.

So **(B)** is feasible only if `DataIr` is extended to be
fully isomorphic to `Ir`'s source-spanning structure. At that
point `DataIr` is no longer "data-language IR"; it's a
generalization of `Ir`. Calling that "DataIr" would be a
misnomer.

---

## Architecture Comparison

### (A) Current — three direct renderers

```text
   parse → IR ─┬─► to_xot.rs   ──► XML        ◄── XPath queries
              ├─► to_json.rs  ──► JSON       (no DataIr involved)
              └─► to_yaml.rs  ──► YAML (future)
```

- Each renderer walks IR independently.
- Source tracing: full (each renderer reads `Ir.range()`).
- Cross-format consistency: low (each renderer reinvents
  modifier→flag, plural-grouping, marker-collapse).
- Cost of new format: high (~1000 LOC of projection logic per format).

This is what produced the iter 29-36 heuristic accretion.

### (B) Universal DataIr — single projection, three renderers

```text
   parse → IR → to_data.rs → DataIr ─┬─► data_to_xot.rs  ──► XML
                                     ├─► data_to_json.rs ──► JSON
                                     └─► data_to_yaml.rs ──► YAML
```

- One projection point. Trivial serializers.
- **Source tracing PRESERVED** *only if* DataIr can carry
  every IR variant's source-range structure faithfully — which
  requires extending DataIr substantially.
- Cross-format consistency: high.
- Cost of new format: low (one new tiny serializer).
- **Risk:** large surface area to migrate. XML output shape
  must remain bit-identical for XPath query stability. Today's
  XML render uses typed knowledge (op_text+op_marker dual
  rendering, modifier ordering, expression-host wrappers,
  decorator decoration) that data_to_xot.rs doesn't know about.
  Achieving parity is months of work.

### (C) Hybrid — direct XML, DataIr for structured outputs

```text
   parse → IR ─┬─► to_xot.rs   ──► XML        ◄── XPath queries
              └─► to_data.rs ──► DataIr ─┬─► data_to_json.rs ──► JSON
                                          ├─► data_to_yaml.rs ──► YAML
                                          └─► data_to_jsonl.rs ──► JSONL
```

- XML stays on its tested rich-render path. Source tracing
  preserved for queries.
- Structured-data outputs go through DataIr — one projection,
  one heuristic site, all the recent iter 29-36 cleanups
  consolidate to the projection layer.
- Asymmetry: programming-lang IR has TWO direct render paths
  (XML directly; JSON/YAML via DataIr). Data-lang IR (already
  `DataIr`) has ONE path (skip the projection step, it's already
  in target form).
- Cost of new structured-data format: low.
- Cost of new XML-style format: medium (write a new
  IR → that-format renderer, like today's to_xot).

---

## Recommendation: (C) Hybrid, with a specific upgrade path to (B)

The hybrid is right for the next 6-12 months. The architectural
elegance of (B) is achievable but expensive, and the migration
cost is concentrated in a single hard problem: making
`DataIr → XML` faithful enough to keep XPath query semantics
stable.

The case for **(C) over (A)**:

- `to_json.rs` already has 1000 LOC of ad-hoc projection.
  Splitting that into `to_data.rs` (projection) +
  `data_to_json.rs` (trivial render) is a refactor that
  IMMEDIATELY pays off when YAML / JSONL / TOML output is added —
  each becomes a tiny module instead of a 1000-LOC clone.
- The recent iter 29-36 heuristic accretion shows the
  one-projection-per-format pattern compounds. Stopping it now
  is cheaper than later.
- Source tracing for XML stays untouched. No risk to query
  semantics.

The case for **(C) over (B)**:

- `data_to_xot.rs` today drops source bytes (`let _ = source`).
  Promoting it to be the canonical XML renderer requires:
  - Adding gap-fill logic (significant — the existing
    `to_xot.rs` is ~2500 LOC, much of it gap-fill scaffolding).
  - Representing `Ir` variants faithfully in `DataIr` (Modifiers
    ordering, op_text+marker dual rendering, expression-host
    presence, etc.).
  - Re-validating every XPath query test under the new render.
  This is a multi-week migration with high blast radius. We
  should not start it without first proving the projection
  approach works for structured-data outputs.

The case for keeping (B) as a future state:

- Once (C) is in place and proven, "extending the projection to
  feed XML rendering" is a contained next migration. The
  projection rules are already exercised; only the
  `DataIr → XML` faithfulness and gap-fill need to be added.
- Naming-wise, `DataIr` becomes `StructuredIr` or `OutputIr` at
  that point — the rename can be deferred.

---

## Constraints That Each Architecture Satisfies

|                                       | (A) | (B) | (C) |
|---------------------------------------|-----|-----|-----|
| 1. IR is canonical source of truth    | yes | yes | yes |
| 2. Source-text recovery               | yes | only if DataIr→XML gets gap-fill | yes |
| 3. XML is queryable for XPath         | yes | only if DataIr→XML preserves shape | yes |
| 4. Alternative query engines cheap    | medium — every engine needs its own projection | yes — engines read DataIr | yes — engines read DataIr OR XML OR IR directly |
| 5. New output format cheap            | no — ~1000 LOC clone | yes | yes (for structured data); medium (for XML-style) |
| 6. No `$`-leak heuristics             | no — heuristics keep accreting | yes | yes |

(C) checks every box modulo "new XML-style format" being medium-
cost. That's an acceptable trade because XML-style formats are
rare; structured-data formats are common.

---

## Alternative Query Engines (the user's specific concern)

User: "we reconstruct or lookup the original IR node and then
can render it in any way we like."

Mapping engine output → IR:

- **XPath today.** Engine returns Xot subtree. Match carries
  `Tree::Ir { ir, source, xml }` for full-document matches; just
  `Tree::Xml` for partial (task #93 pending). Rendering: each
  `Tree` variant has a per-format method.

- **JQ-over-JSON future, with hybrid (C).** Engine returns a
  JSON pointer / path. JSON was rendered via
  `IR → DataIr → data_to_json`. To map back: walk `DataIr` by
  the JSON pointer, read its `range`, look up the `IR` node
  covering that range. **Range-based lookup is the same primitive
  used everywhere in tractor today** — no new architecture
  needed.

- **Pattern-matcher-over-IR future.** Engine operates directly
  on `Ir`, no projection involved. Mapping is trivial: the
  matched node IS the IR.

Hybrid (C) does not block any of these engines. It enables JQ by
giving it a JSON view via DataIr; it doesn't prevent
direct-over-IR queries (those just use IR).

The original IR-layer benefit — "render in any way we like" —
remains: every match is an IR position; renderers pick how to
present it.

---

## Source-Tracing Story Under (C)

For each output:

| Output                  | Path                                | Tracing source                    |
|-------------------------|-------------------------------------|-----------------------------------|
| XML (full doc, partial) | IR → to_xot.rs                     | gap-fill from `Ir.range()`        |
| JSON (full doc)         | IR → to_data.rs → data_to_json.rs  | DataIr.range() carries IR's range, but JSON itself is structure-only (the user-facing format isn't source code) |
| JSON (partial XPath)    | XML subtree → xml_to_json (legacy) until task #93 lands | Xot ranges (carried via XmlNode)  |
| YAML, JSONL, TOML       | IR → to_data.rs → data_to_<fmt>.rs | DataIr.range()                    |
| Tree-text               | IR → to_xot.rs → tree-text walker  | Xot                                |
| Source-text recovery    | IR → to_xot.rs (gap-fill)          | `Ir.range()` slicing source       |

**No path loses source tracing.** XML's gap-fill stays untouched.
Structured-data outputs aren't source-code formats anyway —
their tracing is "what IR position generated this JSON value",
which is preserved via `DataIr.range()`.

---

## Migration Plan Under (C)

Three milestones, each ending in a stable invariant:

### Milestone 1: Projection works for one variant end-to-end

- Build `to_data.rs` covering `Ir::Class` + reachable scalars.
  (DONE — iter 39, slice 1.)
- Wire `--output=projected-json` flag that routes `Ir::Class` to
  `to_data → data_to_json`. Default JSON path unchanged.
- Parity test: for one C# class, both paths produce the same
  JSON modulo intentional cleanup (`$inline` etc.).

**Invariant established:** projection is feasible without
breaking any existing output.

### Milestone 2: Projection covers all programming-lang IR variants

- Cover Function, If, Body, Binary, Unary, Comparison, Is, Cast,
  Tuple, List, Set, Dictionary, Pair, Module, Inline, Skip,
  SimpleStatement, Atom, Name, scalars, Comment, Unknown.
- Cover Modifiers, op_text+op_marker decomposition, plural
  grouping, expression-host invisibility.
- Parity test against existing `to_json.rs` output for ALL
  blueprint fixtures.

**Invariant:** the new projection produces identical-to-current
JSON output for the entire test corpus.

### Milestone 3: Flip default, retire old projection

- Flip JSON default to projected-json.
- Delete `to_json.rs`'s ad-hoc projection (~1000 LOC retires).
- Add YAML / JSONL / TOML serializers as small modules.

**Invariant:** one projection, many serializers. Heuristics
removed from `add_children`.

### Future (post-cutover): converge to (B)

If at some point we want a true universal pipeline:

- Promote `data_to_xot.rs` to support gap-fill from
  `DataIr.range()`.
- Extend `DataIr` to faithfully represent every `Ir` shape
  needed for XML query stability.
- Migrate XML output from `to_xot.rs` to `data_to_xot.rs`.
- Prove XPath query parity on all integration tests.
- Rename `DataIr` → `StructuredIr`.

This is a separate multi-month project. Its cost-benefit gets
re-evaluated AFTER (C) demonstrates the projection-layer
benefits.

---

## Naming

For (C), keep `DataIr`. Once the projection is established as a
universal target across structured-data formats, the name
"DataIr" reads as "data-shape IR" — accurate.

If we ever take the (B) jump, rename to `StructuredIr`.

---

## What This Doc Doesn't Decide

- **Whether to do (C) at all.** This is the recommendation;
  user approval needed before resuming implementation.
- **Whether the recent slice 1 (`to_data.rs` for `Ir::Class`,
  iter 39) was misdirected.** It's compatible with both (C)
  and (B), so it's safe regardless.
- **Whether mutation should ever flow through DataIr.** No —
  mutation stays on IR per Constraint 1. Projection is read-only.

---

## Open Questions for User

1. **Approve (C) as the next direction?** If yes, milestone 1
   continues from slice 1 with the v2 dispatch flag.
2. **Defer (B) explicitly?** Document it as a future state with
   the cost framing in this doc.
3. **Different priority — pause projection work entirely and
   chase another track (e.g., TSQL coverage, task #92/#93)?**

The implementation pause continues until the user picks one.
