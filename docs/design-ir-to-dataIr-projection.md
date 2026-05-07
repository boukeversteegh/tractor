# IR → DataIr projection — design note

**Status:** SUPERSEDED by `docs/design-projection-pipeline.md`
(written under the architecture skill, with rigorous invariants,
ownership boundaries, and slice plan). Kept as the iter-37
exploratory snapshot for history.

Captures the architectural pattern user
flagged in iter 36 review:
> json rendering should just be straightforward. maybe the IR is
> wrong? generic principles should determine how IR is converted
> to JSON. maybe we need lowering from IR to Data-IR first?

## Problem

`tractor/src/ir/to_json.rs` is ~1000 LOC of language-aware
projection logic that mixes three responsibilities:

1. **Render IR primitives** (Name → string, Int → number, ...)
2. **Project IR shape to JSON shape** (modifiers → flags,
   children → role-keyed siblings, multi-target Assign → array)
3. **Patch impedance mismatches** (Skip filter, Inline transparency,
   plural-of-self collapse, Skip-only marker collapse)

The third bucket is the smell. Each patch is a heuristic that
papers over IR/JSON impedance mismatch — the IR variants were
shaped for XML rendering needs (where post-passes handle marker
collapse and slot lifting), not JSON's natural shape.

Recent iters 29-36 cleaned up `$`-prefixed leaks (`$inline`,
`$skip`, `$type": "expression"`, `$type": "left"`, empty op
markers, marker-vs-leaf ambiguity). Each fix added a special-case
branch in `add_children`. The trend is unsustainable.

## Proposal

Insert a lowering pass `Ir → DataIr → JSON` (and YAML, etc.). Each
layer has one job:

| Layer            | Responsibility                                  |
|------------------|-------------------------------------------------|
| `Ir`             | Language semantics (Class, Function, If, ...)   |
| `Ir → DataIr`    | Project to plain structured data — apply marker collapse, slot lifting, plural grouping ONCE here |
| `DataIr → JSON`  | Trivial serializer: Mapping → object, Sequence → array, scalars |
| `DataIr → YAML`  | Same trivial serializer, different escape rules |
| `DataIr → XML`   | Existing — already targets DataIr's variants for data languages |

Today `DataIr` exists for data languages (JSON / YAML / TOML / INI).
Programming-language IR routes through `to_json.rs`'s ad-hoc
projection. The proposal is to **make `DataIr` the one
universal projection target** — programming-language IR lowers to
the same `DataIr` enum that JSON / YAML lowering already uses.

## Mapping sketch

For each `Ir` variant, the lowering rule:

| Ir variant                                  | DataIr projection                                      |
|---------------------------------------------|--------------------------------------------------------|
| `Atom`, `Name`, `Int`, `Float`              | `DataIr::String` / `Number`                            |
| `String`, `True`, `False`, `Null`, `None`   | `DataIr::String` / `Bool` / `Null`                     |
| `Skip`                                      | (omitted — Skip is render-only)                        |
| `Inline { children, list_name=None }`       | flatten children into parent                           |
| `Inline { children, list_name=Some(n) }`    | parent gets `Pair { key=n, value=Sequence }`           |
| `SimpleStatement { name, modifiers, kids }` | `Mapping` with `Pair{flag, true}` for each marker, `Pair{slot, lowered_kid}` for typed slots, plural grouping for repeated kids |
| `Function`, `Class`, `Method`, `Property`   | Same — `Mapping` with typed slot `Pair`s              |
| `If { cond, then, else }`                   | `Mapping { Pair("condition", lowered_cond), Pair("then", ...), Pair("else", ...) }` |
| `Binary { left, op, right }`                | `Mapping { Pair("left", ...), Pair("op", op_value), Pair("right", ...) }` |
| `Is { value, type_target }`                 | `Mapping { Pair("left", lowered_value), Pair("right", lowered_type) }` |

Empty zero-width `SimpleStatement` collapses to a flag during
projection (no JSON-side heuristic needed).

`OPERATOR_MARKERS` and `Modifiers` flag-emission lives in the
projection layer — once, language-agnostic.

## Migration path

1. **Spike** (≤2 iters): pick a small Ir variant (e.g.
   `Ir::Break`/`Continue`, or `Ir::Atom`), implement
   `lower_to_data_ir()` for it, render through DataIr → JSON,
   verify output matches existing `to_json.rs` for the same input.
2. **Pilot** (≤5 iters): port one whole language's blueprint
   (likely Python — already has the most coverage) to the new
   pipeline behind a feature flag. Compare snapshots.
3. **Cutover** (per-language): flip each language as its IR
   variants are all covered. Snapshot diffs land per language.
4. **Retire `to_json.rs`**: when all languages are on the new
   pipeline, delete `to_json.rs`'s projection logic — it becomes
   a thin wrapper around `lower_to_data_ir() → data_to_json()`.

## What this does NOT cover

- **Mutation** (`--set access=private`): operates on `Ir`. The
  DataIr projection is one-way (read-only render).
- **XPath queries**: continue to operate on the rendered XML
  (which still goes through `to_xot.rs` from `Ir`). The DataIr
  pass is only for JSON / YAML / structured-data renders.
- **Source-text recovery**: still handled by `Ir.range()` →
  `to_xot.rs` gap-fill. DataIr projection is independent.

## Open questions

- **Naming.** `DataIr` is named for data languages; if it becomes
  the universal projection target, `StructuredData` / `Projection` /
  `OutputIr` may fit better. Defer the rename until cutover.
- **Markers vs flags.** Today `DataIr::Element { name, markers }`
  carries marker chips for Markdown's `<heading[h1]>`. For
  programming-language IR projection, modifier flags become
  `Pair { key=marker, value=Bool(true) }`. Need to decide whether
  to keep two flag mechanisms or unify.
- **Plural grouping.** The `pluralize_list_name` rule lives in
  `transform/helpers`. Should remain there, called by the
  projection pass — projection is the only consumer.
