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

    /// `<module>` / `<unit>` / `<program>` — top-level program. The
    /// CST root for languages that have one. Children are
    /// statement-or-declaration tree. `element_name` lets each language
    /// pick its own name to match the existing pipeline:
    /// - Python: `"module"`
    /// - C# / TypeScript: `"unit"` or `"program"` (TBD per language)
    /// - Java: `"program"`
    /// Cross-language unification of this name is a Principle #5
    /// audit candidate but requires the existing pipeline's choice
    /// per language to be revisited; we keep parity for now.
    Module {
        element_name: &'static str,
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
    /// `<expression[marker]>` in tree-text view). Used for:
    /// - `non_null` — C#'s `obj!` postfix non-null assertion
    /// - `await` — `await x` (when not in a statement context)
    /// - More may be added as needed.
    ///
    /// **Why on the host, not the operand:** Principle #15 — markers
    /// live in stable predictable locations. The expression host is
    /// ALWAYS present in value positions; the marker decorates it
    /// rather than appearing on the bare inner name/expression.
    Expression {
        inner: Box<SyntaxTree>,
        marker: Option<&'static str>,
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
        receiver: AccessReceiver,
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
        /// Element name. "binary" for arithmetic / bitwise / shift /
        /// comparison; "logical" for short-circuit boolean (`and`,
        /// `or`). Lets the renderer emit the right outer element
        /// without changing shape.
        element_name: &'static str,
        op_text: String,
        op_marker: &'static str,
        op_range: ByteRange,
        left: Box<SyntaxTree>,
        right: Box<SyntaxTree>,
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
        extra_markers: &'static [&'static str],
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

    /// `<type[generic]>` — `Name[T, U, ...]` generic type expression.
    /// `name` is the base type name; `params` are the type arguments.
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
    If {
        condition: Box<SyntaxTree>,
        body: Box<SyntaxTree>,             // SyntaxTree::Body
        else_branch: Option<Box<SyntaxTree>>, // SyntaxTree::ElseIf or SyntaxTree::Else
        range: ByteRange,
        span: Span,
    },

    /// `<else_if>` — `elif cond: body`. Used inside If's else_branch
    /// to keep elif chains flat.
    ElseIf {
        condition: Box<SyntaxTree>,
        body: Box<SyntaxTree>,
        else_branch: Option<Box<SyntaxTree>>,
        range: ByteRange,
        span: Span,
    },

    /// `<else>` — `else: body`.
    Else { body: Box<SyntaxTree>, range: ByteRange, span: Span },

    /// `<for>` — `for target in iter: body [else: body]`.
    /// `<for[async]>` adds an `<async/>` marker.
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
    SimpleStatement {
        element_name: &'static str,
        modifiers: Modifiers,
        /// Extra static markers to emit before children, in order. Used
        /// for pattern combinators (`<and/>`, `<or/>`), keyword markers
        /// (`<stackalloc/>`, `<ref/>`, `<var/>`) etc. — markers that the
        /// imperative pipeline attaches as siblings of anonymous-keyword
        /// text (Principle: every keyword in an element's text must
        /// have a corresponding marker sibling).
        extra_markers: &'static [&'static str],
        children: Vec<SyntaxTree>,
        range: ByteRange,
        span: Span,
    },

    /// `<try>` — `try { body } catch (...) { ... } finally { ... }`
    /// (C# / Java) or `try: ... except E: ... else: ... finally: ...`
    /// (Python). Shared cross-language. `try_body` is the protected
    /// block; `handlers` are catch/except clauses; `else_body` runs
    /// when no exception (Python only); `finally_body` always runs.
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
    ExceptHandler {
        kind: &'static str,            // "except" | "catch"
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
    Function {
        /// Element name. "function" for Python `def`; "method" for
        /// C# `method_declaration` (matching the imperative pipeline's
        /// `Method` rename). Cross-language asymmetry is intentional —
        /// users query `<method>` in C# and `<function>` in Python.
        element_name: &'static str,
        modifiers: Modifiers,
        decorators: Vec<SyntaxTree>,
        name: Box<SyntaxTree>,                  // SyntaxTree::Name
        /// Generic type parameters (each is a [`SyntaxTree::TypeParameter`]).
        /// Empty `Vec` means no generics — the renderer iterates the
        /// items directly with no `Generic` wrapper node.
        generics: Vec<SyntaxTree>,
        parameters: Vec<SyntaxTree>,            // each SyntaxTree::Parameter / SyntaxTree::PositionalSeparator / SyntaxTree::KeywordSeparator
        returns: Option<Box<SyntaxTree>>,       // SyntaxTree::Returns
        /// `throws E1, E2` clause on Java method declarations. Each
        /// entry is the lowering of one exception-type target and
        /// renders as `<throws>/<type>/<name>` (Principle #18 — name
        /// the relationship after the operator, one sibling per
        /// target). Empty for languages without checked exceptions.
        throws: Vec<SyntaxTree>,
        body: Option<Box<SyntaxTree>>,          // SyntaxTree::Body — None for abstract / interface methods
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
    Class {
        kind: &'static str,            // "class" | "struct" | "interface" | "record"
        modifiers: Modifiers,
        decorators: Vec<SyntaxTree>,
        name: Box<SyntaxTree>,
        /// Generic type parameters (each is a [`SyntaxTree::TypeParameter`]).
        /// Empty `Vec` means no generics. The renderer iterates items
        /// directly with no `Generic` wrapper node.
        generics: Vec<SyntaxTree>,
        bases: Vec<SyntaxTree>,                 // each is a base expression
        where_clauses: Vec<SyntaxTree>,         // C# `where T : ...` constraints (other languages: empty)
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
    Parameter {
        kind: ParamKind,
        extra_markers: &'static [&'static str],
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
        leading: bool,
        trailing: bool,
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
    /// [`SyntaxTree::Access`] chain segment instead. (Future: add
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

    /// `<accessor>` — one of `get`, `set`, `init` inside a property's
    /// `{ ... }`. Body is optional (auto-implemented properties have
    /// no body).
    Accessor {
        modifiers: Modifiers,              // Some accessors have their own access modifier
        kind: &'static str,                // "get" | "set" | "init"
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

    /// `<variable>` — `var x = value;` / `int x = value;` /
    /// `int x;`. Used for local variable declarations and class
    /// fields. Renders
    /// `<variable>[<type>...</type>]<name>...</name>[value-expr]</variable>`.
    Variable {
        /// Element name. "variable" for local declarations; "field"
        /// for class-level field declarations. C# uses both; Python
        /// uses neither (assignments take a different tree path).
        element_name: &'static str,
        /// Access + flag modifiers. Empty for locals (their modifiers
        /// like `const` are very limited); fields use them fully.
        modifiers: Modifiers,
        /// Attributes/decorators on the declaration (C# `[Attr]` for
        /// fields, future Java annotations). Empty for locals.
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
    pub static_: bool,
    /// `abstract` — must be overridden / has no implementation.
    pub abstract_: bool,
    /// `sealed` (C#) / `final class` (Java) — cannot be inherited.
    pub sealed: bool,
    /// `virtual` (C#) — overridable but not abstract.
    pub virtual_: bool,
    /// `override` — overrides a base member.
    pub override_: bool,
    /// `readonly` (C# field) / `final` (Java field) — cannot be
    /// reassigned after initialization.
    pub readonly: bool,
    /// `partial` (C#) — definition split across multiple files.
    pub partial: bool,
    /// `async` — async function/method.
    pub async_: bool,
    /// `const` — compile-time constant.
    pub const_: bool,
    /// `extern` (C#) — implementation external (DllImport etc.).
    pub extern_: bool,
    /// `unsafe` (C#) — relaxes safety checks.
    pub unsafe_: bool,
    /// `volatile` (C#/Java) — non-cacheable reads/writes.
    pub volatile: bool,
    /// `new` (C#) — explicitly hides an inherited member.
    pub new_: bool,
    /// `required` (C# 11) — must be assigned during object init.
    pub required: bool,
    /// `final` (Java field/method/class) — renders as `<final/>`
    /// marker. Distinct from C#'s `readonly` (same semantics, different
    /// wire name) so each language's tests see the marker the
    /// imperative pipeline produced.
    pub final_: bool,
    /// `synchronized` (Java method) — renders as `<synchronized/>`.
    pub synchronized_: bool,
    /// `transient` (Java field) — exclude from serialization.
    pub transient: bool,
    /// `native` (Java method) — implementation supplied by the JVM.
    pub native: bool,
    /// `strictfp` (Java) — strict floating-point.
    pub strictfp: bool,
    /// `default` (Java interface method) — has a default body.
    pub default: bool,
    /// `get` (TypeScript / JS class accessor) — `get foo() {...}` —
    /// renders as `<get/>` marker on the method.
    pub getter: bool,
    /// `set` (TypeScript / JS class accessor) — `set foo(v) {...}` —
    /// renders as `<set/>` marker on the method.
    pub setter: bool,
    /// `*` (TypeScript / JS generator function) — renders as
    /// `<generator/>` marker on the method/function.
    pub generator: bool,
}

impl Modifiers {
    /// True iff no modifier is set (no access, no flags). Renders as
    /// no markers at all.
    pub fn is_empty(&self) -> bool {
        self.access.is_none()
            && !self.static_ && !self.abstract_ && !self.sealed
            && !self.virtual_ && !self.override_ && !self.readonly
            && !self.partial && !self.async_ && !self.const_
            && !self.extern_ && !self.unsafe_ && !self.volatile
            && !self.new_ && !self.required
    }

    /// Marker names this modifier set should emit, in stable order
    /// (access first, then alphabetical-ish for predictability).
    /// Used by the renderer to produce zero-width markers.
    pub fn marker_names(&self) -> Vec<&'static str> {
        let mut names: Vec<&'static str> = Vec::new();
        if let Some(a) = self.access {
            for n in a.marker_names() { names.push(n); }
        }
        // Source order — matches Java/C# canonical declaration:
        // access  abstract  static  final/readonly  ... .
        // Tests assert ordinal positions (`*[1][self::public]`,
        // `*[2][self::abstract]`), so the order is observable.
        if self.abstract_ { names.push("abstract"); }
        if self.static_   { names.push("static"); }
        if self.virtual_  { names.push("virtual"); }
        if self.override_ { names.push("override"); }
        if self.sealed    { names.push("sealed"); }
        if self.final_    { names.push("final"); }
        if self.readonly  { names.push("readonly"); }
        if self.partial   { names.push("partial"); }
        if self.async_    { names.push("async"); }
        if self.const_    { names.push("const"); }
        if self.extern_   { names.push("extern"); }
        if self.unsafe_   { names.push("unsafe"); }
        if self.volatile  { names.push("volatile"); }
        if self.new_      { names.push("new"); }
        if self.required  { names.push("required"); }
        if self.synchronized_ { names.push("synchronized"); }
        if self.transient { names.push("transient"); }
        if self.native    { names.push("native"); }
        if self.strictfp  { names.push("strictfp"); }
        if self.default   { names.push("default"); }
        if self.getter    { names.push("get"); }
        if self.setter    { names.push("set"); }
        if self.generator { names.push("generator"); }
        names
    }

    /// Flip a modifier flag from text input. Returns Err for unknown
    /// names. Used by the eventual `tractor modify --set foo=true`
    /// CLI surface.
    pub fn set_flag(&mut self, name: &str, value: bool) -> Result<(), &'static str> {
        match name {
            "static"   => self.static_ = value,
            "abstract" => self.abstract_ = value,
            "sealed"   => self.sealed = value,
            "virtual"  => self.virtual_ = value,
            "override" => self.override_ = value,
            "readonly" => self.readonly = value,
            "partial"  => self.partial = value,
            "async"    => self.async_ = value,
            "const"    => self.const_ = value,
            "extern"   => self.extern_ = value,
            "unsafe"   => self.unsafe_ = value,
            "volatile" => self.volatile = value,
            "new"      => self.new_ = value,
            "required" => self.required = value,
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

/// One step in an [`SyntaxTree::Access`] chain.
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

/// Receiver of an [`SyntaxTree::Access`] chain. Distinguishes the four
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
        let range = self.range();
        let span = self.span();
        SyntaxTree::Expression {
            inner: Box::new(self),
            marker: None,
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
            extra_markers: &[],
            children: vec![self],
            range,
            span,
        }
    }

    /// Source span of this node. Used for XML attribute emission.
    pub fn span(&self) -> Span {
        match self {
            SyntaxTree::Module { span, .. }
            | SyntaxTree::Expression { span, .. }
            | SyntaxTree::Access { span, .. }
            | SyntaxTree::Call { span, .. }
            | SyntaxTree::Binary { span, .. }
            | SyntaxTree::Unary { span, .. }
            | SyntaxTree::Tuple { span, .. }
            | SyntaxTree::List { span, .. }
            | SyntaxTree::Set { span, .. }
            | SyntaxTree::Dictionary { span, .. }
            | SyntaxTree::Pair { span, .. }
            | SyntaxTree::GenericType { span, .. }
            | SyntaxTree::Comparison { span, .. }
            | SyntaxTree::If { span, .. }
            | SyntaxTree::ElseIf { span, .. }
            | SyntaxTree::Else { span, .. }
            | SyntaxTree::For { span, .. }
            | SyntaxTree::Foreach { span, .. }
            | SyntaxTree::CFor { span, .. }
            | SyntaxTree::DoWhile { span, .. }
            | SyntaxTree::While { span, .. }
            | SyntaxTree::Break { span, .. }
            | SyntaxTree::Continue { span, .. }
            | SyntaxTree::Lambda { span, .. }
            | SyntaxTree::ObjectCreation { span, .. }
            | SyntaxTree::Ternary { span, .. }
            | SyntaxTree::FieldWrap { span, .. }
            | SyntaxTree::SimpleStatement { span, .. }
            | SyntaxTree::Try { span, .. }
            | SyntaxTree::ExceptHandler { span, .. }
            | SyntaxTree::TypeAlias { span, .. }
            | SyntaxTree::KeywordArgument { span, .. }
            | SyntaxTree::ListSplat { span, .. }
            | SyntaxTree::DictSplat { span, .. }
            | SyntaxTree::Function { span, .. }
            | SyntaxTree::Class { span, .. }
            | SyntaxTree::Body { span, .. }
            | SyntaxTree::Parameter { span, .. }
            | SyntaxTree::Skip { span, .. }
            | SyntaxTree::PositionalSeparator { span, .. }
            | SyntaxTree::KeywordSeparator { span, .. }
            | SyntaxTree::Decorator { span, .. }
            | SyntaxTree::Returns { span, .. }
            | SyntaxTree::Generic { span, .. }
            | SyntaxTree::TypeParameter { span, .. }
            | SyntaxTree::Return { span, .. }
            | SyntaxTree::Comment { span, .. }
            | SyntaxTree::Assign { span, .. }
            | SyntaxTree::Import { span, .. }
            | SyntaxTree::From { span, .. }
            | SyntaxTree::FromImport { span, .. }
            | SyntaxTree::Path { span, .. }
            | SyntaxTree::Aliased { span, .. }
            | SyntaxTree::Name { span, .. }
            | SyntaxTree::Int { span, .. }
            | SyntaxTree::Float { span, .. }
            | SyntaxTree::String { span, .. }
            | SyntaxTree::True { span, .. }
            | SyntaxTree::False { span, .. }
            | SyntaxTree::None { span, .. }
            | SyntaxTree::Atom { span, .. }
            | SyntaxTree::Enum { span, .. }
            | SyntaxTree::EnumMember { span, .. }
            | SyntaxTree::Property { span, .. }
            | SyntaxTree::Accessor { span, .. }
            | SyntaxTree::Constructor { span, .. }
            | SyntaxTree::Using { span, .. }
            | SyntaxTree::Namespace { span, .. }
            | SyntaxTree::Variable { span, .. }
            | SyntaxTree::Is { span, .. }
            | SyntaxTree::Cast { span, .. }
            | SyntaxTree::Null { span, .. }
            | SyntaxTree::Inline { span, .. }
            | SyntaxTree::Unknown { span, .. }
            | SyntaxTree::Raw { span, .. } => *span,
        }
    }

    /// Source byte range of this node. Used for verbatim-source
    /// recovery (`source[range]`) and for gap-text computation in the
    /// renderer.
    pub fn range(&self) -> ByteRange {
        match self {
            SyntaxTree::Module { range, .. }
            | SyntaxTree::Expression { range, .. }
            | SyntaxTree::Access { range, .. }
            | SyntaxTree::Call { range, .. }
            | SyntaxTree::Binary { range, .. }
            | SyntaxTree::Unary { range, .. }
            | SyntaxTree::Tuple { range, .. }
            | SyntaxTree::List { range, .. }
            | SyntaxTree::Set { range, .. }
            | SyntaxTree::Dictionary { range, .. }
            | SyntaxTree::Pair { range, .. }
            | SyntaxTree::GenericType { range, .. }
            | SyntaxTree::Comparison { range, .. }
            | SyntaxTree::If { range, .. }
            | SyntaxTree::ElseIf { range, .. }
            | SyntaxTree::Else { range, .. }
            | SyntaxTree::For { range, .. }
            | SyntaxTree::Foreach { range, .. }
            | SyntaxTree::CFor { range, .. }
            | SyntaxTree::DoWhile { range, .. }
            | SyntaxTree::While { range, .. }
            | SyntaxTree::Break { range, .. }
            | SyntaxTree::Continue { range, .. }
            | SyntaxTree::Lambda { range, .. }
            | SyntaxTree::ObjectCreation { range, .. }
            | SyntaxTree::Ternary { range, .. }
            | SyntaxTree::FieldWrap { range, .. }
            | SyntaxTree::SimpleStatement { range, .. }
            | SyntaxTree::Try { range, .. }
            | SyntaxTree::ExceptHandler { range, .. }
            | SyntaxTree::TypeAlias { range, .. }
            | SyntaxTree::KeywordArgument { range, .. }
            | SyntaxTree::ListSplat { range, .. }
            | SyntaxTree::DictSplat { range, .. }
            | SyntaxTree::Function { range, .. }
            | SyntaxTree::Class { range, .. }
            | SyntaxTree::Body { range, .. }
            | SyntaxTree::Parameter { range, .. }
            | SyntaxTree::Skip { range, .. }
            | SyntaxTree::PositionalSeparator { range, .. }
            | SyntaxTree::KeywordSeparator { range, .. }
            | SyntaxTree::Decorator { range, .. }
            | SyntaxTree::Returns { range, .. }
            | SyntaxTree::Generic { range, .. }
            | SyntaxTree::TypeParameter { range, .. }
            | SyntaxTree::Return { range, .. }
            | SyntaxTree::Comment { range, .. }
            | SyntaxTree::Assign { range, .. }
            | SyntaxTree::Import { range, .. }
            | SyntaxTree::From { range, .. }
            | SyntaxTree::FromImport { range, .. }
            | SyntaxTree::Path { range, .. }
            | SyntaxTree::Aliased { range, .. }
            | SyntaxTree::Name { range, .. }
            | SyntaxTree::Int { range, .. }
            | SyntaxTree::Float { range, .. }
            | SyntaxTree::String { range, .. }
            | SyntaxTree::True { range, .. }
            | SyntaxTree::False { range, .. }
            | SyntaxTree::None { range, .. }
            | SyntaxTree::Atom { range, .. }
            | SyntaxTree::Enum { range, .. }
            | SyntaxTree::EnumMember { range, .. }
            | SyntaxTree::Property { range, .. }
            | SyntaxTree::Accessor { range, .. }
            | SyntaxTree::Constructor { range, .. }
            | SyntaxTree::Using { range, .. }
            | SyntaxTree::Namespace { range, .. }
            | SyntaxTree::Variable { range, .. }
            | SyntaxTree::Is { range, .. }
            | SyntaxTree::Cast { range, .. }
            | SyntaxTree::Null { range, .. }
            | SyntaxTree::Inline { range, .. }
            | SyntaxTree::Unknown { range, .. }
            | SyntaxTree::Raw { range, .. } => *range,
        }
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
    pub fn scalar_text(&self) -> Option<&str> {
        match self {
            SyntaxTree::Name { text, .. }
            | SyntaxTree::Int { text, .. }
            | SyntaxTree::Float { text, .. }
            | SyntaxTree::String { text, .. }
            | SyntaxTree::True { text, .. }
            | SyntaxTree::False { text, .. }
            | SyntaxTree::None { text, .. }
            | SyntaxTree::Atom { text, .. }
            | SyntaxTree::Null { text, .. } => Some(text.as_str()),
            _ => None,
        }
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
    /// Direct tree children, in source order. Excludes synthetic
    /// render-time wrappers (`<value>`, `<type>`, `<left>`/`<right>`,
    /// `<expression>` host) and modifier markers — those are
    /// rendering metadata, not tree.
    ///
    /// **Internal walker helper, not a public API contract.**
    /// `pub(crate)` until a downstream consumer demands stability.
    /// rustc's HIR uses per-kind `Visitor` methods for this reason —
    /// keeping this internal lets us refactor freely.
    ///
    /// ## What's included
    /// - `Box<SyntaxTree>`, `Vec<SyntaxTree>`, `Option<Box<SyntaxTree>>` fields.
    /// - `AccessSegment` children of `SyntaxTree::Access` (member's name is
    ///   not an SyntaxTree; index/call have inner SyntaxTree children).
    ///
    /// ## What's excluded
    /// - Markers / modifiers (flags, not children).
    /// - Operator text + marker (`op_text`, `op_marker`, `op_range`).
    /// - Static field discriminators (`kind: &'static str` on
    ///   `Accessor`, etc.).
    /// - Comment leading flag, Body pass_only flag.
    pub(crate) fn children(&self) -> Vec<&SyntaxTree> {
        let mut v: Vec<&SyntaxTree> = Vec::new();
        match self {
            SyntaxTree::Module { children, .. } => v.extend(children.iter()),
            SyntaxTree::Expression { inner, .. } => v.push(inner),
            SyntaxTree::Access { receiver, segments, .. } => {
                if let AccessReceiver::Instance(t) = receiver {
                    v.push(t);
                }
                // keyword receivers (Base/This/Super/Self_) are leaves
                // with no SyntaxTree children
                for s in segments {
                    match s {
                        AccessSegment::Member { .. } => {} // property is not an SyntaxTree
                        AccessSegment::Index { indices, .. } => v.extend(indices.iter()),
                        AccessSegment::Call { arguments, .. } => v.extend(arguments.iter()),
                    }
                }
            }
            SyntaxTree::Call { callee, arguments, .. } => {
                v.push(callee);
                v.extend(arguments.iter());
            }
            SyntaxTree::Binary { left, right, .. }
            | SyntaxTree::Comparison { left, right, .. } => {
                v.push(left);
                v.push(right);
            }
            SyntaxTree::Unary { operand, .. } => v.push(operand),
            SyntaxTree::If { condition, body, else_branch, .. }
            | SyntaxTree::ElseIf { condition, body, else_branch, .. } => {
                v.push(condition);
                v.push(body);
                if let Some(e) = else_branch { v.push(e); }
            }
            SyntaxTree::Else { body, .. } => v.push(body),
            SyntaxTree::For { targets, iterables, body, else_body, .. } => {
                v.extend(targets.iter());
                v.extend(iterables.iter());
                v.push(body);
                if let Some(e) = else_body { v.push(e); }
            }
            SyntaxTree::While { condition, body, else_body, .. } => {
                v.push(condition);
                v.push(body);
                if let Some(e) = else_body { v.push(e); }
            }
            SyntaxTree::Foreach { type_ann, target, iterable, body, .. } => {
                if let Some(t) = type_ann { v.push(t); }
                v.push(target);
                v.push(iterable);
                v.push(body);
            }
            SyntaxTree::CFor { initializer, condition, updates, body, .. } => {
                if let Some(i) = initializer { v.push(i); }
                if let Some(c) = condition { v.push(c); }
                v.extend(updates.iter());
                v.push(body);
            }
            SyntaxTree::DoWhile { body, condition, .. } => {
                v.push(body);
                v.push(condition);
            }
            SyntaxTree::Lambda { parameters, body, .. } => {
                v.extend(parameters.iter());
                v.push(body.inner());
            }
            SyntaxTree::ObjectCreation { type_target, arguments, initializer, .. } => {
                if let Some(t) = type_target { v.push(t); }
                v.extend(arguments.iter());
                if let Some(i) = initializer { v.push(i); }
            }
            SyntaxTree::Ternary { condition, if_true, if_false, .. } => {
                v.push(condition);
                v.push(if_true);
                v.push(if_false);
            }
            SyntaxTree::FieldWrap { inner, .. } => v.push(inner),
            SyntaxTree::SimpleStatement { children, .. } => v.extend(children.iter()),
            SyntaxTree::Try { try_body, handlers, else_body, finally_body, .. } => {
                v.push(try_body);
                v.extend(handlers.iter());
                if let Some(e) = else_body { v.push(e); }
                if let Some(f) = finally_body { v.push(f); }
            }
            SyntaxTree::ExceptHandler { type_target, binding, filter, body, .. } => {
                if let Some(t) = type_target { v.push(t); }
                if let Some(b) = binding { v.push(b); }
                if let Some(f) = filter { v.push(f); }
                v.push(body);
            }
            SyntaxTree::TypeAlias { name, type_params, value, .. } => {
                v.push(name);
                if let Some(p) = type_params { v.push(p); }
                v.push(value);
            }
            SyntaxTree::KeywordArgument { name, value, .. } => {
                v.push(name);
                v.push(value);
            }
            SyntaxTree::ListSplat { inner, .. } => v.push(inner),
            SyntaxTree::DictSplat { inner, .. } => v.push(inner),
            SyntaxTree::Function { decorators, name, generics, parameters, returns, throws, body, .. } => {
                v.extend(decorators.iter());
                v.push(name);
                v.extend(generics.iter());
                v.extend(parameters.iter());
                if let Some(r) = returns { v.push(r); }
                v.extend(throws.iter());
                if let Some(b) = body { v.push(b); }
            }
            SyntaxTree::Class { decorators, name, generics, bases, where_clauses, body, .. } => {
                v.extend(decorators.iter());
                v.push(name);
                v.extend(generics.iter());
                v.extend(bases.iter());
                v.extend(where_clauses.iter());
                v.push(body);
            }
            SyntaxTree::Body { children, .. } => v.extend(children.iter()),
            SyntaxTree::Parameter { name, type_ann, default, .. } => {
                v.push(name);
                if let Some(t) = type_ann { v.push(t); }
                if let Some(d) = default { v.push(d); }
            }
            SyntaxTree::Decorator { inner, .. } => v.push(inner),
            SyntaxTree::Returns { type_ann, .. } => v.push(type_ann),
            SyntaxTree::Generic { items, .. } => v.extend(items.iter()),
            SyntaxTree::TypeParameter { name, constraint, .. } => {
                v.push(name);
                if let Some(c) = constraint { v.push(c); }
            }
            SyntaxTree::Return { value, .. } => {
                if let Some(val) = value { v.push(val); }
            }
            SyntaxTree::Assign { targets, type_annotation, values, .. } => {
                v.extend(targets.iter());
                if let Some(t) = type_annotation { v.push(t); }
                v.extend(values.iter());
            }
            SyntaxTree::Import { children, .. } => v.extend(children.iter()),
            SyntaxTree::From { path, imports, .. } => {
                if let Some(p) = path { v.push(p); }
                v.extend(imports.iter());
            }
            SyntaxTree::FromImport { name, alias, .. } => {
                v.push(name);
                if let Some(a) = alias { v.push(a); }
            }
            SyntaxTree::Path { segments, .. } => v.extend(segments.iter()),
            SyntaxTree::Aliased { inner, .. } => v.push(inner),
            SyntaxTree::Tuple { children, .. }
            | SyntaxTree::List { children, .. }
            | SyntaxTree::Set { children, .. } => v.extend(children.iter()),
            SyntaxTree::Dictionary { pairs, .. } => v.extend(pairs.iter()),
            SyntaxTree::Pair { key, value, .. } => {
                v.push(key);
                v.push(value);
            }
            SyntaxTree::GenericType { name, params, .. } => {
                v.push(name);
                v.extend(params.iter());
            }
            SyntaxTree::Is { value, type_target, .. } => {
                v.push(value);
                v.push(type_target);
            }
            SyntaxTree::Cast { type_ann, value, .. } => {
                v.push(type_ann);
                v.push(value);
            }
            SyntaxTree::Enum { decorators, name, underlying_type, members, .. } => {
                v.extend(decorators.iter());
                v.push(name);
                if let Some(t) = underlying_type { v.push(t); }
                v.extend(members.iter());
            }
            SyntaxTree::EnumMember { decorators, name, value, .. } => {
                v.extend(decorators.iter());
                v.push(name);
                if let Some(val) = value { v.push(val); }
            }
            SyntaxTree::Property { decorators, type_ann, name, accessors, value, .. } => {
                v.extend(decorators.iter());
                if let Some(t) = type_ann { v.push(t); }
                v.push(name);
                v.extend(accessors.iter());
                if let Some(val) = value { v.push(val); }
            }
            SyntaxTree::Accessor { body, .. } => {
                if let Some(b) = body { v.push(b); }
            }
            SyntaxTree::Constructor { decorators, name, parameters, body, .. } => {
                v.extend(decorators.iter());
                v.push(name);
                v.extend(parameters.iter());
                v.push(body);
            }
            SyntaxTree::Using { alias, path, .. } => {
                v.push(path);
                if let Some(a) = alias { v.push(a); }
            }
            SyntaxTree::Namespace { name, children, file_scoped: _, .. } => {
                v.push(name);
                v.extend(children.iter());
            }
            SyntaxTree::Variable { decorators, type_ann, name, value, .. } => {
                v.extend(decorators.iter());
                if let Some(t) = type_ann { v.push(t); }
                v.push(name);
                if let Some(val) = value { v.push(&val.inner); }
            }
            SyntaxTree::Inline { children, .. } => v.extend(children.iter()),
            SyntaxTree::Raw { children, .. } => v.extend(children.iter()),
            // Leaves and markers — no SyntaxTree children.
            SyntaxTree::Name { .. } | SyntaxTree::Int { .. } | SyntaxTree::Float { .. } | SyntaxTree::String { .. }
            | SyntaxTree::True { .. } | SyntaxTree::False { .. } | SyntaxTree::None { .. } | SyntaxTree::Null { .. }
            | SyntaxTree::Atom { .. }
            | SyntaxTree::Skip { .. }
            | SyntaxTree::Comment { .. } | SyntaxTree::PositionalSeparator { .. }
            | SyntaxTree::KeywordSeparator { .. } | SyntaxTree::Break { .. } | SyntaxTree::Continue { .. }
            | SyntaxTree::Unknown { .. } => {}
        }
        // Sort by source order so consumers (renderer, audit walker)
        // don't have to repeat. Variants whose fields are already in
        // source order pay a near-zero sort cost.
        v.sort_by_key(|c| c.range().start);
        v
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

    fn span_mut(&mut self) -> &mut Span {
        match self {
            SyntaxTree::Module { span, .. }
            | SyntaxTree::Expression { span, .. }
            | SyntaxTree::Access { span, .. }
            | SyntaxTree::Call { span, .. }
            | SyntaxTree::Binary { span, .. }
            | SyntaxTree::Unary { span, .. }
            | SyntaxTree::Tuple { span, .. }
            | SyntaxTree::List { span, .. }
            | SyntaxTree::Set { span, .. }
            | SyntaxTree::Dictionary { span, .. }
            | SyntaxTree::Pair { span, .. }
            | SyntaxTree::GenericType { span, .. }
            | SyntaxTree::Comparison { span, .. }
            | SyntaxTree::If { span, .. }
            | SyntaxTree::ElseIf { span, .. }
            | SyntaxTree::Else { span, .. }
            | SyntaxTree::For { span, .. }
            | SyntaxTree::Foreach { span, .. }
            | SyntaxTree::CFor { span, .. }
            | SyntaxTree::DoWhile { span, .. }
            | SyntaxTree::While { span, .. }
            | SyntaxTree::Break { span, .. }
            | SyntaxTree::Continue { span, .. }
            | SyntaxTree::Lambda { span, .. }
            | SyntaxTree::ObjectCreation { span, .. }
            | SyntaxTree::Ternary { span, .. }
            | SyntaxTree::FieldWrap { span, .. }
            | SyntaxTree::SimpleStatement { span, .. }
            | SyntaxTree::Try { span, .. }
            | SyntaxTree::ExceptHandler { span, .. }
            | SyntaxTree::TypeAlias { span, .. }
            | SyntaxTree::KeywordArgument { span, .. }
            | SyntaxTree::ListSplat { span, .. }
            | SyntaxTree::DictSplat { span, .. }
            | SyntaxTree::Function { span, .. }
            | SyntaxTree::Class { span, .. }
            | SyntaxTree::Body { span, .. }
            | SyntaxTree::Parameter { span, .. }
            | SyntaxTree::Skip { span, .. }
            | SyntaxTree::PositionalSeparator { span, .. }
            | SyntaxTree::KeywordSeparator { span, .. }
            | SyntaxTree::Decorator { span, .. }
            | SyntaxTree::Returns { span, .. }
            | SyntaxTree::Generic { span, .. }
            | SyntaxTree::TypeParameter { span, .. }
            | SyntaxTree::Return { span, .. }
            | SyntaxTree::Comment { span, .. }
            | SyntaxTree::Assign { span, .. }
            | SyntaxTree::Import { span, .. }
            | SyntaxTree::From { span, .. }
            | SyntaxTree::FromImport { span, .. }
            | SyntaxTree::Path { span, .. }
            | SyntaxTree::Aliased { span, .. }
            | SyntaxTree::Name { span, .. }
            | SyntaxTree::Int { span, .. }
            | SyntaxTree::Float { span, .. }
            | SyntaxTree::String { span, .. }
            | SyntaxTree::True { span, .. }
            | SyntaxTree::False { span, .. }
            | SyntaxTree::None { span, .. }
            | SyntaxTree::Atom { span, .. }
            | SyntaxTree::Enum { span, .. }
            | SyntaxTree::EnumMember { span, .. }
            | SyntaxTree::Property { span, .. }
            | SyntaxTree::Accessor { span, .. }
            | SyntaxTree::Constructor { span, .. }
            | SyntaxTree::Using { span, .. }
            | SyntaxTree::Namespace { span, .. }
            | SyntaxTree::Variable { span, .. }
            | SyntaxTree::Is { span, .. }
            | SyntaxTree::Cast { span, .. }
            | SyntaxTree::Null { span, .. }
            | SyntaxTree::Inline { span, .. }
            | SyntaxTree::Unknown { span, .. }
            | SyntaxTree::Raw { span, .. } => span,
        }
    }

    fn children(&self) -> Vec<&Self> { self.children() }

    fn children_mut(&mut self) -> Vec<&mut Self> {
        let mut v: Vec<&mut SyntaxTree> = Vec::new();
        match self {
            SyntaxTree::Module { children, .. } => v.extend(children.iter_mut()),
            SyntaxTree::Expression { inner, .. } => v.push(inner.as_mut()),
            SyntaxTree::Access { receiver, segments, .. } => {
                if let AccessReceiver::Instance(t) = receiver {
                    v.push(t.as_mut());
                }
                for s in segments.iter_mut() {
                    match s {
                        AccessSegment::Member { .. } => {}
                        AccessSegment::Index { indices, .. } => v.extend(indices.iter_mut()),
                        AccessSegment::Call { arguments, .. } => v.extend(arguments.iter_mut()),
                    }
                }
            }
            SyntaxTree::Call { callee, arguments, .. } => {
                v.push(callee.as_mut());
                v.extend(arguments.iter_mut());
            }
            SyntaxTree::Binary { left, right, .. }
            | SyntaxTree::Comparison { left, right, .. } => {
                v.push(left.as_mut());
                v.push(right.as_mut());
            }
            SyntaxTree::Unary { operand, .. } => v.push(operand.as_mut()),
            SyntaxTree::If { condition, body, else_branch, .. }
            | SyntaxTree::ElseIf { condition, body, else_branch, .. } => {
                v.push(condition.as_mut());
                v.push(body.as_mut());
                if let Some(e) = else_branch { v.push(e.as_mut()); }
            }
            SyntaxTree::Else { body, .. } => v.push(body.as_mut()),
            SyntaxTree::For { targets, iterables, body, else_body, .. } => {
                v.extend(targets.iter_mut());
                v.extend(iterables.iter_mut());
                v.push(body.as_mut());
                if let Some(e) = else_body { v.push(e.as_mut()); }
            }
            SyntaxTree::While { condition, body, else_body, .. } => {
                v.push(condition.as_mut());
                v.push(body.as_mut());
                if let Some(e) = else_body { v.push(e.as_mut()); }
            }
            SyntaxTree::Foreach { type_ann, target, iterable, body, .. } => {
                if let Some(t) = type_ann { v.push(t.as_mut()); }
                v.push(target.as_mut());
                v.push(iterable.as_mut());
                v.push(body.as_mut());
            }
            SyntaxTree::CFor { initializer, condition, updates, body, .. } => {
                if let Some(i) = initializer { v.push(i.as_mut()); }
                if let Some(c) = condition { v.push(c.as_mut()); }
                v.extend(updates.iter_mut());
                v.push(body.as_mut());
            }
            SyntaxTree::DoWhile { body, condition, .. } => {
                v.push(body.as_mut());
                v.push(condition.as_mut());
            }
            SyntaxTree::Lambda { parameters, body, .. } => {
                v.extend(parameters.iter_mut());
                v.push(body.inner_mut());
            }
            SyntaxTree::ObjectCreation { type_target, arguments, initializer, .. } => {
                if let Some(t) = type_target { v.push(t.as_mut()); }
                v.extend(arguments.iter_mut());
                if let Some(i) = initializer { v.push(i.as_mut()); }
            }
            SyntaxTree::Ternary { condition, if_true, if_false, .. } => {
                v.push(condition.as_mut());
                v.push(if_true.as_mut());
                v.push(if_false.as_mut());
            }
            SyntaxTree::FieldWrap { inner, .. } => v.push(inner.as_mut()),
            SyntaxTree::SimpleStatement { children, .. } => v.extend(children.iter_mut()),
            SyntaxTree::Try { try_body, handlers, else_body, finally_body, .. } => {
                v.push(try_body.as_mut());
                v.extend(handlers.iter_mut());
                if let Some(e) = else_body { v.push(e.as_mut()); }
                if let Some(f) = finally_body { v.push(f.as_mut()); }
            }
            SyntaxTree::ExceptHandler { type_target, binding, filter, body, .. } => {
                if let Some(t) = type_target { v.push(t.as_mut()); }
                if let Some(b) = binding { v.push(b.as_mut()); }
                if let Some(f) = filter { v.push(f.as_mut()); }
                v.push(body.as_mut());
            }
            SyntaxTree::TypeAlias { name, type_params, value, .. } => {
                v.push(name.as_mut());
                if let Some(p) = type_params { v.push(p.as_mut()); }
                v.push(value.as_mut());
            }
            SyntaxTree::KeywordArgument { name, value, .. } => {
                v.push(name.as_mut());
                v.push(value.as_mut());
            }
            SyntaxTree::ListSplat { inner, .. } => v.push(inner.as_mut()),
            SyntaxTree::DictSplat { inner, .. } => v.push(inner.as_mut()),
            SyntaxTree::Function { decorators, name, generics, parameters, returns, body, .. } => {
                v.extend(decorators.iter_mut());
                v.push(name.as_mut());
                v.extend(generics.iter_mut());
                v.extend(parameters.iter_mut());
                if let Some(r) = returns { v.push(r.as_mut()); }
                if let Some(b) = body { v.push(b.as_mut()); }
            }
            SyntaxTree::Class { decorators, name, generics, bases, where_clauses, body, .. } => {
                v.extend(decorators.iter_mut());
                v.push(name.as_mut());
                v.extend(generics.iter_mut());
                v.extend(bases.iter_mut());
                v.extend(where_clauses.iter_mut());
                v.push(body.as_mut());
            }
            SyntaxTree::Body { children, .. } => v.extend(children.iter_mut()),
            SyntaxTree::Parameter { name, type_ann, default, .. } => {
                v.push(name.as_mut());
                if let Some(t) = type_ann { v.push(t.as_mut()); }
                if let Some(d) = default { v.push(d.as_mut()); }
            }
            SyntaxTree::Decorator { inner, .. } => v.push(inner.as_mut()),
            SyntaxTree::Returns { type_ann, .. } => v.push(type_ann.as_mut()),
            SyntaxTree::Generic { items, .. } => v.extend(items.iter_mut()),
            SyntaxTree::TypeParameter { name, constraint, .. } => {
                v.push(name.as_mut());
                if let Some(c) = constraint { v.push(c.as_mut()); }
            }
            SyntaxTree::Return { value, .. } => {
                if let Some(val) = value { v.push(val.as_mut()); }
            }
            SyntaxTree::Assign { targets, type_annotation, values, .. } => {
                v.extend(targets.iter_mut());
                if let Some(t) = type_annotation { v.push(t.as_mut()); }
                v.extend(values.iter_mut());
            }
            SyntaxTree::Import { children, .. } => v.extend(children.iter_mut()),
            SyntaxTree::From { path, imports, .. } => {
                if let Some(p) = path { v.push(p.as_mut()); }
                v.extend(imports.iter_mut());
            }
            SyntaxTree::FromImport { name, alias, .. } => {
                v.push(name.as_mut());
                if let Some(a) = alias { v.push(a.as_mut()); }
            }
            SyntaxTree::Path { segments, .. } => v.extend(segments.iter_mut()),
            SyntaxTree::Aliased { inner, .. } => v.push(inner.as_mut()),
            SyntaxTree::Tuple { children, .. }
            | SyntaxTree::List { children, .. }
            | SyntaxTree::Set { children, .. } => v.extend(children.iter_mut()),
            SyntaxTree::Dictionary { pairs, .. } => v.extend(pairs.iter_mut()),
            SyntaxTree::Pair { key, value, .. } => {
                v.push(key.as_mut());
                v.push(value.as_mut());
            }
            SyntaxTree::GenericType { name, params, .. } => {
                v.push(name.as_mut());
                v.extend(params.iter_mut());
            }
            SyntaxTree::Is { value, type_target, .. } => {
                v.push(value.as_mut());
                v.push(type_target.as_mut());
            }
            SyntaxTree::Cast { type_ann, value, .. } => {
                v.push(type_ann.as_mut());
                v.push(value.as_mut());
            }
            SyntaxTree::Enum { decorators, name, underlying_type, members, .. } => {
                v.extend(decorators.iter_mut());
                v.push(name.as_mut());
                if let Some(t) = underlying_type { v.push(t.as_mut()); }
                v.extend(members.iter_mut());
            }
            SyntaxTree::EnumMember { decorators, name, value, .. } => {
                v.extend(decorators.iter_mut());
                v.push(name.as_mut());
                if let Some(val) = value { v.push(val.as_mut()); }
            }
            SyntaxTree::Property { decorators, type_ann, name, accessors, value, .. } => {
                v.extend(decorators.iter_mut());
                if let Some(t) = type_ann { v.push(t.as_mut()); }
                v.push(name.as_mut());
                v.extend(accessors.iter_mut());
                if let Some(val) = value { v.push(val.as_mut()); }
            }
            SyntaxTree::Accessor { body, .. } => {
                if let Some(b) = body { v.push(b.as_mut()); }
            }
            SyntaxTree::Constructor { decorators, name, parameters, body, .. } => {
                v.extend(decorators.iter_mut());
                v.push(name.as_mut());
                v.extend(parameters.iter_mut());
                v.push(body.as_mut());
            }
            SyntaxTree::Using { alias, path, .. } => {
                v.push(path.as_mut());
                if let Some(a) = alias { v.push(a.as_mut()); }
            }
            SyntaxTree::Namespace { name, children, file_scoped: _, .. } => {
                v.push(name.as_mut());
                v.extend(children.iter_mut());
            }
            SyntaxTree::Variable { decorators, type_ann, name, value, .. } => {
                v.extend(decorators.iter_mut());
                if let Some(t) = type_ann { v.push(t.as_mut()); }
                v.push(name.as_mut());
                if let Some(val) = value { v.push(&mut val.inner); }
            }
            SyntaxTree::Inline { children, .. } => v.extend(children.iter_mut()),
            SyntaxTree::Raw { children, .. } => v.extend(children.iter_mut()),
            // Leaves and markers — no SyntaxTree children.
            SyntaxTree::Name { .. } | SyntaxTree::Int { .. } | SyntaxTree::Float { .. } | SyntaxTree::String { .. }
            | SyntaxTree::True { .. } | SyntaxTree::False { .. } | SyntaxTree::None { .. } | SyntaxTree::Null { .. }
            | SyntaxTree::Atom { .. }
            | SyntaxTree::Skip { .. }
            | SyntaxTree::Comment { .. } | SyntaxTree::PositionalSeparator { .. }
            | SyntaxTree::KeywordSeparator { .. } | SyntaxTree::Break { .. } | SyntaxTree::Continue { .. }
            | SyntaxTree::Unknown { .. } => {}
        }
        v.sort_by_key(|c| c.range().start);
        v
    }
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
