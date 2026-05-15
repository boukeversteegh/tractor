# Walker codegen — design

**Status:** In-progress hard switch. The hand-written walker is being deleted; the renderer drops to a no-op stub; build-time codegen driven by `#[shape(...)]` attributes on the `SyntaxTree` enum becomes the SOLE path. No gradual `WalkerEligibility` ramp.

**Core principle:** the XML / JSON / YAML renderers MUST NOT change the tree structure. Any divergence between renderer output and expected shape is fixed by **changing the tree** at lowering, never by adding rules to the renderer or codegen. The codegen reads `#[shape(...)]` annotations mechanically; if a variant doesn't fit, the variant must change. The renderers are projections of the tree — not transformers of it. Source renderer (`tree::source`) is exempt because it reconstructs the original byte sequence.

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

## Migration order (revised — hard switch)

The earlier gradual ramp (one variant at a time, hand-written
walker accessors, eligibility classifier) **kept allowing semantic
leaks** — special cases for List / Set / Dictionary's `[literal]`
marker, slot-unwrapping in JSON projections, etc. Every gradual
step gave a new place for "just this one variant" rules to slip
in. The revised plan removes the temptation by deleting the
gradual path:

1. **Strip the renderer to nothing.** `render_to_xot` becomes a
   no-op stub (emits a placeholder root or nothing). Delete every
   `render_tree_*` function. Delete `walker.rs`. The code compiles
   but most snapshots / tests break — that's expected. There is
   no per-variant rendering knowledge left in the codebase.

2. **Annotate the enum.** Add `#[shape(...)]` attributes to every
   `SyntaxTree` variant per the DSL above.

3. **Build the codegen.** `tractor/src/bin/gen_walker.rs` parses
   `tree/types.rs` with `syn`, reads the annotations, and emits
   `tree/render_generated.rs` containing the three accessors and a
   `render_generic` function. Add `task gen:render-xml` + a CI
   `--check` gate that fails if the committed generated file is
   stale.

4. **Wire the generated renderer in.** `render_to_xot` delegates
   directly to `render_generic`. No fallback. No legacy. Every
   variant renders through one mechanical path.

5. **Iterate at the tree.** Diff the generated output against the
   pre-strip snapshots. For every difference, the fix is at the
   *lowering* / *tree-structure* side: add a marker child, lift a
   slot wrapper into the tree, replace a `bool` field with a `Flag`
   on a sub-variant. The codegen and the walker do not change;
   adding rules to either re-introduces the leak.

6. **Extend codegen to `to_json` / `to_data`.** Same `#[shape(...)]`
   attributes drive two more generated files. Per-variant code in
   `to_json.rs` / `to_data.rs` retires.

Past commits (`6ee9376f` / `3e8cd31f` / `e4e0e64c` / `67e395bd` /
`a6fee7a0` / `8837b64b`) lifted many slot wrappers / Expression
hosts into the tree at lowering — that groundwork is good (tree-
side, the right direction). The leak crept in only when
projections started peeling those wrappers back, and when the
walker grew per-variant marker rules. Step 1 (stripping the
renderer) removes the surface for either kind of leak.

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
