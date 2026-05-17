//! tree variants.
//!
//! Each variant corresponds to a *semantic-tree* concept (the cross-language
//! shape declared in `specs/tractor-parse/semantic-tree/design.md`).
//! The tree is the schema by construction: if there's no variant, no
//! language can emit it.
//!
//! ## Source as the single source of truth
//! Every tree node carries a [`ByteRange`] over the original source.
//! Owned `text: String` fields are *not* stored on the tree — leaf text is
//! derived from `&source[range]` at render time. This guarantees:
//!
//! 1. **Round-trip identity.** `&source[tree.range]` is, by construction,
//!    the verbatim source slice that produced this tree. Recovering the
//!    full source text is `source[root_ir.range]`. Recovering any
//!    sub-tree's source is one slice operation.
//! 2. **XPath text-content matching.** The renderer weaves "gap text"
//!    (anonymous tokens like `(`, `)`, `.`, `,`, `;`, `=`, plus
//!    whitespace and comments) into the XML between source-derived
//!    children, so that `string(.)` on any rendered element equals
//!    `source[tree.range]`. This makes
//!    `//call[.='foobar()']` a valid query — match a node by its
//!    literal source text.
//!
//! Synthetic tree (added by shape decisions, not by source — e.g. an
//! `<access/>` marker, or a slot-wrapper element like `<left>`) has a
//! zero-width range *or* sits inside a parent variant whose renderer
//! puts it at a deterministic position. Synthetic tree contributes no
//! text, so it doesn't disturb XPath text-concatenation.
//!
//! ## Shape contracts as types
//! Several runtime shape rules in
//! `tractor/src/transform/shape_contracts.rs` exist to catch shape bugs
//! produced by imperative mutation. The tree makes most of them
//! *unrepresentable*:
//!
//! - **`marker-stays-empty`** (a name declared `MarkerOnly` must have
//!   no children). In the tree, marker-class variants (when added) carry
//!   no children fields; the rule becomes `cargo check`.
//! - **`container-has-content`** (a `ContainerOnly` name must have ≥1
//!   child). Container variants have required `Box<SyntaxTree>` / non-empty
//!   `Vec<SyntaxTree>` fields.
//! - **`no-marker-wrapper-collision`** (no parent has both `<X/>` empty
//!   and `<X>...</X>` wrapper sibling). Markers and containers are
//!   distinct variant *categories*; a single tree shape cannot produce
//!   both for the same name.
//! - **`name-declared-in-semantic-module`** (every emitted name is
//!   declared in the language's enum). The tree enum *is* the
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
//!   collision). Rendering decides cardinality from the tree; a fast
//!   structural check at render-time replaces the post-hoc walker.
//! - **`op-marker-matches-text`** — operator-text correlation; needs
//!   source.
//! - **`no-anonymous-keyword-leak`** — handled by lowering: tree-sitter
//!   anonymous nodes are explicitly mapped or dropped at lowering time,
//!   never rendered as text.
//! - **`no-repeated-parent-child-name`** — depends on tree shape; can be
//!   asserted at render-time.

/// Stable per-node identity within one in-memory tree session.
///
/// Used to bridge an XPath match against the XML projection back to
/// the typed-tree node that produced it (see `docs/design-editable-trees.md`).
/// Internal-only: never appears in user-facing XML / JSON output, never
/// becomes XPath syntax. `0` is the unassigned sentinel; real IDs start
/// at `1` and are handed out by [`assign_ids`](crate::tree::assign_ids).
pub type NodeId = u32;

/// Source-location span carried on every tree node.
///
/// Mirrors what the imperative builder threads through `xot.with_source_location_from`.
/// All four position fields are 1-based to match tree-sitter / xot
/// conventions. The `id` field is an internal stable identity (see
/// [`NodeId`]); `0` means "not yet assigned" and gets stamped to a
/// real value by the [`crate::tree::assign_ids`] post-construction
/// walk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub line: u32,
    pub column: u32,
    pub end_line: u32,
    pub end_column: u32,
    pub id: NodeId,
}

impl Span {
    pub const fn point(line: u32, column: u32) -> Self {
        Self {
            line,
            column,
            end_line: line,
            end_column: column,
            id: 0,
        }
    }

    /// Span with the same position as `self` but a fresh `id`. Used by
    /// the [`crate::tree::assign_ids`] walker to stamp IDs without
    /// rebuilding the surrounding node.
    pub const fn with_id(self, id: NodeId) -> Self {
        Self { id, ..self }
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
///
/// The `anchored` flag distinguishes ranges that point at a real
/// substring of the original source (parser output, even zero-width
/// fill-ins) from ranges on synthetic nodes built without source
/// (programmatic construction, tests, cross-language codegen). The
/// renderer's gap-fallback chain (S13-Z6) consults this flag to decide
/// whether to slice source for the gap or emit a canonical default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteRange {
    pub start: u32,
    pub end: u32,
    /// True when the range refers to an actual position in some source
    /// string. False for synthetic nodes constructed without source
    /// (tests, programmatic synthesis, cross-language codegen).
    pub anchored: bool,
}

/// Source-locatable boolean flag carried on a variant.
///
/// The tree represents shape-deciding attributes (`async`, `static`,
/// `public`, `inclusive`, `from`, …) as typed fields, not as marker
/// nodes. `Flag` is the carrier: `Off` means the attribute is unset;
/// `On { range, span }` means it's set and the source positions point
/// at the corresponding keyword (or are zero-width for implicit
/// defaults that have no keyword in the source).
///
/// The XML projection emits an empty `<flag_name/>` element for every
/// `On` flag, using the carried `span` for `line` / `column` /
/// `end_line` / `end_column` attributes. The JSON projection emits
/// `"flag_name": true`. The tree itself has no notion of "marker" —
/// that's purely an XML-side rendering choice.
///
/// **Anchored vs. implicit.** When the source has a literal keyword
/// (`async def f`), the flag carries the keyword's actual byte range
/// and span (`range.anchored = true`). When the flag is set
/// implicitly (e.g. Python's default-public access on a class member
/// with no access keyword), the range is zero-width and synthetic
/// (`ByteRange::synthetic_empty()`); the span is a `point` derived
/// from context, typically the parent declaration's start position.
/// Consumers that need to know which is which can check
/// `range.is_anchored()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Flag {
    /// Flag is unset. No XML element / JSON key emitted.
    #[default]
    Off,
    /// Flag is set, carrying source-position metadata.
    On { range: ByteRange, span: Span },
}

impl Flag {
    /// True iff the flag is `On`.
    pub const fn is_set(&self) -> bool {
        matches!(self, Self::On { .. })
    }

    /// Source span if set, `None` otherwise.
    pub const fn span(&self) -> Option<Span> {
        match self {
            Self::Off => None,
            Self::On { span, .. } => Some(*span),
        }
    }

    /// Source byte-range if set, `None` otherwise.
    pub const fn range(&self) -> Option<ByteRange> {
        match self {
            Self::Off => None,
            Self::On { range, .. } => Some(*range),
        }
    }

    /// Construct an `On` flag anchored to a source keyword token.
    /// Used by lowerings that locate the keyword's CST node.
    pub const fn anchored(range: ByteRange, span: Span) -> Self {
        Self::On { range, span }
    }

    /// Construct an `On` flag with no corresponding source keyword
    /// (implicit default). The span is a width-0 point.
    pub const fn implicit_at(line: u32, column: u32) -> Self {
        Self::On {
            range: ByteRange::synthetic_empty(),
            span: Span::point(line, column),
        }
    }

    /// Migration helper: behaves like a `bool` from a caller that
    /// hasn't yet been upgraded to provide positions. `true` produces
    /// a synthetic, position-less `On`; the renderer then synthesises
    /// a span from context (matches the pre-`Flag` behaviour).
    ///
    /// New code should prefer [`Flag::anchored`] (keyword from source)
    /// or [`Flag::implicit_at`] (implicit default at a known point).
    pub const fn from_bool(value: bool) -> Self {
        if value {
            Self::On {
                range: ByteRange::synthetic_empty(),
                span: Span::point(0, 0),
            }
        } else {
            Self::Off
        }
    }
}

impl From<bool> for Flag {
    fn from(value: bool) -> Self {
        Self::from_bool(value)
    }
}

/// A named flag with source-position metadata, used for the
/// per-variant marker set that doesn't fit into the shared
/// [`Modifiers`] struct. Examples: `<from/>` on a `yield`,
/// `<inclusive/>` on a Ruby range, `<group/>` on a TypeScript import.
///
/// Equivalent in semantics to a `Flag::On` plus a name: presence in
/// the variant's `extra_markers: Vec<Marker>` field means the flag is
/// set; the XML projection emits an empty `<{name}/>` element at the
/// carried `span`.
///
/// **Anchored vs implicit.** Same convention as [`Flag`]: when the
/// source has a keyword (e.g. `from` in `yield from x`), use
/// [`Marker::anchored`] with the keyword's byte range and span. When
/// the marker is implicit / synthesised (the variant lifts a marker
/// that doesn't appear in source), use [`Marker::implicit`] with a
/// width-0 synthetic position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Marker {
    pub name: &'static str,
    pub range: ByteRange,
    pub span: Span,
}

impl Marker {
    /// Construct a marker anchored to a source token (e.g. a
    /// keyword). XML emits `<name/>` with line/column matching the
    /// token's source position.
    pub const fn anchored(name: &'static str, range: ByteRange, span: Span) -> Self {
        Self { name, range, span }
    }

    /// Construct a marker without a corresponding source token
    /// (synthesised by the lowering). XML emits `<name/>` with a
    /// width-0 position. Callers that have a meaningful synthetic
    /// position should use [`Marker::implicit_at`] instead.
    pub const fn implicit(name: &'static str) -> Self {
        Self {
            name,
            range: ByteRange::synthetic_empty(),
            span: Span::point(0, 0),
        }
    }

    /// Construct an implicit marker at a specific source point.
    pub const fn implicit_at(name: &'static str, line: u32, column: u32) -> Self {
        Self {
            name,
            range: ByteRange::synthetic_empty(),
            span: Span::point(line, column),
        }
    }
}

impl From<&'static str> for Marker {
    fn from(name: &'static str) -> Self {
        Self::implicit(name)
    }
}

impl ByteRange {
    /// Range produced from real parser output. `anchored: true`.
    pub const fn new(start: u32, end: u32) -> Self {
        Self { start, end, anchored: true }
    }

    /// Zero-width range at `at`. Used by lowerings for parser-derived
    /// zero-width tree (missing optional bodies, fall-through positions);
    /// these are still anchored to a real source offset.
    pub const fn empty_at(at: u32) -> Self {
        Self { start: at, end: at, anchored: true }
    }

    /// Range on a synthetic node not derived from any source.
    /// `anchored: false`. The byte offsets are nominally valid but the
    /// renderer must not slice a source string with them.
    pub const fn synthetic(start: u32, end: u32) -> Self {
        Self { start, end, anchored: false }
    }

    /// Zero-width synthetic range (`0..0`, `anchored: false`). The
    /// default placeholder for tree nodes built without source.
    pub const fn synthetic_empty() -> Self {
        Self { start: 0, end: 0, anchored: false }
    }

    pub const fn len(&self) -> u32 {
        self.end - self.start
    }

    pub const fn is_empty(&self) -> bool {
        self.start >= self.end
    }

    /// True when this range refers to a real source position.
    pub const fn is_anchored(&self) -> bool {
        self.anchored
    }

    /// Slice the source by this range. Caller asserts `source` is the
    /// same string the range was constructed from, and that the range
    /// is anchored.
    pub fn slice<'a>(&self, source: &'a str) -> &'a str {
        &source[self.start as usize..self.end as usize]
    }
}

/// String-literal quote style preserved across the parse → render
/// round-trip. Languages use different lexical forms for the same
/// semantic string; the renderer reproduces the original form when
/// anchored and uses a per-language default when synthetic.
///
/// New variants extend per-language as needed (e.g. Python f-strings,
/// PHP heredocs/nowdocs). Keep the variant list small and well-named
/// rather than encoding raw lexical fragments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuoteStyle {
    /// No surrounding quotes — YAML plain scalars, INI bare values,
    /// Markdown text, T-SQL bare identifiers (`dbo.Users`).
    /// The renderer emits the stored text verbatim.
    Plain,
    /// `'...'` — single-quoted (Python, JS, Ruby, PHP, SQL, YAML).
    Single,
    /// `"..."` — double-quoted (most languages, JSON, TOML, ANSI SQL).
    Double,
    /// `'''...'''` — triple-single-quoted (Python, TOML multiline literal).
    TripleSingle,
    /// `"""..."""` — triple-double-quoted (Python, TOML multiline basic).
    TripleDouble,
    /// `` `...` `` — backtick (TypeScript template literal, Go raw,
    /// Ruby, MySQL identifier).
    Backtick,
    /// `[...]` — T-SQL bracketed identifier (`[Users]`). Allows
    /// reserved words and special characters in identifier names.
    Brackets,
    /// `r"..."`, `b"..."`, `R"..."`, etc. — prefixed raw / byte literal.
    /// `prefix` is the lowercase letter(s) before the opening quote.
    Raw { prefix: String, inner: Box<QuoteStyle> },
    /// `<<EOF ... EOF` — PHP / shell heredoc. `delimiter` is the
    /// terminator identifier.
    Heredoc { delimiter: String },
    /// `|` (literal) / `>` (folded) — YAML block scalar. `folded`
    /// distinguishes the two; `chomp` is `'-'` (strip), `'+'` (keep),
    /// or `' '` (clip / default).
    Block { folded: bool, chomp: char },
}

impl QuoteStyle {
    /// XML marker name to emit on the wrapping element so XPath
    /// queries can filter by quote style (e.g.
    /// `<identifier[bracketed]>` matches T-SQL `[Users]`). Returns
    /// `None` when the style has no marker.
    pub fn marker_name(&self) -> Option<&'static str> {
        match self {
            QuoteStyle::Plain => None,
            QuoteStyle::Single => Some("single-quoted"),
            QuoteStyle::Double => Some("quoted"),
            QuoteStyle::TripleSingle => Some("triple-single-quoted"),
            QuoteStyle::TripleDouble => Some("triple-double-quoted"),
            QuoteStyle::Backtick => Some("backticked"),
            QuoteStyle::Brackets => Some("bracketed"),
            QuoteStyle::Raw { .. } => Some("raw"),
            QuoteStyle::Heredoc { .. } => Some("heredoc"),
            QuoteStyle::Block { folded: false, .. } => Some("block-literal"),
            QuoteStyle::Block { folded: true, .. } => Some("block-folded"),
        }
    }
}

impl QuoteStyle {
    /// Conservative default for synthetic strings when no per-language
    /// preference is supplied. Languages may override in their Syntax
    /// config.
    pub const fn default_double() -> Self {
        QuoteStyle::Double
    }
}

/// One tree node.
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
pub enum SyntaxTree {
    // ----- Containers -----------------------------------------------------

    /// `<module>` — top-level program. The CST root for languages
    /// that have one. Children are statement-or-declaration tree.
    /// Per-language naming (Python "module" / C# "unit" / Java
    /// "program") is intentionally dropped in favor of a uniform
    /// `<module>` element across all code languages; the per-language
    /// lookup layer (C9) can re-introduce native names later.
    Module {
        children: Vec<SyntaxTree>,
        range: ByteRange,
        span: Span,
    },

    // ----- Expression hosts ----------------------------------------------

    /// `<expression>...</expression>` — Principle #15 stable expression
    /// host. Wraps a value-producing position so XPath queries can
    /// match on a uniform parent regardless of inner shape.
    ///
    /// `marker` adds an empty marker child first (rendered as
    /// `<expression[marker]>` in tree-text view). See
    /// [`ExpressionMarker`] for the closed set.
    ///
    /// **Why on the host, not the operand:** Principle #15 — markers
    /// live in stable predictable locations. The expression host is
    /// ALWAYS present in value positions; the marker decorates it
    /// rather than appearing on the bare inner name/expression.
    Expression {
        inner: Box<SyntaxTree>,
        marker: Option<ExpressionMarker>,
        range: ByteRange,
        span: Span,
    },

    /// Typed slot wrapper — value-position role-named container.
    /// `<left>` / `<right>` / `<condition>` / `<then>` / `<else>` /
    /// `<as>` / `<filter>` host elements produced by lowering's
    /// `wrap_slot` family. Each carries an `<expression>`-wrapped
    /// child (or an `Inline` group when the slot fans out across
    /// multiple values).
    ///
    /// Replaces the parity-track `SimpleStatement { element_name:
    /// "left" | "right" | ... }` form with a typed closed enum (no
    /// runtime string discriminator). Adding a new slot name is a
    /// `SlotKind` enum extension + a `wrap_slot` rename — no opt-in
    /// per language.
    ///
    /// @element_name = element_name_for_slot
    Slot {
        kind: SlotKind,
        children: Vec<SyntaxTree>,
        range: ByteRange,
        span: Span,
    },

    // ----- Access chains -------------------------------------------------

    /// `<object>` host for receiver-bearing access chains
    /// (member / index / call). Iter 345 renamed `subscript` to
    /// `index`; chain inversion produces the same `<object>` shape
    /// for member and index access uniformly (Principle #5).
    ///
    /// The variant is named `ObjectAccess` for semantic clarity in
    /// the typed tree (vs. the visibility-level `Access` enum and
    /// the receiver-kind `AccessReceiver` enum that live next to
    /// it); the `@element_name` override below renders it as plain
    /// `<object>` so XPath queries — and the existing renderer
    /// vocabulary — keep working against the conventional shape.
    ///
    /// `receiver` is the leftmost atom of the chain. `segments` are the
    /// access steps in source order. The renderer emits them
    /// right-nested (each segment contains the next) so that XPath
    /// text-concatenation on `<object>` returns the source slice
    /// verbatim — including the `.` / `[` / `]` punctuation that lives
    /// in the segments.
    ///
    /// @element_name = element_name_for_object_access
    ObjectAccess {
        receiver: AccessReceiver,
        segments: Vec<AccessSegment>,
        range: ByteRange,
        span: Span,
    },

    /// `<binary>` operator expression `a op b`. Renders as
    /// `<binary><left><expression>{left}</expression></left>
    /// {gap}<operator>{op_text}<{op_marker}/></operator>{gap}
    /// <right><expression>{right}</expression></right></binary>`.
    /// The two `{gap}`s are whitespace between `left`/`op`/`right`
    /// in the source.
    ///
    /// `<left>` / `<right>` come from the Rust field names via the
    /// field-projection walker — the operand is wrapped in
    /// `<expression>` at lowering, the outer wrapper is synthesised
    /// by the projection layer. The operator collapses to a single
    /// typed [`SyntaxTree::Operator`] sub-tree, which owns the
    /// source text + kind marker + range.
    /// @field_projection
    Binary {
        left: Box<SyntaxTree>,
        op: Box<SyntaxTree>,
        right: Box<SyntaxTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<logical>` short-circuit boolean expression (`a and b`, `a || b`).
    /// Same shape as [`SyntaxTree::Binary`]; sibling variant so the
    /// element name follows the variant tag mechanically.
    /// @field_projection
    Logical {
        left: Box<SyntaxTree>,
        op: Box<SyntaxTree>,
        right: Box<SyntaxTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<operator>` — typed sub-tree representing the operator token
    /// inside a `<binary>` / `<logical>` (later: `<comparison>` /
    /// `<unary>`) expression. Carries the source literal `text`
    /// (`"+"` / `"&&"` / ...), the closed-enum `kind` discriminator
    /// (single source of truth for the `<plus/>` / `<and/>` marker),
    /// and its own source range.
    ///
    /// Rendered as `<operator>{text}<{kind.marker_name}/></operator>`
    /// in XML (the marker child is emitted from `kind`), and as
    /// `{"$type": "operator", "text": "+", "<marker>": true}` in JSON
    /// via field projection.
    /// @field_projection
    Operator {
        text: String,
        kind: OperatorKind,
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
        operand: Box<SyntaxTree>,
        /// Extra markers placed on the `<unary>` element itself
        /// (NOT on `<op>`). Used for `<prefix/>` on `++x`/`--x` to
        /// distinguish from postfix forms.
        extra_markers: Vec<Marker>,
        range: ByteRange,
        span: Span,
    },

    // ----- Collections & literal containers -------------------------------

    /// `<tuple>` — `(a, b, c)` parenthesized tuple. Children are
    /// expressions in source order (no `<expression>` host —
    /// matches existing pipeline shape `<tuple><name>a</name>...</tuple>`).
    Tuple { children: Vec<SyntaxTree>, range: ByteRange, span: Span },

    /// `<list>` with `<literal/>` marker — `[a, b, c]` list literal.
    List { children: Vec<SyntaxTree>, range: ByteRange, span: Span },

    /// `<set>` with `<literal/>` marker — `{a, b}`.
    Set { children: Vec<SyntaxTree>, range: ByteRange, span: Span },

    /// `<dictionary>` with `<literal/>` marker — `{k: v, ...}`.
    Dictionary { pairs: Vec<SyntaxTree>, range: ByteRange, span: Span },

    /// `<pair>` — `key: value` inside a dictionary.
    Pair { key: Box<SyntaxTree>, value: Box<SyntaxTree>, range: ByteRange, span: Span },

    // ----- Generic types --------------------------------------------------

    /// `<type>` — `Name[T, U, ...]` generic type expression. The
    /// variant is named `GenericType` for typed-tree clarity (vs.
    /// the simpler `SimpleStatement { element_name: "type", ... }`
    /// shape that non-parameterized type references use), but renders
    /// as plain `<type>` so XPath queries against the type namespace
    /// match uniformly — `<type>List</type>` and `<type>List<int></type>`
    /// both surface via `//type`. `name` is the base type name;
    /// `params` are the type arguments.
    ///
    /// @element_name = element_name_for_generic_type
    GenericType {
        name: Box<SyntaxTree>,
        params: Vec<SyntaxTree>,
        range: ByteRange,
        span: Span,
    },

    // ----- Comparisons ----------------------------------------------------

    /// `<binary>` for chained comparisons like `a < b < c`. tree-sitter
    /// has a dedicated `comparison_operator` kind; we model it as a
    /// binary chain. For simplicity in the experiment, we emit a
    /// binary tree with the *first* operator and concatenate the
    /// remaining as Unknown-wrapped — this works for the common
    /// two-operand case (`a < b`).
    Comparison {
        left: Box<SyntaxTree>,
        op_text: String,
        op_marker: &'static str,
        op_range: ByteRange,
        right: Box<SyntaxTree>,
        range: ByteRange,
        span: Span,
    },

    // ----- Control flow ---------------------------------------------------

    /// `<if>` — `if cond: ... [elif ...] [else ...]`.
    /// @field_projection
    If {
        condition: Box<SyntaxTree>,
        body: Box<SyntaxTree>,             // SyntaxTree::Body
        else_branch: Option<Box<SyntaxTree>>, // SyntaxTree::ElseIf or SyntaxTree::Else
        range: ByteRange,
        span: Span,
    },

    /// `<else_if>` — `elif cond: body`. Used inside If's else_branch
    /// to keep elif chains flat.
    /// @field_projection
    ElseIf {
        condition: Box<SyntaxTree>,
        body: Box<SyntaxTree>,
        else_branch: Option<Box<SyntaxTree>>,
        range: ByteRange,
        span: Span,
    },

    /// `<else>` — `else: body`.
    /// @field_projection
    Else { body: Box<SyntaxTree>, range: ByteRange, span: Span },

    /// `<for>` — `for target in iter: body [else: body]`.
    /// `<for[async]>` adds an `<async/>` marker.
    /// @field_projection
    For {
        is_async: bool,
        targets: Vec<SyntaxTree>,
        iterables: Vec<SyntaxTree>,
        body: Box<SyntaxTree>,
        else_body: Option<Box<SyntaxTree>>,
        range: ByteRange,
        span: Span,
    },

    /// `<while>` — `while cond: body [else: body]`.
    /// @field_projection
    While {
        condition: Box<SyntaxTree>,
        body: Box<SyntaxTree>,
        else_body: Option<Box<SyntaxTree>>,
        range: ByteRange,
        span: Span,
    },

    /// `<foreach>` — C# `foreach (T x in coll) body` / Java
    /// enhanced-for. Single target, single iterable, optional type
    /// annotation. Distinct from [`SyntaxTree::For`] because Python's
    /// `for x in iter` (a foreach by semantics) renders as `<for>`
    /// for parity with the existing pipeline; cross-language element
    /// naming asymmetry is allowed (Principle #5 scope is intra-
    /// language).
    /// @field_projection
    Foreach {
        type_ann: Option<Box<SyntaxTree>>,
        target: Box<SyntaxTree>,
        iterable: Box<SyntaxTree>,
        body: Box<SyntaxTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<for>` — C-style `for (init; cond; update) body` (C#, Java,
    /// JS, …). All three header parts are optional. `updates` is a
    /// vec because C-style `for` allows comma-separated updates
    /// (`for(int i=0,j=10; i<j; i++,j--)`).
    /// @field_projection
    CFor {
        initializer: Option<Box<SyntaxTree>>,
        condition: Option<Box<SyntaxTree>>,
        updates: Vec<SyntaxTree>,
        body: Box<SyntaxTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<do>` — `do body while(cond);`. Renders the keyword as gap
    /// text; body and condition are the only tree children.
    /// @field_projection
    DoWhile {
        body: Box<SyntaxTree>,
        condition: Box<SyntaxTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<break>` / `<continue>` markers.
    Break { range: ByteRange, span: Span },
    Continue { range: ByteRange, span: Span },

    /// Wrap an inner tree node in a single element. Used as the
    /// parity-track field-wrapping mechanism: when a CST child has
    /// a labelled `field=type` (or `name`, `value`, etc.) and that
    /// field has a wrapping in the language's table, lower it as
    /// `SyntaxTree::FieldWrap { wrapper: "type", inner: ... }` so the
    /// rendered XML is `<type>{inner rendering}</type>`.
    ///
    /// @element_name = element_name_for_field_wrap
    FieldWrap {
        wrapper: &'static str,
        inner: Box<SyntaxTree>,
        range: ByteRange,
        span: Span,
    },

    /// Generic single-keyword statement carrier. Renders as
    /// `<{element_name}>{markers from modifiers}{children with gaps}</{element_name}>`.
    /// Used as the parity-track variant for kinds whose old-pipeline
    /// rule is a simple Rename: `assert`, `raise`, `delete`, `global`,
    /// `nonlocal`, `yield`, etc. Children are the named CST children
    /// in source order; the leading keyword and any punctuation lives
    /// in gap text. `modifiers` lets parity-track declarations
    /// (delegate, event, indexer, etc.) carry their access + flag
    /// markers without designing a dedicated typed variant first.
    ///
    /// Eventually most users of this should be promoted to typed
    /// variants with proper field labels — but for parity-first
    /// rollout, this gets the element name right without designing
    /// each one upfront.
    ///
    /// @element_name = element_name_for_simple_statement
    SimpleStatement {
        element_name: &'static str,
        modifiers: Modifiers,
        /// Extra static markers to emit before children, in order. Used
        /// for pattern combinators (`<and/>`, `<or/>`), keyword markers
        /// (`<stackalloc/>`, `<ref/>`, `<var/>`) etc. — markers that the
        /// imperative pipeline attaches as siblings of anonymous-keyword
        /// text (Principle: every keyword in an element's text must
        /// have a corresponding marker sibling).
        extra_markers: Vec<Marker>,
        children: Vec<SyntaxTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<try>` — `try { body } catch (...) { ... } finally { ... }`
    /// (C# / Java) or `try: ... except E: ... else: ... finally: ...`
    /// (Python). Shared cross-language. `try_body` is the protected
    /// block; `handlers` are catch/except clauses; `else_body` runs
    /// when no exception (Python only); `finally_body` always runs.
    /// @field_projection
    Try {
        try_body: Box<SyntaxTree>,
        handlers: Vec<SyntaxTree>,
        else_body: Option<Box<SyntaxTree>>,
        finally_body: Option<Box<SyntaxTree>>,
        range: ByteRange,
        span: Span,
    },

    /// `<except>` (Python) / `<catch>` (C#) — single exception handler.
    /// `type_target` is the exception type; `binding` is the variable
    /// (`as e` / `Exception ex`); `filter` is C#'s `when (cond)`;
    /// `body` is the handler block.
    /// `<except>` (Python) — single Python-style exception handler.
    /// @field_projection
    Except {
        type_target: Option<Box<SyntaxTree>>,
        binding: Option<Box<SyntaxTree>>,
        filter: Option<Box<SyntaxTree>>,
        body: Box<SyntaxTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<catch>` (C# / Java / JS) — single catch clause. Same shape
    /// as [`SyntaxTree::Except`]; sibling variant.
    /// @field_projection
    Catch {
        type_target: Option<Box<SyntaxTree>>,
        binding: Option<Box<SyntaxTree>>,
        filter: Option<Box<SyntaxTree>>,
        body: Box<SyntaxTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<alias>` — Python 3.12 `type Foo = Bar` /
    /// `type Foo[T] = Bar`. `name` is the alias being declared,
    /// `type_params` is the optional generic list, `value` is the
    /// aliased type.
    TypeAlias {
        name: Box<SyntaxTree>,
        type_params: Option<Box<SyntaxTree>>,
        value: Box<SyntaxTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<keyword>` — `name=value` keyword argument in a call (Python /
    /// C# named arg). `value` is the inner expression.
    KeywordArgument {
        name: Box<SyntaxTree>,
        value: Box<SyntaxTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<splat>` with `<list/>` marker — `*x` (positional splat) in a
    /// call or list literal. Inner is the splatted expression.
    ListSplat {
        inner: Box<SyntaxTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<splat>` with `<dict/>` marker — `**x` (keyword splat) in a
    /// call or dict literal.
    DictSplat {
        inner: Box<SyntaxTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<ternary>` — `cond ? a : b` (C# / Java / JS) or
    /// `a if cond else b` (Python). Renders with logical slots
    /// regardless of source order; the renderer sorts children by
    /// `range().start` to weave gap text correctly.
    /// @field_projection
    Ternary {
        condition: Box<SyntaxTree>,
        if_true: Box<SyntaxTree>,
        if_false: Box<SyntaxTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<new>` — `new Foo(args) { Init }` (C# / Java
     /// `new`-expression). `type_target` is `None` for C#'s
    /// target-typed `new()` form. `initializer` carries an
    /// `SyntaxTree::Inline` of the brace-form initializer's children
    /// (`{ A = 1, B = 2 }`) when present.
    ObjectCreation {
        type_target: Option<Box<SyntaxTree>>,
        arguments: Vec<SyntaxTree>,
        initializer: Option<Box<SyntaxTree>>,
        range: ByteRange,
        span: Span,
    },

    /// `<lambda>` — `x => x*x`, `(x, y) => x+y`, `async x => ...`,
    /// `(x) => { return x; }`. Cross-language: C# lambda, Java
    /// lambda (`x -> x`), Python `lambda` (which has bare-param
    /// syntax). `body` is a typed [`LambdaBody`] enum so the renderer
    /// dispatches on the form (block vs expression) without inspecting
    /// the inner tree's variant.
    /// @field_projection
    Lambda {
        modifiers: Modifiers,
        parameters: Vec<SyntaxTree>,
        body: LambdaBody,
        range: ByteRange,
        span: Span,
    },

    // ----- Function & class declarations ----------------------------------

    /// `<function>` — `def f(...)` / `async def f(...)`. Decorators
    /// are children at the top (renders before `<name>`); generics
    /// after name; parameters after generics; `<returns>` for return
    /// type; `<body>` last.
    ///
    /// `modifiers` carries `async`, `static`, `virtual`, `override`,
    /// `abstract`, etc. as exhaustive flags. Python sets only
    /// `async_`; C# sets many more.
    /// @field_projection
    Function {
        modifiers: Modifiers,
        decorators: Vec<SyntaxTree>,
        name: Box<SyntaxTree>,
        generics: Vec<SyntaxTree>,
        parameters: Vec<SyntaxTree>,
        returns: Option<Box<SyntaxTree>>,
        throws: Vec<SyntaxTree>,
        body: Option<Box<SyntaxTree>>,
        range: ByteRange,
        span: Span,
    },

    /// `<method>` — instance / static method on a class. Same shape
    /// as [`SyntaxTree::Function`]; sibling variant.
    /// @field_projection
    Method {
        modifiers: Modifiers,
        decorators: Vec<SyntaxTree>,
        name: Box<SyntaxTree>,
        generics: Vec<SyntaxTree>,
        parameters: Vec<SyntaxTree>,
        returns: Option<Box<SyntaxTree>>,
        throws: Vec<SyntaxTree>,
        body: Option<Box<SyntaxTree>>,
        range: ByteRange,
        span: Span,
    },

    /// `<class>` / `<struct>` / `<interface>` / `<record>` — type
    /// declaration. `kind` selects the element name; structurally all
    /// four shapes are the same (modifiers, decorators, name,
    /// generics, bases, body), so they share one tree variant. Python
    /// always sets `kind = "class"`; C# picks per CST kind.
    ///
    /// `modifiers` carries access + flags. Empty for languages
    /// without modifier concepts (Python class definitions). The
    /// renderer emits one zero-width marker per active flag.
    /// Flipping any flag swaps the corresponding marker by
    /// construction.
    /// @field_projection
    Class {
        modifiers: Modifiers,
        decorators: Vec<SyntaxTree>,
        name: Box<SyntaxTree>,
        /// Generic type parameters (each is a [`SyntaxTree::TypeParameter`]).
        generics: Vec<SyntaxTree>,
        bases: Vec<SyntaxTree>,                 // each is a base expression
        where_clauses: Vec<SyntaxTree>,         // C# `where T : ...` constraints (other languages: empty)
        body: Box<SyntaxTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<struct>` — C# struct declaration. Same shape as `Class`;
    /// distinct variant so the element name follows the variant tag
    /// mechanically (no string discriminator).
    /// @field_projection
    Struct {
        modifiers: Modifiers,
        decorators: Vec<SyntaxTree>,
        name: Box<SyntaxTree>,
        generics: Vec<SyntaxTree>,
        bases: Vec<SyntaxTree>,
        where_clauses: Vec<SyntaxTree>,
        body: Box<SyntaxTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<interface>` — C# / Java / TypeScript interface. Same shape
    /// as `Class`; sibling variant.
    /// @field_projection
    Interface {
        modifiers: Modifiers,
        decorators: Vec<SyntaxTree>,
        name: Box<SyntaxTree>,
        generics: Vec<SyntaxTree>,
        bases: Vec<SyntaxTree>,
        where_clauses: Vec<SyntaxTree>,
        body: Box<SyntaxTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<record>` — C# / Java record declaration. Same shape as
    /// `Class`; sibling variant.
    /// @field_projection
    Record {
        modifiers: Modifiers,
        decorators: Vec<SyntaxTree>,
        name: Box<SyntaxTree>,
        generics: Vec<SyntaxTree>,
        bases: Vec<SyntaxTree>,
        where_clauses: Vec<SyntaxTree>,
        body: Box<SyntaxTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<body>` — a block of statements. `pass_only` adds a `<pass/>`
    /// empty marker child; visible in tree-text as `<body[pass]>`.
    /// `block_wrap` adds an inner `<block>` element so the rendered
    /// shape is `<body><block>{stmts}</block></body>` — matches C#'s
    /// imperative pipeline (where method `field="body"` field-wraps a
    /// `block` kind to produce body/block nesting). Python sets it
    /// false (its function bodies are flat under `<body>`).
    Body {
        children: Vec<SyntaxTree>,
        pass_only: bool,
        block_wrap: bool,
        range: ByteRange,
        span: Span,
    },

    /// `<parameter>` — one parameter in a function signature.
    /// `kind` controls the marker: `Regular` has none,
    /// `Args` adds `<args/>`, `Kwargs` adds `<kwargs/>`.
    /// `extra_markers` carries C#-style parameter modifiers
    /// (`<ref/>`, `<out/>`, `<in/>`, `<params/>`, `<this/>`).
    /// `modifiers` carries TS constructor-parameter access markers
    /// (`<public/>`, `<private/>`, `<readonly/>`, `<override/>`).
    /// @field_projection
    Parameter {
        kind: ParamKind,
        extra_markers: Vec<Marker>,
        modifiers: Modifiers,
        name: Box<SyntaxTree>,                  // SyntaxTree::Name
        type_ann: Option<Box<SyntaxTree>>,      // <type>...</type>
        default: Option<Box<SyntaxTree>>,       // <value><expression>...</expression></value>
        range: ByteRange,
        span: Span,
    },

    /// Source-range consumer that emits nothing in the rendered tree.
    /// Used by per-language lowerings to "swallow" anonymous keyword
    /// bytes (e.g. T-SQL `SELECT` / `FROM` / `WHERE`) without leaking
    /// them as gap text under the parent element. The parent's
    /// `render_with_gaps` advances its cursor past `range.end`, so
    /// the gap before the next sibling skips the keyword bytes.
    Skip { range: ByteRange, span: Span },

    /// `<positional>/</positional>` — `/` separator marking the end
    /// of positional-only parameters.
    PositionalSeparator { range: ByteRange, span: Span },

    /// `<keyword>*</keyword>` — `*` separator marking the start of
    /// keyword-only parameters.
    KeywordSeparator { range: ByteRange, span: Span },

    /// `<decorator>` — `@expr` decorator above a function/class.
    /// Wraps any expression directly (no `<expression>` host).
    Decorator {
        inner: Box<SyntaxTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<returns>` — return-type annotation slot. Wraps a `<type>`.
    /// @field_projection
    Returns {
        type_ann: Box<SyntaxTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<generic>` — generic-parameter list (PEP 695 `def f[T]`).
    /// Each item is an [`SyntaxTree::TypeParameter`] (renders as `<type>`
    /// containing a `<name>`).
    Generic {
        items: Vec<SyntaxTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<type>` — type-parameter slot inside `<generic>`. Has a name
    /// and optional constraint.
    ///
    /// Renamed for typed-tree clarity (distinct from non-parameterized
    /// type-reference shapes that use `SimpleStatement {
    /// element_name: "type" }`); the override below renders as the
    /// same `<type>` element so XPath queries against the type
    /// namespace work uniformly.
    ///
    /// @element_name = element_name_for_type_parameter
    TypeParameter {
        name: Box<SyntaxTree>,
        constraint: Option<Box<SyntaxTree>>,
        range: ByteRange,
        span: Span,
    },

    /// `<return>` — `return <value>?` statement. `value` is `None`
    /// for bare `return`. Renders as
    /// `<return><expression>...</expression></return>` when value is
    /// present.
    Return {
        value: Option<Box<SyntaxTree>>,
        range: ByteRange,
        span: Span,
    },

    /// `<comment>text</comment>` — standalone source comment.
    /// `leading` adds a `<leading/>` marker (`comment[leading]`); the
    /// existing pipeline classifies comments by adjacency to the next
    /// declaration. `trailing` adds a `<trailing/>` marker for comments
    /// on the same line as a preceding code construct
    /// (`int x; // here`). At most one of leading/trailing is true; a
    /// comment with neither is "floating".
    Comment {
        leading: Flag,
        trailing: Flag,
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
        targets: Vec<SyntaxTree>,
        type_annotation: Option<Box<SyntaxTree>>,
        op_text: String,
        op_range: ByteRange,
        op_markers: Vec<&'static str>,
        values: Vec<SyntaxTree>,
        range: ByteRange,
        span: Span,
    },

    // ----- Imports --------------------------------------------------------

    /// `<import>` — top-level `import x` / `import x, y` / `import x as a`.
    /// `has_alias` adds an empty `<alias/>` marker child first; visible
    /// in the tree-text view as `<import[alias]>`.
    /// `children` are the import items in source order: each is an
    /// [`SyntaxTree::Path`] (plain), or an [`SyntaxTree::Path`] followed by an
    /// [`SyntaxTree::Aliased`] sibling (aliased — `import x as a`).
    Import {
        has_alias: bool,
        children: Vec<SyntaxTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<from>` — `from x import y` (with `<relative/>` marker if the
    /// path is relative). `path` is `None` for bare `from . import x`.
    /// `imports` are one [`SyntaxTree::FromImport`] per imported name.
    From {
        relative: bool,
        path: Option<Box<SyntaxTree>>,
        imports: Vec<SyntaxTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<import>` slot inside `<from>`. Holds the imported name (and
    /// alias if present) directly, *without* a `<path>` wrapper.
    /// `has_alias` adds an empty `<alias/>` marker child first.
    FromImport {
        has_alias: bool,
        /// Always an [`SyntaxTree::Name`] for the imported identifier.
        name: Box<SyntaxTree>,
        /// Some([`SyntaxTree::Aliased`]) if `... as X`.
        alias: Option<Box<SyntaxTree>>,
        range: ByteRange,
        span: Span,
    },

    /// `<path>` — dotted name, used in import / from-import paths and
    /// (later) other path positions. Segments are flat (Principle #19,
    /// iters 151-153).
    Path {
        segments: Vec<SyntaxTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<aliased>` — the renamed-target side of `as` clauses
    /// (`import x as a`, `from m import y as z`). Wraps the alias
    /// `<name>` to disambiguate from the original name.
    Aliased {
        inner: Box<SyntaxTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<call>` for a *standalone* call `f(args)` whose callee is a
    /// bare atom (typically `<name>`). When the callee is itself a
    /// chain (`a.b()`), lowering folds the call into an
    /// [`SyntaxTree::ObjectAccess`] chain segment instead. (Future: add
    /// `AccessSegment::Call` and the chained-call lowering.)
    Call {
        callee: Box<SyntaxTree>,
        arguments: Vec<SyntaxTree>,
        range: ByteRange,
        span: Span,
    },

    // ----- Atoms ---------------------------------------------------------

    /// `<name>text</name>` — value-namespace identifier (variable,
    /// argument, function name when used as a value, etc.). `text`
    /// carries the identifier verbatim so the renderer can emit it
    /// even for synthetic nodes with no source anchor (S13-Z1).
    Name { text: String, range: ByteRange, span: Span },

    /// `<int>` / `<float>` / `<string>` / `<true>` / `<false>` /
    /// `<none>`. Renderer maps the variant to the element name and
    /// emits the stored `text` as the leaf.
    ///
    /// `text` carries the *decoded* lexical form (numeric literal as
    /// written; for `String`, the semantic content with escape
    /// sequences already decoded — see [`QuoteStyle`] for the surface
    /// form). One variant per literal *kind*; we deliberately do **not**
    /// model literals as one `Literal { kind, range }` because (a) some
    /// literals have substructure (concatenated strings, f-strings)
    /// that will need their own variants and (b) keeping each kind as
    /// its own variant lets Rust pattern-match exhaustively.
    Int    { text: String, range: ByteRange, span: Span },
    Float  { text: String, range: ByteRange, span: Span },
    /// `<string>` — string literal. `text` is the *decoded* content
    /// (e.g. for the source `"hello\n"` the text is `hello\n` with a
    /// real newline byte); the renderer re-encodes using `quote_style`
    /// and the language's escape table.
    String {
        text: String,
        quote_style: QuoteStyle,
        range: ByteRange,
        span: Span,
    },
    True   { text: String, range: ByteRange, span: Span },
    False  { text: String, range: ByteRange, span: Span },
    None   { text: String, range: ByteRange, span: Span },

    /// `<{element_name}>text</{element_name}>` — generic per-language
    /// classified atom. Used when a language emits the same source-
    /// text leaf under different XML element names depending on its
    /// CST role: e.g. T-SQL classifies `identifier` text as `<var>`
    /// when it starts with `@`, `<schema>` for the qualifier in
    /// `dbo.Users`, `<alias>` for trailing `AS`-position identifiers,
    /// and `<name>` otherwise. `text` carries the verbatim source so
    /// the renderer can emit it without a source anchor (S13-Z1).
    ///
    /// @element_name = element_name_for_atom
    Atom {
        element_name: &'static str,
        text: String,
        range: ByteRange,
        span: Span,
    },
    /// `<enum>` — `enum Name { Member1, Member2 = 5, ... }`. Members
    /// are `SyntaxTree::EnumMember`. C# enums also accept an optional
    /// underlying type (`enum Trait : uint`).
    Enum {
        modifiers: Modifiers,
        decorators: Vec<SyntaxTree>,
        name: Box<SyntaxTree>,
        underlying_type: Option<Box<SyntaxTree>>,  // C# `: uint`
        members: Vec<SyntaxTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<constant>` — one member of an enum (`Low`, `Medium = 5`).
    EnumMember {
        decorators: Vec<SyntaxTree>,
        name: Box<SyntaxTree>,
        value: Option<Box<SyntaxTree>>,
        range: ByteRange,
        span: Span,
    },

    /// `<property>` — C# `public int X { get; set; } = init;` and
    /// the various property forms. Accessors: getter / setter /
    /// init. Renders
    /// `<property>{markers}<type>...<name>...{accessors}{value}</property>`.
    Property {
        modifiers: Modifiers,
        decorators: Vec<SyntaxTree>,
        type_ann: Option<Box<SyntaxTree>>,
        name: Box<SyntaxTree>,
        accessors: Vec<SyntaxTree>,                // each SyntaxTree::Accessor
        value: Option<Box<SyntaxTree>>,            // initializer expression
        range: ByteRange,
        span: Span,
    },

    /// `<get>` / `<set>` / `<init>` — property accessor. Single
    /// variant because the natural Rust split names (`Get` / `Set` /
    /// `Init`) would collide with the collection `Set` variant. The
    /// discriminator `kind: AccessorKind` is a typed closed enum.
    ///
    /// @element_name = element_name_for_accessor
    Accessor {
        modifiers: Modifiers,
        kind: AccessorKind,
        body: Option<Box<SyntaxTree>>,
        range: ByteRange,
        span: Span,
    },

    /// `<constructor>` — C# constructor (`public Foo(...) : base(x) { }`).
    /// Renders similar to method but with `<constructor>` element.
    /// Initializer `: base(...)` deferred.
    Constructor {
        modifiers: Modifiers,
        decorators: Vec<SyntaxTree>,
        name: Box<SyntaxTree>,                     // class name being constructed
        parameters: Vec<SyntaxTree>,
        body: Box<SyntaxTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<using>` — C#'s `using System;` / `using static System.Math;`
    /// / `using A = B;`. The tree mirrors Python's import shape but
    /// with `<using>` element name. `static_` flag for `using static`,
    /// `alias` for `using X = Y;`.
    Using {
        is_static: bool,
        alias: Option<Box<SyntaxTree>>,
        path: Box<SyntaxTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<namespace>` — C#'s `namespace X { ... }` (block-scoped) or
    /// `namespace X;` (file-scoped). `file_scoped` adds a `<file/>`
    /// marker that the C# post_transform's
    /// `unify_file_scoped_namespace` looks for to fold following
    /// siblings into the namespace's body.
    Namespace {
        name: Box<SyntaxTree>,
        children: Vec<SyntaxTree>,
        file_scoped: bool,
        range: ByteRange,
        span: Span,
    },

    /// `<variable>` — local variable declaration `var x = value;` /
    /// `int x;`. Renders
    /// `<variable>[<type>...</type>]<name>...</name>[value-expr]</variable>`.
    /// `extra_markers` carries language-specific declaration markers
    /// like `<let/>` / `<const/>` / `<var/>` (TS) — not modifiers in
    /// the visibility/static sense, but keyword discriminators that
    /// the lowering surfaces as flag markers on the element.
    /// @field_projection
    Variable {
        modifiers: Modifiers,
        decorators: Vec<SyntaxTree>,
        extra_markers: Vec<Marker>,
        type_ann: Option<Box<SyntaxTree>>,
        name: Box<SyntaxTree>,
        value: Option<Expression>,
        range: ByteRange,
        span: Span,
    },

    /// `<field>` — class-level field declaration. Same shape as
    /// [`SyntaxTree::Variable`]; sibling variant for the distinct
    /// semantic concept (member scope, can carry full modifiers).
    /// @field_projection
    Field {
        modifiers: Modifiers,
        decorators: Vec<SyntaxTree>,
        type_ann: Option<Box<SyntaxTree>>,
        name: Box<SyntaxTree>,
        value: Option<Expression>,
        range: ByteRange,
        span: Span,
    },

    /// `<event>` — C# event declaration. Same shape as
    /// [`SyntaxTree::Field`]; sibling variant for the distinct C#
    /// `event` member concept.
    /// @field_projection
    Event {
        modifiers: Modifiers,
        decorators: Vec<SyntaxTree>,
        type_ann: Option<Box<SyntaxTree>>,
        name: Box<SyntaxTree>,
        value: Option<Expression>,
        range: ByteRange,
        span: Span,
    },

    /// `<is>` — `expr is Type` type-test expression. Renders as
    /// `<is><left><expression>{value}</expression></left>
    /// <right><expression><type>{type_target}</type></expression></right></is>`.
    /// (Pattern-form `is Widget w` not yet covered — would extend
    /// `right` with a pattern variant.)
    Is {
        value: Box<SyntaxTree>,
        type_target: Box<SyntaxTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<cast>` — `(Type)expr` type-cast expression (C#, Java, …).
    /// Renders as `<cast><type>...</type><value><expression>...</expression></value></cast>`.
    Cast {
        type_ann: Box<SyntaxTree>,
        value: Box<SyntaxTree>,
        range: ByteRange,
        span: Span,
    },

    /// `null` literal (C# / Java / TS / PHP). Distinct from `None`
    /// (Python) because the keyword text differs and Principle #5
    /// applies *within* a language. We may unify the *element name*
    /// at render time later if a cross-language audit decides so.
    Null   { text: String, range: ByteRange, span: Span },

    // ----- Escape hatches -------------------------------------------------

    /// "This CST kind has no semantic meaning at this level; render its
    /// children inline at the parent." Used for tree-sitter wrapper
    /// nodes, anonymous tokens we explicitly drop, etc.
    ///
    /// **Not** a stash variant. Lowering must *deliberately* choose
    /// `Inline` — the children list is the lowering's decision about
    /// what to keep.
    ///
    /// `list_name` mirrors the imperative pipeline's
    /// `Flatten { distribute_list: Some("X") }` rule — when present,
    /// each rendered child element gets a `list="X"` attribute, so
    /// JSON projection collects them under a plural key (e.g.
    /// `attributes: [...]`) instead of colliding on a singleton.
    Inline {
        children: Vec<SyntaxTree>,
        list_name: Option<&'static str>,
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

    /// Typed passthrough — the bare CST kind hierarchy with no
    /// field-wrapping or per-language shape decisions.
    ///
    /// Two roles, distinguished by `is_named`:
    /// - **Named (`is_named: true`)**: a grammar node that becomes an
    ///   `<{kind}>{children…}</{kind}>` element. Used by
    ///   [`lower_raw_passthrough`](crate::tree::lower_raw_passthrough)
    ///   for `TreeKind::Syntax` passthrough languages, and by
    ///   [`lower_raw_passthrough_all`](crate::tree::lower_raw_passthrough_all)
    ///   for the named structural nodes inside `TreeMode::Raw`.
    /// - **Anonymous (`is_named: false`)**: a tree-sitter token
    ///   (punctuation, keyword, operator). Renders as bare source
    ///   text — no enclosing element. Only produced by
    ///   `lower_raw_passthrough_all` since named-only passthrough
    ///   filters anonymous tokens out.
    ///
    /// Distinct from [`SyntaxTree::Unknown`]: `Unknown` is the
    /// per-node escape hatch *inside* a partly-typed language ("this
    /// kind isn't yet covered"). `Raw` is the whole-language stance
    /// ("this language is intentionally passthrough"). They share
    /// most plumbing but diverge in semantics — coverage audits
    /// should treat `Unknown` as a debt to pay down and `Raw` as the
    /// chosen shape.
    ///
    /// Lowering ensures `Raw` is only constructed for *named*
    /// tree-sitter nodes; anonymous tokens flow through as
    /// [`SyntaxTree::Inline`] (no wrapper element). That keeps the
    /// element-name override below total — it always has a kind to
    /// return.
    ///
    /// @element_name = element_name_for_raw
    Raw {
        kind: String,
        is_named: bool,
        children: Vec<SyntaxTree>,
        range: ByteRange,
        span: Span,
    },
}

/// Access modifier for class / method / field declarations. The
/// **exhaustive variation** principle: every C# / Java / Kotlin class
/// has *exactly one* access level (no overlap, no absence — defaulted
/// when the source omits it). Encoding this as an enum gives us:
///
/// 1. **Compile-time exhaustiveness.** Adding a new variant forces
///    the renderer + lowering to acknowledge it.
/// 2. **Stable mutation surface.** `access = Access::Private` is a
///    typed operation; re-rendering picks the right marker.
/// 3. **Marker swap is automatic.** `<public/>` becomes `<private/>`
///    by changing one enum value, not by hand-editing XML.
///
/// `Option<Access>` on `SyntaxTree::Class` lets cross-language reuse stay
/// clean: Python sets it to `None` (no access modifier concept);
/// C# / Java / etc. always set `Some(...)` (the default is
/// language-specific — `internal` for top-level C# class, `private`
/// for nested).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    Public,
    Private,
    Protected,
    Internal,            // C# / Kotlin default for top-level
    ProtectedInternal,   // C# `protected internal`
    PrivateProtected,    // C# `private protected`
    File,                // C# 11 file-scoped accessibility
    Package,             // Java package-private (default for class members)
}

impl Access {
    /// Marker element name(s). Returns one name for simple access
    /// levels, two for the C# compound forms (`protected internal`,
    /// `private protected`) — split into separate markers per the
    /// existing pipeline convention (e.g. `op[bitwise and or]` =
    /// `<bitwise/><and/><or/>`). The "no underscore in names" rule
    /// applies; we split rather than concatenate.
    pub const fn marker_names(self) -> &'static [&'static str] {
        match self {
            Access::Public            => &["public"],
            Access::Private           => &["private"],
            Access::Protected         => &["protected"],
            Access::Internal          => &["internal"],
            Access::ProtectedInternal => &["protected", "internal"],
            Access::PrivateProtected  => &["private", "protected"],
            Access::File              => &["file"],
            Access::Package           => &["package"],
        }
    }

    /// Pick the access level out of a list of marker names. Mirror of
    /// [`Self::marker_names`] — recognises both simple forms
    /// (`public`, `private`, ...) and the C# compound pairs
    /// (`protected internal`, `private protected`). Returns `None`
    /// when no access marker is present.
    pub fn from_marker_names(names: &[&str]) -> Option<Access> {
        let has = |n: &str| names.iter().any(|&m| m == n);
        if has("protected") && has("internal") {
            Some(Access::ProtectedInternal)
        } else if has("private") && has("protected") {
            Some(Access::PrivateProtected)
        } else if has("public") {
            Some(Access::Public)
        } else if has("private") {
            Some(Access::Private)
        } else if has("protected") {
            Some(Access::Protected)
        } else if has("internal") {
            Some(Access::Internal)
        } else if has("file") {
            Some(Access::File)
        } else if has("package") {
            Some(Access::Package)
        } else {
            None
        }
    }

    /// Parse from the source-text of a C# `modifier` node. Returns
    /// `None` for non-access modifiers (`static`, `sealed`, `abstract`,
    /// `partial`, `async`, etc.) — those belong on a separate field.
    pub fn from_csharp_modifier_text(text: &str) -> Option<Access> {
        Some(match text {
            "public"    => Access::Public,
            "private"   => Access::Private,
            "protected" => Access::Protected,
            "internal"  => Access::Internal,
            "file"      => Access::File,
            // Compound forms come as two adjacent modifier tokens in
            // the CST (`protected internal`) — handled in lowering by
            // looking at the pair, not here.
            _ => return None,
        })
    }
}

/// Set of access-and-modifier flags applicable to declarations
/// (class, struct, interface, method, field, property, …).
///
/// **Exhaustive variations principle.** Every flag has a defined
/// value (`false` unless set, `None` for `access` if not applicable).
/// Adding a modifier is one struct-field change; the renderer
/// produces a marker for each true-valued flag automatically.
///
/// **Cross-language usage.** Python uses only `access: None` and
/// `async_`. C# uses many. Java would use access + static + abstract
/// + final (which we'd rename to `sealed` to match C#'s naming, or
/// we add a separate `final_`). The struct holds the union; languages
/// populate the relevant subset.
///
/// **Mutation.** Each field is a typed leaf — `modifiers.access =
/// Some(Access::Private)` or `modifiers.static_ = true`. Re-render
/// produces the matching markers. No XML-level edits needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    /// Access level. `None` for languages without explicit access
    /// modifiers (Python, JavaScript before private fields).
    pub access: Option<Access>,
    /// `static` — bound to the type, not instances.
    pub static_: Flag,
    /// `abstract` — must be overridden / has no implementation.
    pub abstract_: Flag,
    /// `sealed` (C#) / `final class` (Java) — cannot be inherited.
    pub sealed: Flag,
    /// `virtual` (C#) — overridable but not abstract.
    pub virtual_: Flag,
    /// `override` — overrides a base member.
    pub override_: Flag,
    /// `readonly` (C# field) / `final` (Java field) — cannot be
    /// reassigned after initialization.
    pub readonly: Flag,
    /// `partial` (C#) — definition split across multiple files.
    pub partial: Flag,
    /// `async` — async function/method.
    pub async_: Flag,
    /// `const` — compile-time constant.
    pub const_: Flag,
    /// `extern` (C#) — implementation external (DllImport etc.).
    pub extern_: Flag,
    /// `unsafe` (C#) — relaxes safety checks.
    pub unsafe_: Flag,
    /// `volatile` (C#/Java) — non-cacheable reads/writes.
    pub volatile: Flag,
    /// `new` (C#) — explicitly hides an inherited member.
    pub new_: Flag,
    /// `required` (C# 11) — must be assigned during object init.
    pub required: Flag,
    /// `final` (Java field/method/class) — renders as `<final/>`
    /// marker. Distinct from C#'s `readonly` (same semantics, different
    /// wire name) so each language's tests see the marker the
    /// imperative pipeline produced.
    pub final_: Flag,
    /// `synchronized` (Java method) — renders as `<synchronized/>`.
    pub synchronized_: Flag,
    /// `transient` (Java field) — exclude from serialization.
    pub transient: Flag,
    /// `native` (Java method) — implementation supplied by the JVM.
    pub native: Flag,
    /// `strictfp` (Java) — strict floating-point.
    pub strictfp: Flag,
    /// `default` (Java interface method) — has a default body.
    pub default: Flag,
    /// `get` (TypeScript / JS class accessor) — `get foo() {...}` —
    /// renders as `<get/>` marker on the method.
    pub getter: Flag,
    /// `set` (TypeScript / JS class accessor) — `set foo(v) {...}` —
    /// renders as `<set/>` marker on the method.
    pub setter: Flag,
    /// `*` (TypeScript / JS generator function) — renders as
    /// `<generator/>` marker on the method/function.
    pub generator: Flag,
}

impl Modifiers {
    /// True iff no modifier is set (no access, no flags). Renders as
    /// no markers at all.
    pub fn is_empty(&self) -> bool {
        self.access.is_none()
            && !self.static_.is_set() && !self.abstract_.is_set() && !self.sealed.is_set()
            && !self.virtual_.is_set() && !self.override_.is_set() && !self.readonly.is_set()
            && !self.partial.is_set() && !self.async_.is_set() && !self.const_.is_set()
            && !self.extern_.is_set() && !self.unsafe_.is_set() && !self.volatile.is_set()
            && !self.new_.is_set() && !self.required.is_set()
    }

    /// Marker names this modifier set should emit, in stable order
    /// (access first, then alphabetical-ish for predictability).
    /// Used by the renderer to produce zero-width markers.
    pub fn marker_names(&self) -> Vec<&'static str> {
        self.markers_with_spans().into_iter().map(|(n, _)| n).collect()
    }

    /// Same as [`marker_names`](Self::marker_names) but also carries
    /// each marker's source span (from the `Flag`'s anchored keyword
    /// position, or the synthetic point for implicit defaults).
    /// Used by the XML renderer to position each empty `<flag/>`
    /// element at the keyword's source location.
    pub fn markers_with_spans(&self) -> Vec<(&'static str, Option<Span>)> {
        let mut out: Vec<(&'static str, Option<Span>)> = Vec::new();
        if let Some(a) = self.access {
            for n in a.marker_names() { out.push((n, None)); }
        }
        // Source order — matches Java/C# canonical declaration:
        // access  abstract  static  final/readonly  ... .
        // Tests assert ordinal positions (`*[1][self::public]`,
        // `*[2][self::abstract]`), so the order is observable.
        let pairs: [(&Flag, &'static str); 23] = [
            (&self.abstract_,      "abstract"),
            (&self.static_,        "static"),
            (&self.virtual_,       "virtual"),
            (&self.override_,      "override"),
            (&self.sealed,         "sealed"),
            (&self.final_,         "final"),
            (&self.readonly,       "readonly"),
            (&self.partial,        "partial"),
            (&self.async_,         "async"),
            (&self.const_,         "const"),
            (&self.extern_,        "extern"),
            (&self.unsafe_,        "unsafe"),
            (&self.volatile,       "volatile"),
            (&self.new_,           "new"),
            (&self.required,       "required"),
            (&self.synchronized_,  "synchronized"),
            (&self.transient,      "transient"),
            (&self.native,         "native"),
            (&self.strictfp,       "strictfp"),
            (&self.default,        "default"),
            (&self.getter,         "get"),
            (&self.setter,         "set"),
            (&self.generator,      "generator"),
        ];
        for (flag, name) in pairs {
            if flag.is_set() {
                out.push((name, flag.span()));
            }
        }
        out
    }

    /// Reconstruct a `Modifiers` value from the list of marker names
    /// emitted by [`Self::marker_names`] / [`Self::markers_with_spans`].
    /// Used by `from_json` to invert the XML/JSON marker projection
    /// back into the typed flag set.
    ///
    /// Unknown names are ignored (a renderer may emit markers that
    /// don't map to typed flags — e.g. `[sideeffect]` lives in
    /// `extra_markers`, not `Modifiers`). Access markers map back to
    /// the closed enum via [`Access::from_marker_names`].
    pub fn from_marker_names(names: &[&str]) -> Self {
        let mut m = Modifiers::default();
        // Access modifiers come from a closed set; collect first so
        // multi-token access names (`protected internal`) reconstruct
        // correctly.
        m.access = Access::from_marker_names(names);
        for n in names {
            let _ = m.set_flag(n, true);
        }
        m
    }

    /// Flip a modifier flag from text input. Returns Err for unknown
    /// names. Used by the eventual `tractor modify --set foo=true`
    /// CLI surface.
    pub fn set_flag(&mut self, name: &str, value: bool) -> Result<(), &'static str> {
        let flag = Flag::from_bool(value);
        match name {
            "static"       => self.static_ = flag,
            "abstract"     => self.abstract_ = flag,
            "sealed"       => self.sealed = flag,
            "virtual"      => self.virtual_ = flag,
            "override"     => self.override_ = flag,
            "readonly"     => self.readonly = flag,
            "partial"      => self.partial = flag,
            "async"        => self.async_ = flag,
            "const"        => self.const_ = flag,
            "extern"       => self.extern_ = flag,
            "unsafe"       => self.unsafe_ = flag,
            "volatile"     => self.volatile = flag,
            "new"          => self.new_ = flag,
            "required"     => self.required = flag,
            "final"        => self.final_ = flag,
            "synchronized" => self.synchronized_ = flag,
            "transient"    => self.transient = flag,
            "native"       => self.native = flag,
            "strictfp"     => self.strictfp = flag,
            "default"      => self.default = flag,
            "get"          => self.getter = flag,
            "set"          => self.setter = flag,
            "generator"    => self.generator = flag,
            _ => return Err("unknown modifier flag"),
        }
        Ok(())
    }
}

/// `SyntaxTree::Parameter` kind discriminator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamKind {
    /// Regular positional / keyword parameter `x` / `x=default` /
    /// `x: T = default`.
    Regular,
    /// `*args` — adds `<args/>` marker.
    Args,
    /// `**kwargs` — adds `<kwargs/>` marker.
    Kwargs,
}

/// `SyntaxTree::Accessor` kind discriminator — typed closed enum,
/// replacing the legacy `&'static str` parity-track form (C5).
/// The active variant's snake_case name becomes the XML element
/// name (`<get>` / `<set>` / `<init>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessorKind {
    Get,
    Set,
    Init,
}

/// `SyntaxTree::Slot` kind discriminator — typed closed enum naming
/// the value-position role wrapped by `wrap_slot` / `wrap_typed_slot`.
/// Replaces the parity-track `SimpleStatement { element_name: "left"
/// | "right" | "condition" | ... }` form. Adding a new slot kind is
/// a deliberate enum extension; no runtime string discrimination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotKind {
    /// `<left>` — left operand of a binary / assign / compare /
    /// for-target / for-target-list shape.
    Left,
    /// `<right>` — right operand of binary / assign / compare /
    /// for-iterable / for-iterable-list shape.
    Right,
    /// `<condition>` — `if` / `while` / `for-when` / `assert`
    /// / `ternary` condition slot.
    Condition,
    /// `<then>` — `ternary` true-branch / TS conditional-type
    /// true-branch.
    Then,
    /// `<else>` — `ternary` false-branch / TS conditional-type
    /// false-branch. The else-clause of `if`/`for-else`/`while-else`
    /// uses the separate `wrap_clause("else")` shape — those go
    /// through Pass 4's clause container variants.
    Else,
    /// `<as>` — `with item as x` / `case ... as x` rename slot.
    As,
    /// `<filter>` — list-comprehension / generator-expression
    /// `if cond` filter clause.
    Filter,
}

impl SlotKind {
    /// Element name for this slot. Called from the generated
    /// metadata via the `@element_name = element_name_for_slot`
    /// annotation on `SyntaxTree::Slot`.
    pub const fn as_element_name(self) -> &'static str {
        match self {
            SlotKind::Left => "left",
            SlotKind::Right => "right",
            SlotKind::Condition => "condition",
            SlotKind::Then => "then",
            SlotKind::Else => "else",
            SlotKind::As => "as",
            SlotKind::Filter => "filter",
        }
    }

    /// Parse a slot name from a string. Returns `None` for unknown
    /// names; used by `wrap_slot` to validate its argument at the
    /// transition boundary.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "left" => SlotKind::Left,
            "right" => SlotKind::Right,
            "condition" => SlotKind::Condition,
            "then" => SlotKind::Then,
            "else" => SlotKind::Else,
            "as" => SlotKind::As,
            "filter" => SlotKind::Filter,
            _ => return None,
        })
    }
}

impl AccessorKind {
    /// Snake_case element name for this accessor kind. Called from
    /// the generated metadata via the per-variant
    /// `@element_name = element_name_for_accessor` annotation.
    pub const fn as_element_name(self) -> &'static str {
        match self {
            AccessorKind::Get => "get",
            AccessorKind::Set => "set",
            AccessorKind::Init => "init",
        }
    }
}

/// Closed taxonomy of binary / logical operators across all
/// languages. The operator is the source of truth for which
/// variant (`Binary` vs `Logical`) a `left op right` expression
/// becomes, and for the marker name (`<plus/>` / `<and/>` / ...)
/// that decorates the rendered `<operator>` element.
///
/// Replaces the per-variant scalar triple
/// `op_text` / `op_marker` / `op_range` with a typed `Operator`
/// sub-tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OperatorKind {
    // ----- Arithmetic ---------------------------------------------
    #[default]
    Plus,            // +
    Minus,           // -
    Multiply,        // *
    Divide,          // /
    Modulo,          // %
    Power,           // **
    FloorDivide,     // //   (Python)
    MatrixMultiply,  // @    (Python)
    Concat,          // .    (PHP string concat)

    // ----- Bitwise ------------------------------------------------
    BitwiseAnd,         // &
    BitwiseOr,          // |
    BitwiseXor,         // ^
    ShiftLeft,          // <<
    ShiftRight,         // >>
    ShiftRightUnsigned, // >>>  (Java / C# / TS / JS)
    BitwiseClear,       // &^   (Go-specific bit clear)

    // ----- Logical (build_binary returns Logical variant) ---------
    And,             // && / `and`
    Or,              // || / `or`
    Xor,             // xor (PHP keyword)

    // ----- Comparison ---------------------------------------------
    // Languages that have a dedicated `Comparison` variant route via
    // that path instead; the kinds here cover languages where
    // comparison ops still flow through Binary lowering today.
    Equal,           // ==   (or === in TS where it folds to Equal)
    NotEqual,        // != / <> / !==-in-TS
    Identical,       // === (PHP only — value+type identity)
    NotIdentical,    // !== (PHP)
    CaseEqual,       // === (Ruby — case-equality for case/when matching)
    Less,            // <
    LessOrEqual,     // <=
    Greater,         // >
    GreaterOrEqual,  // >=
    Spaceship,       // <=>  (PHP)

    // ----- Keyword binary ----------------------------------------
    Instanceof,      // instanceof  (Java / PHP / TS)
    In,              // in  (TS membership operator)
    ChannelReceive,  // <-  (Go channel ops)

    // ----- Null / coalesce ---------------------------------------
    NullCoalesce,    // ??  (TS / PHP / C#)
}

/// Coarse family grouping for [`OperatorKind`]. Today only the
/// `Logical` case affects which `SyntaxTree` variant gets built
/// (Logical vs Binary); the other families exist for future use
/// and to keep the taxonomy explicit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperatorFamily {
    Arithmetic,
    Bitwise,
    Logical,
    Comparison,
    Keyword,
    Channel,
    NullCoalesce,
}

impl OperatorKind {
    /// Snake_case marker name, projected as the empty-element inside
    /// `<operator>`: `<operator>+<plus/></operator>`. Single source of
    /// truth — every per-language `op_kind` function maps source text
    /// to a kind and the marker name is derived once here.
    pub const fn marker_name(self) -> &'static str {
        match self {
            OperatorKind::Plus => "plus",
            OperatorKind::Minus => "minus",
            OperatorKind::Multiply => "multiply",
            OperatorKind::Divide => "divide",
            OperatorKind::Modulo => "modulo",
            OperatorKind::Power => "power",
            OperatorKind::FloorDivide => "floor_divide",
            OperatorKind::MatrixMultiply => "matrix_multiply",
            OperatorKind::Concat => "concat",
            OperatorKind::BitwiseAnd => "bitwise_and",
            OperatorKind::BitwiseOr => "bitwise_or",
            OperatorKind::BitwiseXor => "bitwise_xor",
            OperatorKind::ShiftLeft => "shift_left",
            OperatorKind::ShiftRight => "shift_right",
            OperatorKind::ShiftRightUnsigned => "shift_right_unsigned",
            OperatorKind::BitwiseClear => "bitwise_clear",
            OperatorKind::And => "and",
            OperatorKind::Or => "or",
            OperatorKind::Xor => "xor",
            OperatorKind::Equal => "equal",
            OperatorKind::NotEqual => "not_equal",
            OperatorKind::Identical => "identical",
            OperatorKind::NotIdentical => "not_identical",
            OperatorKind::CaseEqual => "case_equal",
            OperatorKind::Less => "less",
            OperatorKind::LessOrEqual => "less_or_equal",
            OperatorKind::Greater => "greater",
            OperatorKind::GreaterOrEqual => "greater_or_equal",
            OperatorKind::Spaceship => "spaceship",
            OperatorKind::Instanceof => "instanceof",
            OperatorKind::In => "in",
            OperatorKind::ChannelReceive => "channel_receive",
            OperatorKind::NullCoalesce => "null_coalesce",
        }
    }

    pub const fn family(self) -> OperatorFamily {
        match self {
            OperatorKind::Plus
            | OperatorKind::Minus
            | OperatorKind::Multiply
            | OperatorKind::Divide
            | OperatorKind::Modulo
            | OperatorKind::Power
            | OperatorKind::FloorDivide
            | OperatorKind::MatrixMultiply
            | OperatorKind::Concat => OperatorFamily::Arithmetic,

            OperatorKind::BitwiseAnd
            | OperatorKind::BitwiseOr
            | OperatorKind::BitwiseXor
            | OperatorKind::ShiftLeft
            | OperatorKind::ShiftRight
            | OperatorKind::ShiftRightUnsigned
            | OperatorKind::BitwiseClear => OperatorFamily::Bitwise,

            OperatorKind::And | OperatorKind::Or | OperatorKind::Xor => OperatorFamily::Logical,

            OperatorKind::Equal
            | OperatorKind::NotEqual
            | OperatorKind::Identical
            | OperatorKind::NotIdentical
            | OperatorKind::CaseEqual
            | OperatorKind::Less
            | OperatorKind::LessOrEqual
            | OperatorKind::Greater
            | OperatorKind::GreaterOrEqual
            | OperatorKind::Spaceship => OperatorFamily::Comparison,

            OperatorKind::Instanceof | OperatorKind::In => OperatorFamily::Keyword,
            OperatorKind::ChannelReceive => OperatorFamily::Channel,
            OperatorKind::NullCoalesce => OperatorFamily::NullCoalesce,
        }
    }

    /// Build the binary-or-logical variant for two operands. Owns the
    /// whole construction — wraps operands in `Expression`, boxes
    /// them, builds the inner `Operator` sub-tree, and dispatches to
    /// `Binary` vs `Logical` via [`family`](Self::family). Call sites
    /// drop the wrap/box/variant-pick boilerplate.
    pub fn build_binary(
        self,
        left: SyntaxTree,
        right: SyntaxTree,
        text: String,
        op_range: ByteRange,
        range: ByteRange,
        span: Span,
    ) -> SyntaxTree {
        let op = Box::new(SyntaxTree::Operator {
            text,
            kind: self,
            range: op_range,
            span,
        });
        let left = Box::new(left.wrap_expression());
        let right = Box::new(right.wrap_expression());
        match self.family() {
            OperatorFamily::Logical => {
                SyntaxTree::Logical { left, op, right, range, span }
            }
            _ => SyntaxTree::Binary { left, op, right, range, span },
        }
    }
}

/// Closed set of markers attachable to [`SyntaxTree::Expression`]
/// (the Principle #15 value-host element). Each marker renders as
/// an empty child element inside `<expression>`: `<expression><try/>x?</expression>`.
///
/// Replaces the prior open-string `marker: Option<&'static str>` field
/// — the typed enum guarantees we cover every emitted marker name in
/// one place, and `from_marker_name` gives `from_json` round-trip
/// support.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpressionMarker {
    /// `non_null` — C# postfix non-null assertion `obj!`.
    NonNull,
    /// `await` — `await x` in expression position (where the
    /// expression host is the stable home for the marker per
    /// Principle #15).
    Await,
    /// `try` — Rust `?` postfix operator and `try { ... }` block.
    Try,
}

impl ExpressionMarker {
    /// Snake_case name emitted as the empty marker element name.
    pub const fn marker_name(self) -> &'static str {
        match self {
            ExpressionMarker::NonNull => "non_null",
            ExpressionMarker::Await => "await",
            ExpressionMarker::Try => "try",
        }
    }

    /// Inverse of [`marker_name`]: used by `from_json` to reconstruct
    /// the enum from its serialised name.
    pub fn from_marker_name(name: &str) -> Option<Self> {
        Some(match name {
            "non_null" => ExpressionMarker::NonNull,
            "await" => ExpressionMarker::Await,
            "try" => ExpressionMarker::Try,
            _ => return None,
        })
    }
}

// ---------------------------------------------------------------------------
// Per-variant `element_name_for_*` overrides (C5). The metadata
// codegen looks for `@element_name = <fn_ident>` on a variant's doc
// comment; when found, the variant's `element_name_of` arm calls the
// named function instead of falling back to snake_case(variant). All
// override functions live here so the knowledge stays local —
// no convention-based detection in the codegen.

/// Override for [`SyntaxTree::Slot`]: routes through the typed
/// [`SlotKind`] discriminator. The variant carries the kind so this
/// is a one-line const lookup — no leak, no map.
pub fn element_name_for_slot(t: &SyntaxTree) -> &'static str {
    if let SyntaxTree::Slot { kind, .. } = t {
        kind.as_element_name()
    } else {
        "slot"
    }
}

/// Override for [`SyntaxTree::Accessor`]: routes through the typed
/// [`AccessorKind`] discriminator.
pub fn element_name_for_accessor(t: &SyntaxTree) -> &'static str {
    if let SyntaxTree::Accessor { kind, .. } = t {
        kind.as_element_name()
    } else {
        "accessor"
    }
}

/// Override for [`SyntaxTree::SimpleStatement`]: the legacy
/// parity-track wrapper carries an open-set `&'static str`
/// element name (~87 distinct values across the codebase).
/// Retirement to typed slot/clause/statement variants is tracked in
/// [`tractor/src/tree/syntax/PARITY_TRACK_RETIREMENT.md`].
pub fn element_name_for_simple_statement(t: &SyntaxTree) -> &'static str {
    if let SyntaxTree::SimpleStatement { element_name, .. } = t {
        *element_name
    } else {
        "simple_statement"
    }
}

/// Override for [`SyntaxTree::FieldWrap`]: open-set `wrapper`
/// string. Retirement to typed variants tracked in the same doc.
pub fn element_name_for_field_wrap(t: &SyntaxTree) -> &'static str {
    if let SyntaxTree::FieldWrap { wrapper, .. } = t {
        *wrapper
    } else {
        "field_wrap"
    }
}

/// Override for [`SyntaxTree::Atom`]: open-set `element_name`
/// string used by T-SQL identifier classification.
pub fn element_name_for_atom(t: &SyntaxTree) -> &'static str {
    if let SyntaxTree::Atom { element_name, .. } = t {
        *element_name
    } else {
        "atom"
    }
}

/// Override for [`SyntaxTree::ObjectAccess`]: the variant is named
/// `ObjectAccess` for typed-tree clarity (distinct from the
/// visibility-level `Access` enum and the `AccessReceiver` kind),
/// but renders as plain `<object>` to match the conventional XPath
/// vocabulary that pre-dates the typed promotion. No state — every
/// `ObjectAccess` becomes `<object>`.
pub fn element_name_for_object_access(_t: &SyntaxTree) -> &'static str {
    "object"
}

/// Override for [`SyntaxTree::Raw`]: tree-sitter passthrough nodes
/// render with their grammar `kind` as the element name
/// (`<let_declaration>`, `<object>`, `<pair>`, ...). The `kind`
/// string is leaked into the static pool once per distinct kind —
/// acceptable here because raw mode is a developer-facing debug
/// view, the kind set per grammar is bounded, and the leak avoids
/// extending the `&'static str` codegen protocol for one use site.
pub fn element_name_for_raw(t: &SyntaxTree) -> &'static str {
    if let SyntaxTree::Raw { kind, .. } = t {
        Box::leak(kind.clone().into_boxed_str())
    } else {
        "raw"
    }
}

/// Override for [`SyntaxTree::GenericType`]: parameterized type
/// references render as plain `<type>` (matching non-parameterized
/// `SimpleStatement { element_name: "type", ... }`) so XPath queries
/// against the type namespace (`//type`, `//type[.='List<int>']`)
/// see both forms uniformly. No state — every `GenericType` becomes
/// `<type>`.
pub fn element_name_for_generic_type(_t: &SyntaxTree) -> &'static str {
    "type"
}

/// Override for [`SyntaxTree::TypeParameter`]: PEP 695 / generic
/// parameter declarations render as `<type>` so the type namespace
/// stays uniform under `<generic>` — `<generic><type><name>T</name></type></generic>`
/// rather than `<generic><type_parameter><name>T</name></type_parameter></generic>`.
pub fn element_name_for_type_parameter(_t: &SyntaxTree) -> &'static str {
    "type"
}

/// One step in an [`SyntaxTree::ObjectAccess`] chain.
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
    /// `optional: true` adds an `<optional/>` empty marker first child
    /// (visible in tree-text as `<member[optional]>`), modelling
    /// null-conditional access (`?.`) in C# / TS / Ruby etc.
    /// **Architectural note:** with this flag, conditional and regular
    /// member access produce the *same shape* differing only by the
    /// marker — exactly Principle #15. The existing C# pipeline has a
    /// deferred design problem here (`<member[conditional]>` parent +
    /// `<condition>` wrapper, see `todo/39-…md` lesson 5d); the
    /// typed-tree architecture sidesteps it by construction.
    Member {
        property_range: ByteRange,
        property_span: Span,
        optional: bool,
        range: ByteRange,
        span: Span,
    },

    /// `[indices...]` — emits `<index>{gap}{indices}{gap}...</index>`.
    /// Iter 345 renamed `subscript` → `index`; one tree variant covers
    /// both the chain-segment case (`a[0]`) and the future standalone
    /// case.
    Index {
        indices: Vec<SyntaxTree>,
        range: ByteRange,
        span: Span,
    },

    /// `(args)` — call segment in a chain `a.b()`. Renders as
    /// `<call>[<name>method</name>]{args...}</call>`. When the
    /// preceding chain step is a member access (`.Method(...)`),
    /// `name` carries the method name range so the renderer folds
    /// the member+call into a single `<call><name>Method</name>...</call>`
    /// element — matches the imperative pipeline's chain inversion.
    /// For bare invocations (`f()`), use `SyntaxTree::Call` instead.
    Call {
        name: Option<ByteRange>,
        name_span: Option<Span>,
        arguments: Vec<SyntaxTree>,
        range: ByteRange,
        span: Span,
    },
}

impl AccessSegment {
    pub fn span(&self) -> Span {
        match self {
            AccessSegment::Member { span, .. } => *span,
            AccessSegment::Index { span, .. } => *span,
            AccessSegment::Call { span, .. } => *span,
        }
    }

    pub fn range(&self) -> ByteRange {
        match self {
            AccessSegment::Member { range, .. } => *range,
            AccessSegment::Index { range, .. } => *range,
            AccessSegment::Call { range, .. } => *range,
        }
    }
}

/// Receiver of an [`SyntaxTree::ObjectAccess`] chain. Distinguishes the four
/// reserved-keyword receivers (`base`, `this`, `super`, `self`) from
/// arbitrary expression receivers so the renderer dispatches by type
/// rather than by inspecting source text.
///
/// Lowering for every language constructs receivers via
/// [`AccessReceiver::from_tree`], which classifies a `SyntaxTree::Name`
/// whose text matches one of the four keywords into the corresponding
/// typed variant. Everything else falls through to
/// [`AccessReceiver::Instance`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessReceiver {
    /// `base` keyword — C# super-class reference.
    Base { range: ByteRange, span: Span },
    /// `this` keyword — Java / C# / TS / Rust instance reference.
    This { range: ByteRange, span: Span },
    /// `super` keyword — Python / Java / Ruby / TS / Rust parent reference.
    Super { range: ByteRange, span: Span },
    /// `self` keyword — Python / Rust instance reference.
    Self_ { range: ByteRange, span: Span },
    /// Arbitrary expression receiver — any non-keyword tree.
    Instance(Box<SyntaxTree>),
}

impl AccessReceiver {
    /// Classify a tree into a receiver. A `SyntaxTree::Name` whose text
    /// matches one of the language's reserved access keywords becomes
    /// the corresponding typed variant; everything else wraps in
    /// `Instance`. The `keywords` list scopes classification per
    /// language so e.g. a Java identifier named `base` doesn't render
    /// as the C#-style `<base/>` marker.
    pub fn from_tree(tree: SyntaxTree, keywords: &[&'static str]) -> Self {
        if let SyntaxTree::Name { text, range, span } = &tree {
            if keywords.contains(&text.as_str()) {
                let range = *range;
                let span = *span;
                return match text.as_str() {
                    "base" => AccessReceiver::Base { range, span },
                    "this" => AccessReceiver::This { range, span },
                    "super" => AccessReceiver::Super { range, span },
                    "self" => AccessReceiver::Self_ { range, span },
                    _ => unreachable!("keyword in list but not a recognized receiver keyword"),
                };
            }
        }
        AccessReceiver::Instance(Box::new(tree))
    }

    pub fn range(&self) -> ByteRange {
        match self {
            AccessReceiver::Base { range, .. }
            | AccessReceiver::This { range, .. }
            | AccessReceiver::Super { range, .. }
            | AccessReceiver::Self_ { range, .. } => *range,
            AccessReceiver::Instance(t) => t.range(),
        }
    }

    pub fn span(&self) -> Span {
        match self {
            AccessReceiver::Base { span, .. }
            | AccessReceiver::This { span, .. }
            | AccessReceiver::Super { span, .. }
            | AccessReceiver::Self_ { span, .. } => *span,
            AccessReceiver::Instance(t) => t.span(),
        }
    }

    /// XML element name for keyword receivers; `None` for `Instance`.
    /// Lets the renderer marshal the receiver without per-variant code.
    pub fn keyword_element(&self) -> Option<&'static str> {
        match self {
            AccessReceiver::Base { .. } => Some("base"),
            AccessReceiver::This { .. } => Some("this"),
            AccessReceiver::Super { .. } => Some("super"),
            AccessReceiver::Self_ { .. } => Some("self"),
            AccessReceiver::Instance(_) => None,
        }
    }
}

/// Body of an [`SyntaxTree::Lambda`]. Distinguishes statement-block
/// bodies (`x => { stmts; }`) from expression bodies (`x => x + 1`)
/// at the type level so the renderer dispatches by variant rather
/// than by inspecting the inner tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LambdaBody {
    /// Block-bodied lambda — inner is a [`SyntaxTree::Body`].
    Block(Box<SyntaxTree>),
    /// Expression-bodied lambda — inner is an expression tree.
    Expression(Box<SyntaxTree>),
}

impl LambdaBody {
    pub fn inner(&self) -> &SyntaxTree {
        match self {
            LambdaBody::Block(b) | LambdaBody::Expression(b) => b,
        }
    }

    pub fn inner_mut(&mut self) -> &mut SyntaxTree {
        match self {
            LambdaBody::Block(b) | LambdaBody::Expression(b) => b.as_mut(),
        }
    }

    pub fn range(&self) -> ByteRange { self.inner().range() }
    pub fn span(&self) -> Span { self.inner().span() }
}

/// Typed wrapper for expression-position slots (`SyntaxTree::Variable.value`,
/// `SyntaxTree::If.condition`, `SyntaxTree::Binary.left/right`, `SyntaxTree::Return.value`, …).
/// The type system enforces Principle #15: anything in these slots
/// renders as `<expression>` so XPath queries match a uniform parent
/// regardless of inner shape.
///
/// Construct via [`Expression::wrap`], which is idempotent: if the
/// supplied `SyntaxTree` already renders as `<expression>` (`SyntaxTree::Expression`
/// variant, or `SyntaxTree::SimpleStatement { element_name: "expression", … }`
/// — used for `<expression[ref]>`/etc. with extra markers), the inner
/// is stored as-is; otherwise it is wrapped in `SyntaxTree::Expression`. Either
/// way, exactly one `<expression>` element renders per slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expression {
    pub inner: Box<SyntaxTree>,
}

impl Expression {
    pub fn wrap(inner: SyntaxTree) -> Self {
        let already = matches!(&inner,
            SyntaxTree::Expression { .. }
                | SyntaxTree::SimpleStatement { element_name: "expression", .. }
        );
        if already {
            Self { inner: Box::new(inner) }
        } else {
            let range = inner.range();
            let span = inner.span();
            Self {
                inner: Box::new(SyntaxTree::Expression {
                    inner: Box::new(inner),
                    marker: None,
                    range,
                    span,
                }),
            }
        }
    }

    pub fn range(&self) -> ByteRange { self.inner.range() }
    pub fn span(&self) -> Span { self.inner.span() }
}

impl SyntaxTree {
    /// Wrap this tree in a `SyntaxTree::Expression` value-position host
    /// (Principle #15). Idempotent — already-wrapped trees pass through
    /// unchanged. Lowering sites call this so the renderer never has
    /// to decide whether to insert `<expression>`; the tree shape is
    /// final.
    pub fn wrap_expression(self) -> SyntaxTree {
        if matches!(self, SyntaxTree::Expression { .. }) {
            return self;
        }
        if matches!(&self, SyntaxTree::SimpleStatement { element_name: "expression", .. }) {
            return self;
        }
        let range = self.range();
        let span = self.span();
        SyntaxTree::Expression {
            inner: Box::new(self),
            marker: None,
            range,
            span,
        }
    }

    /// Wrap this tree in `<expression>` host(s), threading through
    /// `Inline` so each child of an `Inline` is wrapped individually.
    /// Used at lowering sites whose value position may be a multi-value
    /// `Inline` (e.g. `return a, b`, multi-target assign). Idempotent.
    pub fn wrap_expression_inline_aware(self) -> SyntaxTree {
        if let SyntaxTree::Inline { children, list_name, range, span } = self {
            let wrapped = children.into_iter()
                .map(|c| c.wrap_expression())
                .collect();
            return SyntaxTree::Inline { children: wrapped, list_name, range, span };
        }
        self.wrap_expression()
    }

    /// Pre-wrap a value-position subtree in a named slot wrapper:
    /// `SimpleStatement(slot_name, [Expression(self)])`. The inner
    /// `<expression>` host (Principle #15) wraps the value, and the
    /// outer `<{slot_name}>` element (e.g. `<left>`, `<right>`,
    /// `<condition>`, `<value>`) is the structural slot the parent
    /// expects.
    ///
    /// Used by `Binary`, `Assign`, `For`, `If`, `While`, `Ternary`,
    /// `ExceptHandler`, `Parameter`, `TypeAlias` lowerings to lift
    /// renderer-side slot synthesis into the tree.
    ///
    /// `Inline` values are threaded through: each child is wrapped
    /// in `<expression>` first, then the whole `Inline` becomes the
    /// slot's children (one slot wrapping a flat list).
    pub fn wrap_slot(self, slot_name: &'static str) -> SyntaxTree {
        let kind = SlotKind::from_name(slot_name).unwrap_or_else(|| {
            panic!(
                "wrap_slot called with unknown slot name {:?}; expected one of \
                 left/right/condition/then/else/as/filter",
                slot_name,
            )
        });
        self.wrap_typed_slot(kind)
    }

    /// Typed equivalent of [`Self::wrap_slot`]: takes a [`SlotKind`]
    /// directly so call sites can opt out of the string-name → enum
    /// validation roundtrip. Preferred for new code.
    pub fn wrap_typed_slot(self, kind: SlotKind) -> SyntaxTree {
        let wrapped = self.wrap_expression_inline_aware();
        let range = wrapped.range();
        let span = wrapped.span();
        // If the wrapped value is an `Inline`, hoist its children
        // directly under the slot wrapper so they render as siblings
        // (rather than nested under the slot via `<inline>`).
        let children = if let SyntaxTree::Inline { children: inner, .. } = wrapped {
            inner
        } else {
            vec![wrapped]
        };
        SyntaxTree::Slot { kind, children, range, span }
    }

    /// Inverse of [`wrap_slot`]: if `self` is a slot wrapper
    /// `SimpleStatement(_, [Expression(inner)])`, return `inner`.
    /// Otherwise return `self`. The Expression host is also peeled.
    ///
    /// Used by JSON / DataTree projections that historically operated
    /// on the bare operand (ergonomic flat shape) — they call this
    /// to skip past the structural slot wrapper added by lowering.
    pub fn unwrap_slot(&self) -> &SyntaxTree {
        let children = match self {
            // Pass 1: typed slot wrapper.
            SyntaxTree::Slot { children, .. } => children,
            // Pre-Pass-1: raw construction sites still emit
            // `SimpleStatement { element_name: "left" | ... }`. Recognise
            // them for back-compat so the unwrap helper covers both
            // forms during the migration window.
            SyntaxTree::SimpleStatement { children, .. } => children,
            _ => return self,
        };
        if children.len() == 1 {
            if let SyntaxTree::Expression { inner, marker: None, .. } = &children[0] {
                return inner.as_ref();
            }
            return &children[0];
        }
        self
    }

    /// Wrap a statement-position subtree in a named clause wrapper:
    /// `SimpleStatement(slot_name, [self])`. No `<expression>` host
    /// (statement positions, not value positions). Used for `<else>`,
    /// `<finally>`, etc. clauses around bodies — produces e.g.
    /// `<else><body>...</body></else>`.
    pub fn wrap_clause(self, slot_name: &'static str) -> SyntaxTree {
        let range = self.range();
        let span = self.span();
        SyntaxTree::SimpleStatement {
            element_name: slot_name,
            modifiers: Modifiers::default(),
            extra_markers: Vec::new(),
            children: vec![self],
            range,
            span,
        }
    }

    /// Wrap this tree in a `<extends><type>...</type></extends>`
    /// host so the renderer doesn't have to. Idempotent —
    /// already-wrapped shapes (`SimpleStatement{element_name:
    /// "extends" | "implements"}`) pass through unchanged.
    pub fn wrap_extends(self) -> SyntaxTree {
        if matches!(
            &self,
            SyntaxTree::SimpleStatement { element_name: "extends", .. }
                | SyntaxTree::SimpleStatement { element_name: "implements", .. }
        ) {
            return self;
        }
        let range = self.range();
        let span = self.span();
        SyntaxTree::SimpleStatement {
            element_name: "extends",
            modifiers: Modifiers::default(),
            extra_markers: Vec::new(),
            children: vec![self.wrap_type()],
            range,
            span,
        }
    }

    /// Wrap this tree in a `<type>` host using
    /// [`SyntaxTree::SimpleStatement`]. Idempotent — already-typed
    /// shapes (`GenericType`, `SimpleStatement{element_name: "type"}`,
    /// `FieldWrap{wrapper: "type"}`) pass through unchanged. Lowering
    /// calls this at type-position slots so the renderer never has to
    /// decide whether to insert `<type>`.
    pub fn wrap_type(self) -> SyntaxTree {
        let already_typed = matches!(
            &self,
            SyntaxTree::GenericType { .. }
                | SyntaxTree::SimpleStatement { element_name: "type", .. }
                | SyntaxTree::SimpleStatement { element_name: "predicate", .. }
                | SyntaxTree::FieldWrap { wrapper: "type", .. }
        );
        if already_typed {
            return self;
        }
        let range = self.range();
        let span = self.span();
        SyntaxTree::SimpleStatement {
            element_name: "type",
            modifiers: Modifiers::default(),
            extra_markers: Vec::new(),
            children: vec![self],
            range,
            span,
        }
    }

    /// Source span of this node. Used for XML attribute emission.
    /// Delegates to the generated reflection accessor.
    pub fn span(&self) -> Span {
        super::metadata_generated::span_of(self)
    }

    /// Source byte range of this node. Used for verbatim-source
    /// recovery (`source[range]`) and for gap-text computation in the
    /// renderer. Delegates to the generated reflection accessor.
    pub fn range(&self) -> ByteRange {
        super::metadata_generated::range_of(self)
    }

    /// True iff this node's range refers to a real source position
    /// (parser output or parser-derived fill-in). False for synthetic
    /// nodes built without source — the renderer's gap-fallback chain
    /// (S13-Z6) consults this to decide between source slicing and
    /// canonical defaults.
    pub fn is_anchored(&self) -> bool {
        self.range().is_anchored()
    }

    /// Stored text for scalar-leaf variants (`Name`, `Atom`, `Int`,
    /// `Float`, `String`, `True`, `False`, `None`, `Null`). Returns
    /// `None` for compound variants. The renderer uses this to emit
    /// leaf literals without consulting the source string (S13-Z1).
    /// Delegates to the generated reflection accessor.
    pub fn scalar_text(&self) -> Option<&str> {
        super::metadata_generated::scalar_text_of(self)
    }

    /// Replace the stored text on a scalar-leaf variant. Returns
    /// `Err` for compound variants. Used by the editable-trees
    /// mutation pipeline (S15-Z3): after [`find_by_id_syntax`]
    /// locates a node, the mutation rewrites its stored text and the
    /// renderer emits the new literal.
    pub fn set_scalar_text(&mut self, new_text: &str) -> Result<(), &'static str> {
        match self {
            SyntaxTree::Name { text, .. }
            | SyntaxTree::Int { text, .. }
            | SyntaxTree::Float { text, .. }
            | SyntaxTree::String { text, .. }
            | SyntaxTree::True { text, .. }
            | SyntaxTree::False { text, .. }
            | SyntaxTree::None { text, .. }
            | SyntaxTree::Atom { text, .. }
            | SyntaxTree::Null { text, .. } => {
                *text = new_text.to_string();
                Ok(())
            }
            _ => Err("set_scalar_text only applies to scalar leaf variants"),
        }
    }
}

impl SyntaxTree {
    /// Construct a `Function` or `Method` variant. Transitional
    /// helper used by lowering sites that still pick the variant via
    /// the legacy `"function"` / `"method"` `element_name` value.
    #[inline]
    pub fn function_or_method(
        element_name: &'static str,
        modifiers: Modifiers,
        decorators: Vec<SyntaxTree>,
        name: Box<SyntaxTree>,
        generics: Vec<SyntaxTree>,
        parameters: Vec<SyntaxTree>,
        returns: Option<Box<SyntaxTree>>,
        throws: Vec<SyntaxTree>,
        body: Option<Box<SyntaxTree>>,
        range: ByteRange,
        span: Span,
    ) -> SyntaxTree {
        match element_name {
            "method" => SyntaxTree::Method { modifiers, decorators, name, generics, parameters, returns, throws, body, range, span },
            _ => SyntaxTree::Function { modifiers, decorators, name, generics, parameters, returns, throws, body, range, span },
        }
    }

    /// Construct a `Variable`, `Field`, or `Event` variant.
    #[inline]
    pub fn variable_or_field(
        element_name: &'static str,
        modifiers: Modifiers,
        decorators: Vec<SyntaxTree>,
        type_ann: Option<Box<SyntaxTree>>,
        name: Box<SyntaxTree>,
        value: Option<Expression>,
        range: ByteRange,
        span: Span,
    ) -> SyntaxTree {
        match element_name {
            "field" => SyntaxTree::Field { modifiers, decorators, type_ann, name, value, range, span },
            "event" => SyntaxTree::Event { modifiers, decorators, type_ann, name, value, range, span },
            _ => SyntaxTree::Variable {
                modifiers,
                decorators,
                extra_markers: Vec::new(),
                type_ann,
                name,
                value,
                range,
                span,
            },
        }
    }

    /// Construct an `Except` or `Catch` variant.
    #[inline]
    pub fn except_or_catch(
        kind: &'static str,
        type_target: Option<Box<SyntaxTree>>,
        binding: Option<Box<SyntaxTree>>,
        filter: Option<Box<SyntaxTree>>,
        body: Box<SyntaxTree>,
        range: ByteRange,
        span: Span,
    ) -> SyntaxTree {
        match kind {
            "catch" => SyntaxTree::Catch { type_target, binding, filter, body, range, span },
            _ => SyntaxTree::Except { type_target, binding, filter, body, range, span },
        }
    }

    /// Direct tree children, in source order. Delegates to the
    /// generated `children_of` accessor in `metadata_generated`.
    /// Excludes synthetic render-time wrappers and modifier markers —
    /// those are rendering metadata, not tree.
    pub(crate) fn children(&self) -> Vec<&SyntaxTree> {
        super::metadata_generated::children_of(self)
    }
}

/// Unifying contract over the three tree families (`SyntaxTree`,
/// `DataTree`, `SqlTree`). Lets generic pre-order walkers — ID
/// stamping, ID lookup, coverage audits — work without per-variant
/// match arms.
///
/// Children come back in source order so callers don't have to
/// repeat the sort. `Vec<&_ T>` is intentional rather than a borrowed
/// iterator: it lets the implementation push disjoint mutable borrows
/// out of multiple struct fields in one match arm.
pub trait TreeNode: Sized {
    fn span(&self) -> Span;
    fn span_mut(&mut self) -> &mut Span;
    fn range(&self) -> ByteRange;
    fn children(&self) -> Vec<&Self>;
    fn children_mut(&mut self) -> Vec<&mut Self>;

    /// True iff this node's range refers to a real source position.
    /// False for synthetic nodes built without source.
    fn is_anchored(&self) -> bool { self.range().is_anchored() }

    /// Verbatim source slice covered by this node, equivalent to
    /// `self.range().slice(source)`.
    fn to_source<'a>(&self, source: &'a str) -> &'a str {
        self.range().slice(source)
    }
}

impl TreeNode for SyntaxTree {
    fn span(&self) -> Span { self.span() }
    fn range(&self) -> ByteRange { self.range() }

    fn span_mut(&mut self) -> &mut Span { super::metadata_generated::span_mut_of(self) }

    fn children(&self) -> Vec<&Self> { self.children() }

    fn children_mut(&mut self) -> Vec<&mut Self> { super::metadata_generated::children_mut_of(self) }
}

/// Round-trip helper: recover the original source slice covered by
/// this tree node. Equivalent to `tree.range().slice(source)`.
///
/// **Round-trip identity:** `to_source(lower(parse(s)), s) == s` (the
/// root tree's range covers the whole source). For sub-trees,
/// `to_source(child, source)` is the verbatim source slice that
/// produced `child`.
pub fn to_source<'a>(tree: &SyntaxTree, source: &'a str) -> &'a str {
    tree.range().slice(source)
}
