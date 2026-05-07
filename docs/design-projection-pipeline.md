# IR Projection Pipeline — Architectural Design

**Supersedes** `docs/design-ir-to-dataIr-projection.md` (the iter 37
exploratory note). Written under the architecture skill: invariants
first, ownership boundaries explicit, refactor before bolt-on.

---

## 1. Invariants

The system is healthy when ALL of these are true:

1. **One projection model.** Every output format (JSON, YAML, TOML,
   tree-text, JSON-Lines) reads from a single structured-data
   representation. There is no per-format ad-hoc IR-walking.

2. **Projection is read-only.** The structured-data layer is
   produced by lowering. Editors / mutation operate on `Ir` (or
   `DataIr` for data-language inputs). The projection is
   regenerated on each render.

3. **Generic principles, not heuristics.** Projection rules are
   driven by IR variant semantics (e.g. "Skip is non-semantic
   → omitted", "Modifiers are boolean flags → key→true pairs"),
   not by post-hoc shape detection like "if the child renders to
   `{plural: [...]}`, lift one layer".

4. **`$`-prefixed JSON keys are reserved metadata.** `$type`,
   `$children`, `$truncated`. Anything else with `$` is a bug —
   internal IR concepts must not surface as user-visible keys.

5. **XML and JSON output are independent projections.** Today
   they share `to_xot` rendering for some derived data
   (`xml_to_json`); tomorrow JSON output goes through projection
   directly. XML continues to be the queryable surface for XPath.

6. **Round-trip identity for source recovery is preserved on the
   `Ir`/`DataIr` side.** The projection layer doesn't need to
   carry source-text-recovery — it's a one-way reduction for
   structured rendering. Source text comes from `Ir.range()` →
   `to_xot.rs`.

7. **Mutation surface stays on `Ir`.** `--set access=private`
   continues to operate on the typed `Ir::Modifiers.access` field.
   The projection is regenerated on each render, so mutations
   automatically flow through.

---

## 2. Ownership Boundaries

Each concern has one obvious owner:

| Concern                                | Owner                                 |
|----------------------------------------|---------------------------------------|
| Source CST → Ir                        | `tractor/src/ir/<lang>.rs` lowering   |
| Source CST → DataIr (data languages)   | `tractor/src/ir/<format>_data.rs`     |
| Ir → XML / source-text                 | `tractor/src/ir/to_xot.rs`            |
| **Ir → DataIr (projection)**           | **`tractor/src/ir/to_data.rs` (new)** |
| DataIr → JSON                          | `tractor/src/ir/data_to_json.rs`      |
| DataIr → YAML                          | `tractor/src/ir/data_to_yaml.rs` (future) |
| DataIr → XML                           | `tractor/src/ir/data_to_xot.rs`       |
| Mutation operations                    | `tractor/src/ir/types.rs` (typed setters) |
| XPath query execution                  | rendered XML (queryable surface)      |

The single new owner is `to_data.rs`: the Ir → DataIr projection.
Once it exists, `to_json.rs`'s 1000 LOC becomes obsolete — its
work moves to `to_data.rs` (one direction) plus `data_to_json.rs`
(trivial serializer).

---

## 3. Sufficiency of Current `DataIr`

The existing `DataIr` enum has 11 variants:

`Document`, `Mapping`, `Sequence`, `Pair`, `Section`, `String`,
`Number`, `Bool`, `Null`, `Comment`, `Directive`, `Element`,
`Unknown`.

**Verdict: nearly sufficient — needs two extensions.**

### 3.1 What works as-is

- **Containers.** `Ir::Module` / `Ir::Class` / `Ir::Function` →
  `DataIr::Mapping { pairs: [...] }`. Each typed slot becomes a
  `Pair { key: <slot_name>, value: lowered_subtree }`. Modifiers
  become flag pairs.
- **Sequences.** `Ir::Tuple` / `Ir::List` / `Ir::Set` /
  `Ir::Dictionary` → `DataIr::Sequence { items }` (or `Mapping`
  for Dictionary).
- **Scalars.** `Ir::Name`, `Ir::Atom`, `Ir::Int`, `Ir::Float`,
  `Ir::String`, `Ir::True`, `Ir::False`, `Ir::Null`, `Ir::None` →
  the matching `DataIr` scalar.
- **Pairs.** `Ir::Pair { key, value }` (dictionary entries) →
  `DataIr::Pair`.
- **Comments.** `Ir::Comment` → `DataIr::Comment`.
- **Unknown.** `Ir::Unknown` → `DataIr::Unknown`.

### 3.2 Gaps that need new `DataIr` variants

**Gap A: typed scalar with embedded markers** — `Ir::Binary`'s
`<op>` element carries text (`">="`) AND a marker (`"compare and
greater and equal"`). DataIr has no scalar-with-markers variant.

The current `to_json.rs` synthesizes:

```json
"op": { "text": ">=", "compare": true, "greater": true, "equal": true }
```

This is a Mapping with one well-known key (`text`) plus marker
flags. **No new variant needed** — represent as
`DataIr::Mapping { pairs: [Pair("text", String), Pair("greater",
Bool(true)), ...] }`. The op-marker decomposition (splitting
`"compare and greater and equal"` → three flag pairs) lives in
projection.

**Gap B: ordered flag-bearing leaf** — Markers like
`<break/>`, `<continue/>`, `<star/>` are zero-width or skip-only.
JSON wants them as `"break": true` flags on the parent. DataIr
already has `Bool { value: true }` for this. **No new variant
needed** — the projection emits `Pair { key: "break", value: Bool(true) }`.

**Gap C: rich text leaves with role metadata** — `Ir::Atom {
element_name: "literal", range: ... }` becomes the source-text
slice. DataIr's `String { value }` carries the parsed string.
**No new variant needed** — `DataIr::String { value: source.slice(range).to_string() }`.

**Gap D: source-text recovery for tree-text rendering.** Tree-text
output today walks XML with attribute-aware compaction. DataIr
doesn't preserve element attributes. **Tree-text continues to
walk Xot,** unaffected by this work. (Future iter could move
tree-text onto DataIr too, but out of scope here.)

**Conclusion: zero new `DataIr` variants required for the JSON path.**
Current shape is sufficient. Extensions for tree-text or future
exotic outputs can come later.

---

## 4. How Each `Ir` Concept Projects

Reference table — the projection rules in one place:

| `Ir` element                           | `DataIr` shape                                                   |
|----------------------------------------|------------------------------------------------------------------|
| `Skip { .. }`                          | (omitted from parent)                                            |
| `Inline { children, list_name: None }` | flatten children into parent's `pairs`                           |
| `Inline { children, list_name: Some(k) }` | `Pair { key: k, value: Sequence(children) }` on parent       |
| `Atom { element_name, range }`         | `String { value: source.slice(range) }`                          |
| `Name`, `Int`, `Float`, `String`       | matching `DataIr::String`/`Number`                               |
| `True`, `False`, `Null`, `None`        | matching `DataIr::Bool`/`Null`                                   |
| `Module { children }`                  | `Mapping { pairs: collect_pairs(children) }`                     |
| `Class { modifiers, name, body }`      | `Mapping { Pair(flag, true) for each marker, Pair("name", ...), Pair("body", ...) }` |
| `Function`, `Method`, `Property`       | same shape — modifier flags + typed slot pairs                   |
| `If { cond, body, else }`              | `Mapping { Pair("condition", ...), Pair("body", ...), Pair("else", ...) }` |
| `Binary { op_text, op_marker, left, right }` | `Mapping { Pair("left", ...), Pair("op", op_value), Pair("right", ...) }` where `op_value = Mapping { Pair("text", op_text), Pair(marker, true) for each marker_token }` |
| `Unary`                                | similar — `op` + `operand`                                       |
| `Is { value, type_target }`            | `Mapping { Pair("left", lowered_value), Pair("right", lowered_type) }` |
| `Tuple`, `List`, `Set`                 | `Sequence { items: lowered_children }`                           |
| `Dictionary { pairs }`                 | `Mapping { pairs: lowered_pairs }`                               |
| `Pair { key, value }`                  | `DataIr::Pair { key: lowered_key, value: lowered_value }`        |
| `SimpleStatement` (zero-width, no kids) | `Pair { key: element_name, value: Bool(true) }` (synthetic marker — collapses at projection, NOT at render) |
| `SimpleStatement` (Skip-only kids)     | same — synthetic marker (e.g. `<star/>` for `SELECT *`)          |
| `SimpleStatement` (children only)      | `Mapping` keyed by element_name with children pluralized         |
| `Modifiers`                            | flag pairs at parent level — no separate node                    |
| `extra_markers: &[&str]`               | flag pairs at parent level                                       |
| `Comment`                              | `DataIr::Comment`                                                |
| `Unknown`                              | `DataIr::Unknown`                                                |

### Plural grouping

When a Mapping accumulates multiple `Pair(k, ...)` with the same
`k`, the projection pluralizes: `[Pair("column", v1), Pair("column",
v2)]` → `Pair("columns", Sequence([v1, v2]))`. Single-occurrence
keys stay singular.

This rule lives ONCE in the projection. JSON-side has no
collision-promotion logic; YAML-side neither.

### Op marker decomposition

`op_marker` is a space-separated string like `"compare and greater and equal"`.
Projection splits on whitespace and emits one flag pair per token,
plus a `text` pair with the literal `op_text`. No more
empty-marker `""` keys (since empty strings produce zero tokens →
no flag pairs).

### `<expression>` host wrappers

Today's `Ir::Expression { inner, marker }` is a structural XML
host (Principle #15). For JSON projection, the host is invisible:
inner is projected directly, the optional marker becomes a flag
pair on the inner's parent (or, when at a slot like `right`, is
attached to the slot's value).

This eliminates `$type": "expression"` leaks (iter 31) by
construction.

---

## 5. The Spike Target

Phase 4 demands invariant-bearing slices. The first slice should
prove the architecture is sound on the smallest possible input.

**Spike target: `Ir::Class` end-to-end, behind `--projection=v2` flag.**

Why `Ir::Class` and not something smaller:
- It exercises the full set of projection concerns: modifiers
  (flags), typed slots (name, body), repeated children
  (methods/fields/properties → plural grouping), nested IR
  (parent class declarations contain bodies with more declarations).
- It's a non-trivial-but-bounded test case: a single class
  produces ~50 lines of structured JSON. Easy to diff.
- It already has dense snapshot coverage across 7 languages
  (C#, Java, Python, TS, Rust, Go, Ruby, PHP).

The spike:
1. Add `to_data.rs` with `lower_to_data_ir(ir: &Ir, source: &str) -> DataIr`.
2. Implement only `Ir::Class` and the variants reachable from it
   (Module, Body, Method, Property, Atom, Modifiers).
3. Wire a feature flag `--projection=v2` that routes Class JSON
   through `to_data → data_to_json` instead of `to_json`.
4. Add a parity test: for each existing C#/Java fixture
   containing a class, assert v2 output equals v1 output (modulo
   intentional cleanup — `$type": "expression"` leaks gone, etc.).

If parity holds, expand the spike to include `Function`,
`If`, `Binary` — covering the typical procedural-code blueprint —
and flip the flag to default on for the pilot language.

If parity reveals divergent shapes that aren't intentional
cleanup, **stop** and revisit `Ir` rather than patching the
projection. Per Phase 3: refactor structure before adding more
logic.

---

## 6. Mutation Interaction

Mutation operates on `Ir`. Projection is regenerated.

```text
   parse → Ir → [ mutate Ir.Modifiers.access ] → render
                                                  ├─ to_xot     → XML
                                                  └─ to_data    → DataIr → JSON
                                                                          → YAML
```

Each render call re-projects from scratch. There's no DataIr
mutation API and there shouldn't be — the projection is lossy
(Skip omitted, plural-grouped pairs lose their original order
hint, etc.). Projection is one-way.

---

## 7. XPath Query Interaction

XPath queries operate on rendered XML (the Xot tree from
`to_xot.rs`). Queries are unaffected by this work.

Concretely:
- `query --format xml` → walk Xot, no projection involved.
- `query --format json` → today: walk Xot + `xml_to_json`
  projection. After this work: walk Xot for the matched subtree,
  then optionally re-project that subtree through `to_data`. Or
  (simpler): when a query matches the root document, render via
  `to_data → data_to_json`; for partial matches, fall back to
  `xml_to_json` until the IR / DataIr can carry partial subtrees.

The partial-match story aligns with task #93 ("Wire xot↔IR
mapping so partial matches carry IR"). Until that's done,
JSON output for partial XPath matches keeps the old path.

---

## 8. Naming

`DataIr` was named for data languages. After this work, it's the
universal projection target.

**Decision: keep the name `DataIr` for now, retire the
"data-language" framing in the doc-comment.** The name reads as
"structured data IR" — accurate for the new role. Renaming
introduces churn across many files for marginal clarity gain;
defer until the cutover is complete.

If a rename eventually happens, candidates:

- `StructuredIr` — accurate but generic.
- `OutputIr` — emphasizes its read-only / projection role.
- `Projection` — emphasizes the data-shape role.

`StructuredIr` is the strongest contender. Decide post-cutover.

---

## 9. Slices (Phase 4)

Each slice establishes a stable invariant.

| #  | Slice                                                                                        | Invariant established |
|----|----------------------------------------------------------------------------------------------|------------------------|
| 1  | Add `to_data.rs` with `lower_to_data_ir` skeleton + `Ir::Class` arm                         | "Projection module exists, one variant covered" |
| 2  | Add `--projection=v2` feature flag in CLI + JSON renderer dispatch                          | "Two paths exist side-by-side; default unchanged" |
| 3  | Cover `Ir::Module`, `Function`, `Method`, `Property`, `Body`, `Modifiers`, `Atom`, `Name`   | "Every variant reachable from a class declaration projects via v2" |
| 4  | Parity test: C# blueprint v1 JSON ≡ v2 JSON (allowing pre-declared cleanup deltas)          | "v2 reproduces v1 output for the pilot fixture"  |
| 5  | Cover expression-side variants: `Binary`, `Unary`, `Comparison`, `Is`, `Cast`, `Inline`, `Skip` | "Every variant in Python blueprint projects"     |
| 6  | Parity tests for Python / Java / TS / Rust / Go / Ruby / PHP                                | "v2 covers all programming languages"            |
| 7  | Flip default to `--projection=v2`; add `--projection=v1` for opt-out                        | "v2 is the default JSON path"                    |
| 8  | Delete `to_json.rs`'s ad-hoc projection logic; keep only the format-glue (CLI, atomic-output) | "v1 path retired; one projection model"          |
| 9  | Add YAML / JSON-Lines via `data_to_yaml.rs` / `data_to_jsonl.rs`                            | "Projection is universal across structured outputs" |
| 10 | Update `docs/`, retire iter-37 design note as superseded                                     | "Docs reflect the implemented model"             |

Each slice is a small, focused commit. Slices 4 and 6 are gates —
parity must hold before flipping defaults.

---

## 10. What Could Go Wrong

Risks worth flagging:

- **`Ir` variant explosion.** If projection reveals that the
  `Ir` enum has many "almost-but-not-quite" variants (`Function`
  vs `Method` vs `Constructor` differing only in element_name),
  the projection arms become repetitive. Resolution: use a
  generic helper for the shared shape ("declaration-style: flags
  + name + signature + body"); keep variant-specific arms only
  where the projection genuinely differs.

- **Plural-grouping ambiguity.** Some children should not be
  pluralized — e.g. `Ir::If`'s `else` slot, even when there's
  only one, stays as `"else"` not `"elses"`. Resolution: typed
  slot pairs are emitted with explicit keys; plural grouping only
  applies to repeat-keyed pairs. Single-keyed slots stay
  singular.

- **op_text serialization quirks.** Operators like `&&`, `||`,
  `>>=` need to render as their literal text in `op.text`.
  Resolution: the projection takes `op_text: String` and emits
  it verbatim — no escaping needed (JSON's `serde_json` handles it).

- **Order observation.** Some tests assert ordinal positions of
  markers (`*[1][self::public]`). If projection reorders, tests
  break. Resolution: projection preserves declaration order from
  `Modifiers::marker_names()` (which is already canonical).

- **Performance.** Projection is an extra walk. For large files
  it doubles JSON-render time. Resolution: measure, accept a
  small regression for the architectural cleanup. If
  unacceptable, fold projection + serialization into one pass.

---

## 11. Definition of Done

This work is complete when ALL of these are true:

- ☐ `tractor/src/ir/to_data.rs` exists and covers every `Ir` variant.
- ☐ `to_json.rs`'s ad-hoc projection logic (~1000 LOC) is deleted.
- ☐ Every JSON / YAML / JSON-Lines output flows through
  `Ir → DataIr → format`.
- ☐ Snapshot diffs from the cutover are explained and intentional
  (no surprise shape changes).
- ☐ `$`-prefixed JSON keys remain only as designated metadata
  (`$type`, `$children`, `$truncated`).
- ☐ Mutation tests still pass (Ir mutation surface untouched).
- ☐ XPath query tests still pass (rendered-XML path untouched).
- ☐ Per-iter add_children heuristics removed: Skip-filter,
  Inline-transparency-in-singleton-slot, plural-of-self collapse,
  Skip-only marker collapse, op_marker emptiness check, `$type`
  slot label cleanup. All of these become natural consequences
  of the projection rules — none survive as conditionals.

The standard, per the architecture skill: the next related change
should be easier, not harder, after this work lands.

---

## Appendix A: Why Not Skip DataIr and Project Directly to JSON?

A simpler alternative: write `Ir → serde_json::Value` directly,
no DataIr intermediate. Why insert DataIr?

1. **Multiple format targets.** Once we have YAML, JSON-Lines,
   future TOML output for programming-language IR — going through
   DataIr means each of those is a tiny module, not a copy of the
   1000-LOC projection logic.
2. **DataIr is already the home for data-language projection.**
   Programming-language IR rejoining the same target unifies the
   model rather than adding a parallel one.
3. **Testing surface.** DataIr has its own structural shape that
   can be asserted independently of the JSON serialization.
   "Every Ir::Class projects to a Mapping with N pairs" is a
   stronger contract than "Every Ir::Class produces JSON object
   with N keys" because the first is JSON-output-format-
   independent.

The DataIr layer is worth its weight.

---

## Appendix B: Migration of Recent Iter Heuristics

Mapping each `add_children` special case to its principled
replacement:

| Iter | Heuristic                                                | Principled rule                                                              |
|------|----------------------------------------------------------|-------------------------------------------------------------------------------|
| 29   | Filter `Ir::Skip` from JSON children                     | `Skip → omitted` is a projection rule, not a JSON quirk                       |
| 30   | Treat `Ir::Inline` as transparent in singleton-slot      | `Inline → flattened` at projection                                            |
| 31   | Drop `$type": "expression"` from `Ir::Is` right          | `Ir::Expression` host invisible at projection (Section 4)                     |
| 31   | Drop `$type": "left"/"right"` for multi-Assign           | Multi-target Assign projects to `Sequence` directly (no slot-label mapping)   |
| 32   | Skip empty `op_marker` key                               | `op_marker.split_whitespace()` produces zero tokens → zero flag pairs         |
| 33   | T-SQL literal as `Ir::Atom` not `Ir::SimpleStatement`    | Atom → String scalar; SimpleStatement → Mapping. Was an IR fix, stays.        |
| 34   | Empty zero-width SimpleStatement → flag                  | Synthetic-marker SimpleStatement projects to `Pair(name, Bool(true))`         |
| 35   | Plural-of-self double-wrap collapse                      | Plural grouping happens at projection, only once. No double-wrap to collapse. |
| 36   | Skip-only SimpleStatement → flag                         | Same as iter 34 — covered by synthetic-marker rule                            |

Eight of nine recent heuristics evaporate. The ninth (iter 33)
was an IR-side fix that was already correct; it stays.
