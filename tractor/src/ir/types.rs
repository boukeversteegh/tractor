//! IR variants.
//!
//! Each variant corresponds to a *semantic-tree* concept (the cross-language
//! shape declared in `specs/tractor-parse/semantic-tree/design.md`).
//! The IR is the schema by construction: if there's no variant, no
//! language can emit it.
//!
//! ## Source as the single source of truth
//! Every IR node carries a [`ByteRange`] over the original source.
//! Owned `text: String` fields are *not* stored on the IR — leaf text is
//! derived from `&source[range]` at render time. This guarantees:
//!
//! 1. **Round-trip identity.** `&source[ir.range]` is, by construction,
//!    the verbatim source slice that produced this IR. Recovering the
//!    full source text is `source[root_ir.range]`. Recovering any
//!    sub-tree's source is one slice operation.
//! 2. **XPath text-content matching.** The renderer weaves "gap text"
//!    (anonymous tokens like `(`, `)`, `.`, `,`, `;`, `=`, plus
//!    whitespace and comments) into the XML between source-derived
//!    children, so that `string(.)` on any rendered element equals
//!    `source[ir.range]`. This makes
//!    `//call[.='foobar()']` a valid query — match a node by its
//!    literal source text.
//!
//! Synthetic IR (added by shape decisions, not by source — e.g. an
//! `<access/>` marker, or a slot-wrapper element like `<left>`) has a
//! zero-width range *or* sits inside a parent variant whose renderer
//! puts it at a deterministic position. Synthetic IR contributes no
//! text, so it doesn't disturb XPath text-concatenation.
//!
//! ## Shape contracts as types
//! Several runtime shape rules in
//! `tractor/src/transform/shape_contracts.rs` exist to catch shape bugs
//! produced by imperative mutation. The IR makes most of them
//! *unrepresentable*:
//!
//! - **`marker-stays-empty`** (a name declared `MarkerOnly` must have
//!   no children). In the IR, marker-class variants (when added) carry
//!   no children fields; the rule becomes `cargo check`.
//! - **`container-has-content`** (a `ContainerOnly` name must have ≥1
//!   child). Container variants have required `Box<Ir>` / non-empty
//!   `Vec<Ir>` fields.
//! - **`no-marker-wrapper-collision`** (no parent has both `<X/>` empty
//!   and `<X>...</X>` wrapper sibling). Markers and containers are
//!   distinct variant *categories*; a single IR shape cannot produce
//!   both for the same name.
//! - **`name-declared-in-semantic-module`** (every emitted name is
//!   declared in the language's enum). The IR enum *is* the
//!   declaration.
//! - **`no-grammar-kind-suffix`** / **`node-name-lowercase`** /
//!   **`no-dash-in-node-name`**. Each variant has an explicit
//!   [`render`](super::render) mapping; raw tree-sitter kinds never
//!   leak.
//!
//! Other rules still need runtime checks because they depend on
//! cardinality decisions / source-text correlation:
//!
//! - **`no-children-overflow`** (≥2 untagged same-name siblings = JSON
//!   collision). Rendering decides cardinality from the IR; a fast
//!   structural check at render-time replaces the post-hoc walker.
//! - **`op-marker-matches-text`** — operator-text correlation; needs
//!   source.
//! - **`no-anonymous-keyword-leak`** — handled by lowering: tree-sitter
//!   anonymous nodes are explicitly mapped or dropped at lowering time,
//!   never rendered as text.
//! - **`no-repeated-parent-child-name`** — depends on IR shape; can be
//!   asserted at render-time.

/// Source-location span carried on every IR node.
///
/// Mirrors what the imperative builder threads through `xot.with_source_location_from`.
/// All four fields are 1-based to match tree-sitter / xot conventions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub line: u32,
    pub column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

impl Span {
    pub const fn point(line: u32, column: u32) -> Self {
        Self { line, column, end_line: line, end_column: column }
    }
}

/// Half-open byte range `[start, end)` into the original source string.
///
/// Used for two things: leaf-text extraction (`source[range]` is the
/// node's verbatim text) and gap-text computation between source-derived
/// children (text not covered by any child is the "gap" emitted in XML).
///
/// `Copy` so it threads cheaply; `u32` because no source we transform
/// approaches 4 GiB.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteRange {
    pub start: u32,
    pub end: u32,
}

impl ByteRange {
    pub const fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    /// Zero-width range at `at`. Used for synthetic IR (markers, slot
    /// wrappers) that has no source coverage but needs a `range` field.
    pub const fn empty_at(at: u32) -> Self {
        Self { start: at, end: at }
    }

    pub const fn len(&self) -> u32 {
        self.end - self.start
    }

    pub const fn is_empty(&self) -> bool {
        self.start >= self.end
    }

    /// Slice the source by this range. Caller asserts `source` is the
    /// same string the range was constructed from.
    pub fn slice<'a>(&self, source: &'a str) -> &'a str {
        &source[self.start as usize..self.end as usize]
    }
}

/// One IR node.
///
/// Variants are clustered by semantic role:
///
/// - **Containers** — top-level + statement scope.
/// - **Expression hosts** — Principle #15 stable expression positions.
/// - **Atoms** — leaf-level value carriers.
/// - **Escape hatches** — `Inline` / `Unknown`.
///
/// Initial slice is intentionally tiny. Variants are added as parity
/// scope grows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ir {
    // ----- Containers -----------------------------------------------------

    /// `<module>` — top-level program. The CST root for languages that
    /// have one (Python `module`, Java `program`, …). Children are
    /// statement-or-declaration IR.
    Module {
        children: Vec<Ir>,
        range: ByteRange,
        span: Span,
    },

    // ----- Expression hosts ----------------------------------------------

    /// `<expression>...</expression>` — Principle #15 stable expression
    /// host. Wraps a value-producing position so XPath queries can
    /// match on a uniform parent regardless of inner shape.
    Expression {
        inner: Box<Ir>,
        range: ByteRange,
        span: Span,
    },

    // ----- Access chains -------------------------------------------------

    /// `<object>` host for receiver-bearing access chains
    /// (member / index / call). Iter 345 renamed `subscript` to
    /// `index`; chain inversion produces the same `<object[access]>`
    /// shape for member and index access uniformly (Principle #5).
    ///
    /// `receiver` is the leftmost atom of the chain. `segments` are the
    /// access steps in source order. The renderer emits them
    /// right-nested (each segment contains the next) so that XPath
    /// text-concatenation on `<object>` returns the source slice
    /// verbatim — including the `.` / `[` / `]` punctuation that lives
    /// in the segments.
    Access {
        receiver: Box<Ir>,
        segments: Vec<AccessSegment>,
        range: ByteRange,
        span: Span,
    },

    /// `<binary>` operator expression `a op b`. Renders as
    /// `<binary><left><expression>{left}</expression></left>
    /// {gap}<op>{op_text}<{op_marker}/></op>{gap}
    /// <right><expression>{right}</expression></right></binary>`.
    /// The two `{gap}`s are whitespace between `left`/`op`/`right` in
    /// the source, derived from `op_range` and the operands' ranges.
    Binary {
        op_text: String,
        op_marker: &'static str,
        op_range: ByteRange,
        left: Box<Ir>,
        right: Box<Ir>,
        range: ByteRange,
        span: Span,
    },

    /// `<unary>` prefix-operator expression `op x`. The operand is
    /// rendered *unwrapped* (no `<expression>` host) to match the
    /// existing Python pipeline.
    Unary {
        op_text: String,
        op_marker: &'static str,
        op_range: ByteRange,
        operand: Box<Ir>,
        range: ByteRange,
        span: Span,
    },

    // ----- Assignments ----------------------------------------------------

    /// `<assign>` — `target = value` / `target: type = value` /
    /// `target += value` (augmented). Renders as
    /// `<assign><left>...</left>[<type>...</type>]<op>...</op><right>...</right></assign>`.
    ///
    /// `targets` are the LHS — wrapped in `<expression>` host(s)
    /// inside `<left>`. Multiple targets only when the source uses
    /// pattern_list / tuple_pattern (`a, b = ...`).
    /// `values` are the RHS — wrapped in `<expression>` host(s)
    /// inside `<right>`. Multiple values only for tuple right-hand
    /// sides matching the multi-target form.
    /// `type_annotation` is `Some` for annotated assignments
    /// (`x: int = …`).
    /// `op_markers` are emitted as empty children of `<op>`:
    /// `[]` for plain `=`, `["assign", "plus"]` for `+=`, etc.
    Assign {
        targets: Vec<Ir>,
        type_annotation: Option<Box<Ir>>,
        op_text: String,
        op_range: ByteRange,
        op_markers: Vec<&'static str>,
        values: Vec<Ir>,
        range: ByteRange,
        span: Span,
    },

    // ----- Imports --------------------------------------------------------

    /// `<import>` — top-level `import x` / `import x, y` / `import x as a`.
    /// `has_alias` adds an empty `<alias/>` marker child first; visible
    /// in the tree-text view as `<import[alias]>`.
    /// `children` are the import items in source order: each is an
    /// [`Ir::Path`] (plain), or an [`Ir::Path`] followed by an
    /// [`Ir::Aliased`] sibling (aliased — `import x as a`).
    Import {
        has_alias: bool,
        children: Vec<Ir>,
        range: ByteRange,
        span: Span,
    },

    /// `<from>` — `from x import y` (with `<relative/>` marker if the
    /// path is relative). `path` is `None` for bare `from . import x`.
    /// `imports` are one [`Ir::FromImport`] per imported name.
    From {
        relative: bool,
        path: Option<Box<Ir>>,
        imports: Vec<Ir>,
        range: ByteRange,
        span: Span,
    },

    /// `<import>` slot inside `<from>`. Holds the imported name (and
    /// alias if present) directly, *without* a `<path>` wrapper.
    /// `has_alias` adds an empty `<alias/>` marker child first.
    FromImport {
        has_alias: bool,
        /// Always an [`Ir::Name`] for the imported identifier.
        name: Box<Ir>,
        /// Some([`Ir::Aliased`]) if `... as X`.
        alias: Option<Box<Ir>>,
        range: ByteRange,
        span: Span,
    },

    /// `<path>` — dotted name, used in import / from-import paths and
    /// (later) other path positions. Segments are flat (Principle #19,
    /// iters 151-153).
    Path {
        segments: Vec<Ir>,
        range: ByteRange,
        span: Span,
    },

    /// `<aliased>` — the renamed-target side of `as` clauses
    /// (`import x as a`, `from m import y as z`). Wraps the alias
    /// `<name>` to disambiguate from the original name.
    Aliased {
        inner: Box<Ir>,
        range: ByteRange,
        span: Span,
    },

    /// `<call>` for a *standalone* call `f(args)` whose callee is a
    /// bare atom (typically `<name>`). When the callee is itself a
    /// chain (`a.b()`), lowering folds the call into an
    /// [`Ir::Access`] chain segment instead. (Future: add
    /// `AccessSegment::Call` and the chained-call lowering.)
    Call {
        callee: Box<Ir>,
        arguments: Vec<Ir>,
        range: ByteRange,
        span: Span,
    },

    // ----- Atoms ---------------------------------------------------------

    /// `<name>text</name>` — value-namespace identifier (variable,
    /// argument, function name when used as a value, etc.). Text is
    /// `source[range]` at render time.
    Name { range: ByteRange, span: Span },

    /// `<int>` / `<float>` / `<string>` / `<true>` / `<false>` /
    /// `<none>`. Renderer maps the variant to the element name and
    /// emits `source[range]` as the text leaf.
    ///
    /// One variant per literal *kind*; we deliberately do **not** model
    /// literals as one `Literal { kind, range }` because (a) some
    /// literals have substructure (concatenated strings, f-strings)
    /// that will need their own variants and (b) keeping each kind as
    /// its own variant lets Rust pattern-match exhaustively.
    Int    { range: ByteRange, span: Span },
    Float  { range: ByteRange, span: Span },
    String { range: ByteRange, span: Span },
    True   { range: ByteRange, span: Span },
    False  { range: ByteRange, span: Span },
    None   { range: ByteRange, span: Span },

    // ----- Escape hatches -------------------------------------------------

    /// "This CST kind has no semantic meaning at this level; render its
    /// children inline at the parent." Used for tree-sitter wrapper
    /// nodes, anonymous tokens we explicitly drop, etc.
    ///
    /// **Not** a stash variant. Lowering must *deliberately* choose
    /// `Inline` — the children list is the lowering's decision about
    /// what to keep.
    Inline {
        children: Vec<Ir>,
        range: ByteRange,
        span: Span,
    },

    /// Last-resort hatch for an un-handled CST kind. Renders as
    /// `<unknown kind="…">{source[range]}</unknown>`. Visible,
    /// queryable, and ratchet-able to zero per language as coverage
    /// fills in.
    Unknown {
        kind: String,
        range: ByteRange,
        span: Span,
    },
}

/// One step in an [`Ir::Access`] chain.
///
/// The renderer emits these *right-nested*: the first segment is a
/// child of `<object>`, the second is a child of the first, and so on.
/// `range` covers this segment's *own* source portion (e.g. `.b` for
/// the first segment of `a.b.c`); the renderer is responsible for
/// chaining the next segment inside this one and weaving any inter-
/// segment gap text.
///
/// `property_range` (for `Member`) is the byte range of the property
/// name itself, so that `<member><name>...</name></member>` emits the
/// name as a leaf and the dot as a gap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessSegment {
    /// `.property` — emits `<member>{gap}<name>property</name>...</member>`.
    Member {
        property_range: ByteRange,
        property_span: Span,
        range: ByteRange,
        span: Span,
    },

    /// `[indices...]` — emits `<index>{gap}{indices}{gap}...</index>`.
    /// Iter 345 renamed `subscript` → `index`; one IR variant covers
    /// both the chain-segment case (`a[0]`) and the future standalone
    /// case.
    Index {
        indices: Vec<Ir>,
        range: ByteRange,
        span: Span,
    },
}

impl AccessSegment {
    pub fn span(&self) -> Span {
        match self {
            AccessSegment::Member { span, .. } => *span,
            AccessSegment::Index { span, .. } => *span,
        }
    }

    pub fn range(&self) -> ByteRange {
        match self {
            AccessSegment::Member { range, .. } => *range,
            AccessSegment::Index { range, .. } => *range,
        }
    }
}

impl Ir {
    /// Source span of this node. Used for XML attribute emission.
    pub fn span(&self) -> Span {
        match self {
            Ir::Module { span, .. }
            | Ir::Expression { span, .. }
            | Ir::Access { span, .. }
            | Ir::Call { span, .. }
            | Ir::Binary { span, .. }
            | Ir::Unary { span, .. }
            | Ir::Assign { span, .. }
            | Ir::Import { span, .. }
            | Ir::From { span, .. }
            | Ir::FromImport { span, .. }
            | Ir::Path { span, .. }
            | Ir::Aliased { span, .. }
            | Ir::Name { span, .. }
            | Ir::Int { span, .. }
            | Ir::Float { span, .. }
            | Ir::String { span, .. }
            | Ir::True { span, .. }
            | Ir::False { span, .. }
            | Ir::None { span, .. }
            | Ir::Inline { span, .. }
            | Ir::Unknown { span, .. } => *span,
        }
    }

    /// Source byte range of this node. Used for verbatim-source
    /// recovery (`source[range]`) and for gap-text computation in the
    /// renderer.
    pub fn range(&self) -> ByteRange {
        match self {
            Ir::Module { range, .. }
            | Ir::Expression { range, .. }
            | Ir::Access { range, .. }
            | Ir::Call { range, .. }
            | Ir::Binary { range, .. }
            | Ir::Unary { range, .. }
            | Ir::Assign { range, .. }
            | Ir::Import { range, .. }
            | Ir::From { range, .. }
            | Ir::FromImport { range, .. }
            | Ir::Path { range, .. }
            | Ir::Aliased { range, .. }
            | Ir::Name { range, .. }
            | Ir::Int { range, .. }
            | Ir::Float { range, .. }
            | Ir::String { range, .. }
            | Ir::True { range, .. }
            | Ir::False { range, .. }
            | Ir::None { range, .. }
            | Ir::Inline { range, .. }
            | Ir::Unknown { range, .. } => *range,
        }
    }
}

/// Round-trip helper: recover the original source slice covered by
/// this IR node. Equivalent to `ir.range().slice(source)`.
///
/// **Round-trip identity:** `to_source(lower(parse(s)), s) == s` (the
/// root IR's range covers the whole source). For sub-trees,
/// `to_source(child, source)` is the verbatim source slice that
/// produced `child`.
pub fn to_source<'a>(ir: &Ir, source: &'a str) -> &'a str {
    ir.range().slice(source)
}
