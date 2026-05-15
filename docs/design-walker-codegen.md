# Walker codegen — design

**Status:** Sketch. The walker module exists (`tractor/src/tree/walker.rs`) with hand-written accessors for `SimpleStatement` + leaves. The next phase replaces those hand-written accessors with build-time codegen driven by `#[shape(...)]` attributes on the `SyntaxTree` enum.

This document fixes the attribute DSL, the build pipeline, and the
contract between hand-written lowering and generated projection.

## Why codegen

Adding a typed field to a variant currently requires touching ~5
files in lock-step:

1. `tree/types.rs` — the enum field
2. `tree/types.rs` — `children_iter` / `range` / `span` accessor arms
3. `tree/to_xot.rs` — per-variant render function
4. `tree/to_data.rs` — DataTree projection arm
5. `tree/to_json.rs` — JSON render arm

The S16-Z23 (Java throws) fix in this branch touched all five, plus
every `SyntaxTree::Function` construction site (4 lowering files). The
construction sites need lang-specific code (CST traversal), but items
1–5 are mechanical projections of the same structural decision. The
codegen replaces 2–5 with one attribute on the field; (1) stays, (6)
stays.

## The attribute DSL

Each `SyntaxTree` variant is annotated to declare its XML / JSON
projection shape. The DSL has four field annotations and one variant
annotation. Anything beyond these is an escape hatch.

### Variant-level

```rust
#[derive(TreeShape)]
pub enum SyntaxTree {
    // Element name = literal "function". Most variants use this.
    #[shape(element = "function")]
    Function { … }

    // Element name = the value of a field. Used by variants whose
    // element name depends on context (Function.element_name picks
    // `function`/`method`; Class.kind picks `class`/`struct`/
    // `interface`/`record`).
    #[shape(element_from = "kind")]
    Class { kind: &'static str, … }

    // For variants that need a custom render — typed access chains,
    // anything where the generated code can't capture the intent.
    // The codegen emits a `fn render_X` stub that calls the named
    // hand-written function.
    #[shape(custom = "render_access_segments")]
    Access { … }
}
```

### Field-level

Four kinds, matching the four content categories from the design
discussion:

```rust
Function {
    // 1. Flag — projects as empty `<async/>` XML element with the
    //    flag's carried source position, OR as `"async": true` in
    //    JSON. Fields of type `Flag` or `Modifiers` (which expands
    //    to its named flags) carry this annotation.
    #[shape(flag)]
    async_: Flag,

    // 2. Child — singleton subtree. Box<SyntaxTree> or Option<Box<SyntaxTree>>.
    //    XML: render the child wrapped in its own element name.
    //    JSON: keyed by the field name (`"name": …`, `"body": …`).
    #[shape(child)]
    name: Box<SyntaxTree>,
    #[shape(child)]
    body: Option<Box<SyntaxTree>>,

    // 3. Children — Vec<SyntaxTree> (homogeneous OR heterogeneous —
    //    the projection rule is the same; the Rust type-system
    //    constraint is informal).
    //    XML: render each child as a sibling inside the parent. **No
    //         list wrapper element** (deliberate, per Principle #12).
    //    JSON: `"<field_name>": [item1, item2, …]`. The field name
    //          becomes the array key.
    #[shape(children)]
    parameters: Vec<SyntaxTree>,
    #[shape(children)]
    throws: Vec<SyntaxTree>,

    // 4. Range / Span — every variant has these; codegen pulls them
    //    automatically when present (no annotation needed, but
    //    `#[shape(range)]` / `#[shape(span)]` are valid for
    //    disambiguation if the field names differ).
    range: ByteRange,
    span: Span,

    // Field not annotated → ignored by the projection. Used for
    // intermediate state or for fields consumed only by lowering /
    // mutation, not by the projection (e.g. some hidden caches).
    decorators: Vec<SyntaxTree>,  // <- intentionally NOT projected
                                  //    here; decorators are walked
                                  //    only by the function's
                                  //    explicit slot order; see below
}
```

### Source-order render: ranges drive child interleaving

`#[shape(children)]` and `#[shape(child)]` slots emit children in
**source order** (by `range.start`), with gap text from the parent's
source slice filling the bytes between children. This is identical
to the existing `render_with_gaps` contract.

A variant with multiple `#[shape(...)]` slots emits them in one
merged source-order pass. The codegen wires this automatically; the
DSL doesn't expose an explicit ordering knob.

For variants whose children must be emitted in a *non*-source order
(rare — the chain-inversion variants `Access`, `Member` re-nest from
operator-precedence to source order during lowering), use
`#[shape(custom)]`.

### Modifiers as a flag bundle

`Modifiers` is a struct of `Flag` fields. Codegen treats
`modifiers: Modifiers` specially: it expands to the named flags
returned by `Modifiers::markers_with_spans()`, each emitting an empty
XML marker / JSON boolean.

This is the only collection-flag pattern; everything else is direct
field annotations.

## Generated output

For Function, the generator emits (committed to source as
`tree/walker_generated.rs`):

```rust
// AUTO-GENERATED by build.rs from #[shape(...)] attributes on
// SyntaxTree variants. Do not edit by hand; regenerate via
// `task gen:walker`. The generation source is the SyntaxTree enum
// in tree/types.rs.

pub fn element_name_of(tree: &SyntaxTree) -> Option<&'static str> {
    match tree {
        SyntaxTree::Function { element_name, .. } => Some(element_name),
        SyntaxTree::Class { kind, .. } => Some(kind),
        SyntaxTree::Name { .. } => Some("name"),
        // … one arm per variant
    }
}

pub fn flags_of(tree: &SyntaxTree) -> Vec<Marker> {
    let mut out: Vec<Marker> = Vec::new();
    match tree {
        SyntaxTree::Function { modifiers, span, .. } => {
            for (name, marker_span) in modifiers.markers_with_spans() {
                out.push(Marker { name, range: ByteRange::synthetic_empty(),
                                  span: marker_span.unwrap_or(*span) });
            }
        }
        // … one arm per variant; each pushes Modifiers flags +
        //    per-variant Flag fields + extra_markers entries
    }
    out
}

pub fn children_of(tree: &SyntaxTree) -> Vec<&SyntaxTree> {
    let mut out: Vec<&SyntaxTree> = Vec::new();
    match tree {
        SyntaxTree::Function { name, generics, parameters, returns,
                               throws, body, .. } => {
            out.push(name);
            if let Some(g) = generics { out.push(g); }
            out.extend(parameters);
            if let Some(r) = returns { out.push(r); }
            out.extend(throws);
            if let Some(b) = body { out.push(b); }
        }
        // … one arm per variant
    }
    out.sort_by_key(|c| c.range().start);
    out
}
```

The generated file is in source control. Reviewers see exactly what
the codegen produced; debuggers step into real `.rs` line numbers.

## Build pipeline

```
build.rs
  └─ reads tractor/src/tree/types.rs via `syn`
     ├─ parses the SyntaxTree enum
     ├─ extracts each variant's #[shape(...)] attributes
     ├─ produces tractor/src/tree/walker_generated.rs
     └─ Cargo rebuilds on types.rs changes via println!("cargo:rerun-if-changed=…")
```

Equivalent `task gen:walker` regenerates the file manually (for
out-of-CI cycles). CI runs `task gen:walker` and fails if the
generated file diverges from the committed version — same enforcement
model as `task gen:kinds`.

The build script is small: ~150 LOC of `syn` walks + `proc-macro2`
quote output. The pattern follows
`tractor/build.rs` for `gen:kinds` (which already exists).

## Escape hatches

Two: `#[shape(custom = "render_X")]` on a variant delegates rendering
to a hand-written `render_X` function; `#[shape(skip)]` on a field
makes the codegen ignore it (for fields consumed only by lowering or
mutation, not by projection).

Use sparingly. If three variants need `custom`, the DSL is missing a
feature — extend the DSL rather than proliferating custom escapes.

## Migration order

1. **Land the hand-written walker** (current state — `walker.rs` with
   `element_name_of` / `flags_of` / `children_of` plus a `NeedsLegacy`
   eligibility classifier). The accessor functions are real Rust today;
   only the `SimpleStatement` + leaf arms are eligible.

2. **Migrate variants to the walker one-by-one.** For each variant:
   - Move its renderer-side wrapper synthesis into lowering (the
     Assign / Class.bases work — steps 2 and 3 of the unified
     renderer plan).
   - Flip its `walker_eligibility` arm from `NeedsLegacy` to
     `Eligible`.
   - Add hand-written accessor arms in `walker.rs`.
   - Run snapshot parity; commit.

3. **Once every variant is `Eligible`**, retire the per-variant
   `render_tree_*` functions in `to_xot.rs`. The walker is the only
   path.

4. **Then introduce the codegen.** Add `#[shape(...)]` attributes to
   each variant; replace the hand-written accessors in `walker.rs`
   with `walker_generated.rs` produced by `build.rs`. The accessors'
   API stays identical; only their authorship changes.

5. **Extend codegen to drive `to_data` and `to_json`.** Same
   attributes, two more generated files. Three projections from one
   declaration.

The codegen is the *last* step — it formalises a pattern we've
already validated against the hand-written walker. Doing it first
would have us iterating the DSL while still discovering edge cases.

## Open questions

- **Range derivation for synthetic wrappers.** When lowering
  constructs `SimpleStatement("left", [Expression(target)])`, what's
  the range? Currently we copy the wrapped target's range; the codegen
  expects each shape node to have a range. This is fine but worth
  pinning explicitly in the DSL semantics.

- **`children_of` ownership.** The current walker returns
  `Vec<&SyntaxTree>` — references into the variant's fields. Variants
  with synthesised wrappers (Assign today) would need to materialise
  the wrapper into a real tree node first (during lowering, per
  steps 2/3). Confirmed: no on-the-fly materialisation in the
  accessor.

- **Codegen for `to_data`.** DataTree's shape differs from XML enough
  that the same attributes may not suffice. May need
  `#[data_shape(...)]` as a separate namespace.

- **Versioning.** The generated file format will evolve as the DSL
  does. We commit the generated file, so version compatibility is
  enforced at PR review (a stale generated file is a visible diff).
