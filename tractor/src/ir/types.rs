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

    /// `<module>` / `<unit>` / `<program>` — top-level program. The
    /// CST root for languages that have one. Children are
    /// statement-or-declaration IR. `element_name` lets each language
    /// pick its own name to match the existing pipeline:
    /// - Python: `"module"`
    /// - C# / TypeScript: `"unit"` or `"program"` (TBD per language)
    /// - Java: `"program"`
    /// Cross-language unification of this name is a Principle #5
    /// audit candidate but requires the existing pipeline's choice
    /// per language to be revisited; we keep parity for now.
    Module {
        element_name: &'static str,
        children: Vec<Ir>,
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
        inner: Box<Ir>,
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
        /// Element name. "binary" for arithmetic / bitwise / shift /
        /// comparison; "logical" for short-circuit boolean (`and`,
        /// `or`). Lets the renderer emit the right outer element
        /// without changing shape.
        element_name: &'static str,
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

    // ----- Collections & literal containers -------------------------------

    /// `<tuple>` — `(a, b, c)` parenthesized tuple. Children are
    /// expressions in source order (no `<expression>` host —
    /// matches existing pipeline shape `<tuple><name>a</name>...</tuple>`).
    Tuple { children: Vec<Ir>, range: ByteRange, span: Span },

    /// `<list>` with `<literal/>` marker — `[a, b, c]` list literal.
    List { children: Vec<Ir>, range: ByteRange, span: Span },

    /// `<set>` with `<literal/>` marker — `{a, b}`.
    Set { children: Vec<Ir>, range: ByteRange, span: Span },

    /// `<dictionary>` with `<literal/>` marker — `{k: v, ...}`.
    Dictionary { pairs: Vec<Ir>, range: ByteRange, span: Span },

    /// `<pair>` — `key: value` inside a dictionary.
    Pair { key: Box<Ir>, value: Box<Ir>, range: ByteRange, span: Span },

    // ----- Generic types --------------------------------------------------

    /// `<type[generic]>` — `Name[T, U, ...]` generic type expression.
    /// `name` is the base type name; `params` are the type arguments.
    GenericType {
        name: Box<Ir>,
        params: Vec<Ir>,
        range: ByteRange,
        span: Span,
    },

    // ----- Comparisons ----------------------------------------------------

    /// `<binary>` for chained comparisons like `a < b < c`. tree-sitter
    /// has a dedicated `comparison_operator` kind; we model it as a
    /// binary chain. For simplicity in the experiment, we emit a
    /// binary IR with the *first* operator and concatenate the
    /// remaining as Unknown-wrapped — this works for the common
    /// two-operand case (`a < b`).
    Comparison {
        left: Box<Ir>,
        op_text: String,
        op_marker: &'static str,
        op_range: ByteRange,
        right: Box<Ir>,
        range: ByteRange,
        span: Span,
    },

    // ----- Control flow ---------------------------------------------------

    /// `<if>` — `if cond: ... [elif ...] [else ...]`.
    If {
        condition: Box<Ir>,
        body: Box<Ir>,             // Ir::Body
        else_branch: Option<Box<Ir>>, // Ir::ElseIf or Ir::Else
        range: ByteRange,
        span: Span,
    },

    /// `<else_if>` — `elif cond: body`. Used inside If's else_branch
    /// to keep elif chains flat.
    ElseIf {
        condition: Box<Ir>,
        body: Box<Ir>,
        else_branch: Option<Box<Ir>>,
        range: ByteRange,
        span: Span,
    },

    /// `<else>` — `else: body`.
    Else { body: Box<Ir>, range: ByteRange, span: Span },

    /// `<for>` — `for target in iter: body [else: body]`.
    /// `<for[async]>` adds an `<async/>` marker.
    For {
        is_async: bool,
        targets: Vec<Ir>,
        iterables: Vec<Ir>,
        body: Box<Ir>,
        else_body: Option<Box<Ir>>,
        range: ByteRange,
        span: Span,
    },

    /// `<while>` — `while cond: body [else: body]`.
    While {
        condition: Box<Ir>,
        body: Box<Ir>,
        else_body: Option<Box<Ir>>,
        range: ByteRange,
        span: Span,
    },

    /// `<foreach>` — C# `foreach (T x in coll) body` / Java
    /// enhanced-for. Single target, single iterable, optional type
    /// annotation. Distinct from [`Ir::For`] because Python's
    /// `for x in iter` (a foreach by semantics) renders as `<for>`
    /// for parity with the existing pipeline; cross-language element
    /// naming asymmetry is allowed (Principle #5 scope is intra-
    /// language).
    Foreach {
        type_ann: Option<Box<Ir>>,
        target: Box<Ir>,
        iterable: Box<Ir>,
        body: Box<Ir>,
        range: ByteRange,
        span: Span,
    },

    /// `<for>` — C-style `for (init; cond; update) body` (C#, Java,
    /// JS, …). All three header parts are optional. `updates` is a
    /// vec because C-style `for` allows comma-separated updates
    /// (`for(int i=0,j=10; i<j; i++,j--)`).
    CFor {
        initializer: Option<Box<Ir>>,
        condition: Option<Box<Ir>>,
        updates: Vec<Ir>,
        body: Box<Ir>,
        range: ByteRange,
        span: Span,
    },

    /// `<do>` — `do body while(cond);`. Renders the keyword as gap
    /// text; body and condition are the only IR children.
    DoWhile {
        body: Box<Ir>,
        condition: Box<Ir>,
        range: ByteRange,
        span: Span,
    },

    /// `<break>` / `<continue>` markers.
    Break { range: ByteRange, span: Span },
    Continue { range: ByteRange, span: Span },

    /// Wrap an inner IR node in a single element. Used as the
    /// parity-track field-wrapping mechanism: when a CST child has
    /// a labelled `field=type` (or `name`, `value`, etc.) and that
    /// field has a wrapping in the language's table, lower it as
    /// `Ir::FieldWrap { wrapper: "type", inner: ... }` so the
    /// rendered XML is `<type>{inner rendering}</type>`.
    FieldWrap {
        wrapper: &'static str,
        inner: Box<Ir>,
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
        children: Vec<Ir>,
        range: ByteRange,
        span: Span,
    },

    /// `<try>` — `try { body } catch (...) { ... } finally { ... }`
    /// (C# / Java) or `try: ... except E: ... else: ... finally: ...`
    /// (Python). Shared cross-language. `try_body` is the protected
    /// block; `handlers` are catch/except clauses; `else_body` runs
    /// when no exception (Python only); `finally_body` always runs.
    Try {
        try_body: Box<Ir>,
        handlers: Vec<Ir>,
        else_body: Option<Box<Ir>>,
        finally_body: Option<Box<Ir>>,
        range: ByteRange,
        span: Span,
    },

    /// `<except>` (Python) / `<catch>` (C#) — single exception handler.
    /// `type_target` is the exception type; `binding` is the variable
    /// (`as e` / `Exception ex`); `filter` is C#'s `when (cond)`;
    /// `body` is the handler block.
    ExceptHandler {
        kind: &'static str,            // "except" | "catch"
        type_target: Option<Box<Ir>>,
        binding: Option<Box<Ir>>,
        filter: Option<Box<Ir>>,
        body: Box<Ir>,
        range: ByteRange,
        span: Span,
    },

    /// `<alias>` — Python 3.12 `type Foo = Bar` /
    /// `type Foo[T] = Bar`. `name` is the alias being declared,
    /// `type_params` is the optional generic list, `value` is the
    /// aliased type.
    TypeAlias {
        name: Box<Ir>,
        type_params: Option<Box<Ir>>,
        value: Box<Ir>,
        range: ByteRange,
        span: Span,
    },

    /// `<keyword>` — `name=value` keyword argument in a call (Python /
    /// C# named arg). `value` is the inner expression.
    KeywordArgument {
        name: Box<Ir>,
        value: Box<Ir>,
        range: ByteRange,
        span: Span,
    },

    /// `<splat>` with `<list/>` marker — `*x` (positional splat) in a
    /// call or list literal. Inner is the splatted expression.
    ListSplat {
        inner: Box<Ir>,
        range: ByteRange,
        span: Span,
    },

    /// `<splat>` with `<dict/>` marker — `**x` (keyword splat) in a
    /// call or dict literal.
    DictSplat {
        inner: Box<Ir>,
        range: ByteRange,
        span: Span,
    },

    /// `<ternary>` — `cond ? a : b` (C# / Java / JS) or
    /// `a if cond else b` (Python). Renders with logical slots
    /// regardless of source order; the renderer sorts children by
    /// `range().start` to weave gap text correctly.
    Ternary {
        condition: Box<Ir>,
        if_true: Box<Ir>,
        if_false: Box<Ir>,
        range: ByteRange,
        span: Span,
    },

    /// `<new>` — `new Foo(args) { Init }` (C# / Java
     /// `new`-expression). `type_target` is `None` for C#'s
    /// target-typed `new()` form. `initializer` carries an
    /// `Ir::Inline` of the brace-form initializer's children
    /// (`{ A = 1, B = 2 }`) when present.
    ObjectCreation {
        type_target: Option<Box<Ir>>,
        arguments: Vec<Ir>,
        initializer: Option<Box<Ir>>,
        range: ByteRange,
        span: Span,
    },

    /// `<lambda>` — `x => x*x`, `(x, y) => x+y`, `async x => ...`,
    /// `(x) => { return x; }`. Cross-language: C# lambda, Java
    /// lambda (`x -> x`), Python `lambda` (which has bare-param
    /// syntax). `body` is `Ir::Body` for block-bodied lambdas
    /// (renders `<body>`) or any expression IR for expression-bodied
    /// (renders `<value><expression>...</expression></value>`).
    Lambda {
        modifiers: Modifiers,
        parameters: Vec<Ir>,
        body: Box<Ir>,
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
        decorators: Vec<Ir>,
        name: Box<Ir>,                  // Ir::Name
        generics: Option<Box<Ir>>,      // Ir::Generic
        parameters: Vec<Ir>,            // each Ir::Parameter / Ir::PositionalSeparator / Ir::KeywordSeparator
        returns: Option<Box<Ir>>,       // Ir::Returns
        body: Box<Ir>,                  // Ir::Body
        range: ByteRange,
        span: Span,
    },

    /// `<class>` / `<struct>` / `<interface>` / `<record>` — type
    /// declaration. `kind` selects the element name; structurally all
    /// four shapes are the same (modifiers, decorators, name,
    /// generics, bases, body), so they share one IR variant. Python
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
        decorators: Vec<Ir>,
        name: Box<Ir>,
        generics: Option<Box<Ir>>,
        bases: Vec<Ir>,                 // each is a base expression
        body: Box<Ir>,
        range: ByteRange,
        span: Span,
    },

    /// `<body>` — a block of statements. `pass_only` adds a `<pass/>`
    /// empty marker child; visible in tree-text as `<body[pass]>`.
    Body {
        children: Vec<Ir>,
        pass_only: bool,
        range: ByteRange,
        span: Span,
    },

    /// `<parameter>` — one parameter in a function signature.
    /// `kind` controls the marker: `Regular` has none,
    /// `Args` adds `<args/>`, `Kwargs` adds `<kwargs/>`.
    /// `extra_markers` carries C#-style parameter modifiers
    /// (`<ref/>`, `<out/>`, `<in/>`, `<params/>`, `<this/>`).
    Parameter {
        kind: ParamKind,
        extra_markers: &'static [&'static str],
        name: Box<Ir>,                  // Ir::Name
        type_ann: Option<Box<Ir>>,      // <type>...</type>
        default: Option<Box<Ir>>,       // <value><expression>...</expression></value>
        range: ByteRange,
        span: Span,
    },

    /// `<positional>/</positional>` — `/` separator marking the end
    /// of positional-only parameters.
    PositionalSeparator { range: ByteRange, span: Span },

    /// `<keyword>*</keyword>` — `*` separator marking the start of
    /// keyword-only parameters.
    KeywordSeparator { range: ByteRange, span: Span },

    /// `<decorator>` — `@expr` decorator above a function/class.
    /// Wraps any expression directly (no `<expression>` host).
    Decorator {
        inner: Box<Ir>,
        range: ByteRange,
        span: Span,
    },

    /// `<returns>` — return-type annotation slot. Wraps a `<type>`.
    Returns {
        type_ann: Box<Ir>,
        range: ByteRange,
        span: Span,
    },

    /// `<generic>` — generic-parameter list (PEP 695 `def f[T]`).
    /// Each item is an [`Ir::TypeParameter`] (renders as `<type>`
    /// containing a `<name>`).
    Generic {
        items: Vec<Ir>,
        range: ByteRange,
        span: Span,
    },

    /// `<type>` — type-parameter slot inside `<generic>`. Has a name
    /// and optional constraint.
    TypeParameter {
        name: Box<Ir>,
        constraint: Option<Box<Ir>>,
        range: ByteRange,
        span: Span,
    },

    /// `<return>` — `return <value>?` statement. `value` is `None`
    /// for bare `return`. Renders as
    /// `<return><expression>...</expression></return>` when value is
    /// present.
    Return {
        value: Option<Box<Ir>>,
        range: ByteRange,
        span: Span,
    },

    /// `<comment>text</comment>` — standalone source comment.
    /// `leading` adds a `<leading/>` marker (`comment[leading]`); the
    /// existing pipeline classifies comments by adjacency to the next
    /// declaration.
    Comment {
        leading: bool,
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
    /// `<enum>` — `enum Name { Member1, Member2 = 5, ... }`. Members
    /// are `Ir::EnumMember`. C# enums also accept an optional
    /// underlying type (`enum Trait : uint`).
    Enum {
        modifiers: Modifiers,
        decorators: Vec<Ir>,
        name: Box<Ir>,
        underlying_type: Option<Box<Ir>>,  // C# `: uint`
        members: Vec<Ir>,
        range: ByteRange,
        span: Span,
    },

    /// `<constant>` — one member of an enum (`Low`, `Medium = 5`).
    EnumMember {
        decorators: Vec<Ir>,
        name: Box<Ir>,
        value: Option<Box<Ir>>,
        range: ByteRange,
        span: Span,
    },

    /// `<property>` — C# `public int X { get; set; } = init;` and
    /// the various property forms. Accessors: getter / setter /
    /// init. Renders
    /// `<property>{markers}<type>...<name>...{accessors}{value}</property>`.
    Property {
        modifiers: Modifiers,
        decorators: Vec<Ir>,
        type_ann: Option<Box<Ir>>,
        name: Box<Ir>,
        accessors: Vec<Ir>,                // each Ir::Accessor
        value: Option<Box<Ir>>,            // initializer expression
        range: ByteRange,
        span: Span,
    },

    /// `<accessor>` — one of `get`, `set`, `init` inside a property's
    /// `{ ... }`. Body is optional (auto-implemented properties have
    /// no body).
    Accessor {
        modifiers: Modifiers,              // Some accessors have their own access modifier
        kind: &'static str,                // "get" | "set" | "init"
        body: Option<Box<Ir>>,
        range: ByteRange,
        span: Span,
    },

    /// `<constructor>` — C# constructor (`public Foo(...) : base(x) { }`).
    /// Renders similar to method but with `<constructor>` element.
    /// Initializer `: base(...)` deferred.
    Constructor {
        modifiers: Modifiers,
        decorators: Vec<Ir>,
        name: Box<Ir>,                     // class name being constructed
        parameters: Vec<Ir>,
        body: Box<Ir>,
        range: ByteRange,
        span: Span,
    },

    /// `<using>` — C#'s `using System;` / `using static System.Math;`
    /// / `using A = B;`. The IR mirrors Python's import shape but
    /// with `<using>` element name. `static_` flag for `using static`,
    /// `alias` for `using X = Y;`.
    Using {
        is_static: bool,
        alias: Option<Box<Ir>>,
        path: Box<Ir>,
        range: ByteRange,
        span: Span,
    },

    /// `<namespace>` — C#'s `namespace X { ... }` (block-scoped) or
    /// `namespace X;` (file-scoped). For now block-scoped only;
    /// renders `<namespace><name>...</name>{children...}</namespace>`.
    Namespace {
        name: Box<Ir>,
        children: Vec<Ir>,
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
        /// uses neither (assignments take a different IR path).
        element_name: &'static str,
        /// Access + flag modifiers. Empty for locals (their modifiers
        /// like `const` are very limited); fields use them fully.
        modifiers: Modifiers,
        type_ann: Option<Box<Ir>>,
        name: Box<Ir>,
        value: Option<Box<Ir>>,
        range: ByteRange,
        span: Span,
    },

    /// `<is>` — `expr is Type` type-test expression. Renders as
    /// `<is><left><expression>{value}</expression></left>
    /// <right><expression><type>{type_target}</type></expression></right></is>`.
    /// (Pattern-form `is Widget w` not yet covered — would extend
    /// `right` with a pattern variant.)
    Is {
        value: Box<Ir>,
        type_target: Box<Ir>,
        range: ByteRange,
        span: Span,
    },

    /// `<cast>` — `(Type)expr` type-cast expression (C#, Java, …).
    /// Renders as `<cast><type>...</type><value><expression>...</expression></value></cast>`.
    Cast {
        type_ann: Box<Ir>,
        value: Box<Ir>,
        range: ByteRange,
        span: Span,
    },

    /// `null` literal (C# / Java / TS / PHP). Distinct from `None`
    /// (Python) because the keyword text differs and Principle #5
    /// applies *within* a language. We may unify the *element name*
    /// at render time later if a cross-language audit decides so.
    Null   { range: ByteRange, span: Span },

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
/// `Option<Access>` on `Ir::Class` lets cross-language reuse stay
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
        if self.static_   { names.push("static"); }
        if self.abstract_ { names.push("abstract"); }
        if self.sealed    { names.push("sealed"); }
        if self.virtual_  { names.push("virtual"); }
        if self.override_ { names.push("override"); }
        if self.readonly  { names.push("readonly"); }
        if self.partial   { names.push("partial"); }
        if self.async_    { names.push("async"); }
        if self.const_    { names.push("const"); }
        if self.extern_   { names.push("extern"); }
        if self.unsafe_   { names.push("unsafe"); }
        if self.volatile  { names.push("volatile"); }
        if self.new_      { names.push("new"); }
        if self.required  { names.push("required"); }
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

/// `Ir::Parameter` kind discriminator.
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
    /// `optional: true` adds an `<optional/>` empty marker first child
    /// (visible in tree-text as `<member[optional]>`), modelling
    /// null-conditional access (`?.`) in C# / TS / Ruby etc.
    /// **Architectural note:** with this flag, conditional and regular
    /// member access produce the *same shape* differing only by the
    /// marker — exactly Principle #15. The existing C# pipeline has a
    /// deferred design problem here (`<member[conditional]>` parent +
    /// `<condition>` wrapper, see `todo/39-…md` lesson 5d); the
    /// typed-IR architecture sidesteps it by construction.
    Member {
        property_range: ByteRange,
        property_span: Span,
        optional: bool,
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

    /// `(args)` — call segment in a chain `a.b()`. Renders as
    /// `<call>{args...}</call>`. The bare-call form (`f()` with no
    /// preceding chain) stays as `Ir::Call`; lowering decides which
    /// based on whether the function position is itself an access
    /// chain.
    Call {
        arguments: Vec<Ir>,
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
            | Ir::Tuple { span, .. }
            | Ir::List { span, .. }
            | Ir::Set { span, .. }
            | Ir::Dictionary { span, .. }
            | Ir::Pair { span, .. }
            | Ir::GenericType { span, .. }
            | Ir::Comparison { span, .. }
            | Ir::If { span, .. }
            | Ir::ElseIf { span, .. }
            | Ir::Else { span, .. }
            | Ir::For { span, .. }
            | Ir::Foreach { span, .. }
            | Ir::CFor { span, .. }
            | Ir::DoWhile { span, .. }
            | Ir::While { span, .. }
            | Ir::Break { span, .. }
            | Ir::Continue { span, .. }
            | Ir::Lambda { span, .. }
            | Ir::ObjectCreation { span, .. }
            | Ir::Ternary { span, .. }
            | Ir::FieldWrap { span, .. }
            | Ir::SimpleStatement { span, .. }
            | Ir::Try { span, .. }
            | Ir::ExceptHandler { span, .. }
            | Ir::TypeAlias { span, .. }
            | Ir::KeywordArgument { span, .. }
            | Ir::ListSplat { span, .. }
            | Ir::DictSplat { span, .. }
            | Ir::Function { span, .. }
            | Ir::Class { span, .. }
            | Ir::Body { span, .. }
            | Ir::Parameter { span, .. }
            | Ir::PositionalSeparator { span, .. }
            | Ir::KeywordSeparator { span, .. }
            | Ir::Decorator { span, .. }
            | Ir::Returns { span, .. }
            | Ir::Generic { span, .. }
            | Ir::TypeParameter { span, .. }
            | Ir::Return { span, .. }
            | Ir::Comment { span, .. }
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
            | Ir::Enum { span, .. }
            | Ir::EnumMember { span, .. }
            | Ir::Property { span, .. }
            | Ir::Accessor { span, .. }
            | Ir::Constructor { span, .. }
            | Ir::Using { span, .. }
            | Ir::Namespace { span, .. }
            | Ir::Variable { span, .. }
            | Ir::Is { span, .. }
            | Ir::Cast { span, .. }
            | Ir::Null { span, .. }
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
            | Ir::Tuple { range, .. }
            | Ir::List { range, .. }
            | Ir::Set { range, .. }
            | Ir::Dictionary { range, .. }
            | Ir::Pair { range, .. }
            | Ir::GenericType { range, .. }
            | Ir::Comparison { range, .. }
            | Ir::If { range, .. }
            | Ir::ElseIf { range, .. }
            | Ir::Else { range, .. }
            | Ir::For { range, .. }
            | Ir::Foreach { range, .. }
            | Ir::CFor { range, .. }
            | Ir::DoWhile { range, .. }
            | Ir::While { range, .. }
            | Ir::Break { range, .. }
            | Ir::Continue { range, .. }
            | Ir::Lambda { range, .. }
            | Ir::ObjectCreation { range, .. }
            | Ir::Ternary { range, .. }
            | Ir::FieldWrap { range, .. }
            | Ir::SimpleStatement { range, .. }
            | Ir::Try { range, .. }
            | Ir::ExceptHandler { range, .. }
            | Ir::TypeAlias { range, .. }
            | Ir::KeywordArgument { range, .. }
            | Ir::ListSplat { range, .. }
            | Ir::DictSplat { range, .. }
            | Ir::Function { range, .. }
            | Ir::Class { range, .. }
            | Ir::Body { range, .. }
            | Ir::Parameter { range, .. }
            | Ir::PositionalSeparator { range, .. }
            | Ir::KeywordSeparator { range, .. }
            | Ir::Decorator { range, .. }
            | Ir::Returns { range, .. }
            | Ir::Generic { range, .. }
            | Ir::TypeParameter { range, .. }
            | Ir::Return { range, .. }
            | Ir::Comment { range, .. }
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
            | Ir::Enum { range, .. }
            | Ir::EnumMember { range, .. }
            | Ir::Property { range, .. }
            | Ir::Accessor { range, .. }
            | Ir::Constructor { range, .. }
            | Ir::Using { range, .. }
            | Ir::Namespace { range, .. }
            | Ir::Variable { range, .. }
            | Ir::Is { range, .. }
            | Ir::Cast { range, .. }
            | Ir::Null { range, .. }
            | Ir::Inline { range, .. }
            | Ir::Unknown { range, .. } => *range,
        }
    }
}

impl Ir {
    /// Direct IR children, in source order. Excludes synthetic
    /// render-time wrappers (`<value>`, `<type>`, `<left>`/`<right>`,
    /// `<expression>` host) and modifier markers — those are
    /// rendering metadata, not IR.
    ///
    /// **Internal walker helper, not a public API contract.**
    /// `pub(crate)` until a downstream consumer demands stability.
    /// rustc's HIR uses per-kind `Visitor` methods for this reason —
    /// keeping this internal lets us refactor freely.
    ///
    /// ## What's included
    /// - `Box<Ir>`, `Vec<Ir>`, `Option<Box<Ir>>` fields.
    /// - `AccessSegment` children of `Ir::Access` (member's name is
    ///   not an Ir; index/call have inner Ir children).
    ///
    /// ## What's excluded
    /// - Markers / modifiers (flags, not children).
    /// - Operator text + marker (`op_text`, `op_marker`, `op_range`).
    /// - Static field discriminators (`kind: &'static str` on
    ///   `Accessor`, etc.).
    /// - Comment leading flag, Body pass_only flag.
    pub(crate) fn children(&self) -> Vec<&Ir> {
        let mut v: Vec<&Ir> = Vec::new();
        match self {
            Ir::Module { children, .. } => v.extend(children.iter()),
            Ir::Expression { inner, .. } => v.push(inner),
            Ir::Access { receiver, segments, .. } => {
                v.push(receiver);
                for s in segments {
                    match s {
                        AccessSegment::Member { .. } => {} // property is not an Ir
                        AccessSegment::Index { indices, .. } => v.extend(indices.iter()),
                        AccessSegment::Call { arguments, .. } => v.extend(arguments.iter()),
                    }
                }
            }
            Ir::Call { callee, arguments, .. } => {
                v.push(callee);
                v.extend(arguments.iter());
            }
            Ir::Binary { left, right, .. }
            | Ir::Comparison { left, right, .. } => {
                v.push(left);
                v.push(right);
            }
            Ir::Unary { operand, .. } => v.push(operand),
            Ir::If { condition, body, else_branch, .. }
            | Ir::ElseIf { condition, body, else_branch, .. } => {
                v.push(condition);
                v.push(body);
                if let Some(e) = else_branch { v.push(e); }
            }
            Ir::Else { body, .. } => v.push(body),
            Ir::For { targets, iterables, body, else_body, .. } => {
                v.extend(targets.iter());
                v.extend(iterables.iter());
                v.push(body);
                if let Some(e) = else_body { v.push(e); }
            }
            Ir::While { condition, body, else_body, .. } => {
                v.push(condition);
                v.push(body);
                if let Some(e) = else_body { v.push(e); }
            }
            Ir::Foreach { type_ann, target, iterable, body, .. } => {
                if let Some(t) = type_ann { v.push(t); }
                v.push(target);
                v.push(iterable);
                v.push(body);
            }
            Ir::CFor { initializer, condition, updates, body, .. } => {
                if let Some(i) = initializer { v.push(i); }
                if let Some(c) = condition { v.push(c); }
                v.extend(updates.iter());
                v.push(body);
            }
            Ir::DoWhile { body, condition, .. } => {
                v.push(body);
                v.push(condition);
            }
            Ir::Lambda { parameters, body, .. } => {
                v.extend(parameters.iter());
                v.push(body);
            }
            Ir::ObjectCreation { type_target, arguments, initializer, .. } => {
                if let Some(t) = type_target { v.push(t); }
                v.extend(arguments.iter());
                if let Some(i) = initializer { v.push(i); }
            }
            Ir::Ternary { condition, if_true, if_false, .. } => {
                v.push(condition);
                v.push(if_true);
                v.push(if_false);
            }
            Ir::FieldWrap { inner, .. } => v.push(inner),
            Ir::SimpleStatement { children, .. } => v.extend(children.iter()),
            Ir::Try { try_body, handlers, else_body, finally_body, .. } => {
                v.push(try_body);
                v.extend(handlers.iter());
                if let Some(e) = else_body { v.push(e); }
                if let Some(f) = finally_body { v.push(f); }
            }
            Ir::ExceptHandler { type_target, binding, filter, body, .. } => {
                if let Some(t) = type_target { v.push(t); }
                if let Some(b) = binding { v.push(b); }
                if let Some(f) = filter { v.push(f); }
                v.push(body);
            }
            Ir::TypeAlias { name, type_params, value, .. } => {
                v.push(name);
                if let Some(p) = type_params { v.push(p); }
                v.push(value);
            }
            Ir::KeywordArgument { name, value, .. } => {
                v.push(name);
                v.push(value);
            }
            Ir::ListSplat { inner, .. } => v.push(inner),
            Ir::DictSplat { inner, .. } => v.push(inner),
            Ir::Function { decorators, name, generics, parameters, returns, body, .. } => {
                v.extend(decorators.iter());
                v.push(name);
                if let Some(g) = generics { v.push(g); }
                v.extend(parameters.iter());
                if let Some(r) = returns { v.push(r); }
                v.push(body);
            }
            Ir::Class { decorators, name, generics, bases, body, .. } => {
                v.extend(decorators.iter());
                v.push(name);
                if let Some(g) = generics { v.push(g); }
                v.extend(bases.iter());
                v.push(body);
            }
            Ir::Body { children, .. } => v.extend(children.iter()),
            Ir::Parameter { name, type_ann, default, .. } => {
                v.push(name);
                if let Some(t) = type_ann { v.push(t); }
                if let Some(d) = default { v.push(d); }
            }
            Ir::Decorator { inner, .. } => v.push(inner),
            Ir::Returns { type_ann, .. } => v.push(type_ann),
            Ir::Generic { items, .. } => v.extend(items.iter()),
            Ir::TypeParameter { name, constraint, .. } => {
                v.push(name);
                if let Some(c) = constraint { v.push(c); }
            }
            Ir::Return { value, .. } => {
                if let Some(val) = value { v.push(val); }
            }
            Ir::Assign { targets, type_annotation, values, .. } => {
                v.extend(targets.iter());
                if let Some(t) = type_annotation { v.push(t); }
                v.extend(values.iter());
            }
            Ir::Import { children, .. } => v.extend(children.iter()),
            Ir::From { path, imports, .. } => {
                if let Some(p) = path { v.push(p); }
                v.extend(imports.iter());
            }
            Ir::FromImport { name, alias, .. } => {
                v.push(name);
                if let Some(a) = alias { v.push(a); }
            }
            Ir::Path { segments, .. } => v.extend(segments.iter()),
            Ir::Aliased { inner, .. } => v.push(inner),
            Ir::Tuple { children, .. }
            | Ir::List { children, .. }
            | Ir::Set { children, .. } => v.extend(children.iter()),
            Ir::Dictionary { pairs, .. } => v.extend(pairs.iter()),
            Ir::Pair { key, value, .. } => {
                v.push(key);
                v.push(value);
            }
            Ir::GenericType { name, params, .. } => {
                v.push(name);
                v.extend(params.iter());
            }
            Ir::Is { value, type_target, .. } => {
                v.push(value);
                v.push(type_target);
            }
            Ir::Cast { type_ann, value, .. } => {
                v.push(type_ann);
                v.push(value);
            }
            Ir::Enum { decorators, name, underlying_type, members, .. } => {
                v.extend(decorators.iter());
                v.push(name);
                if let Some(t) = underlying_type { v.push(t); }
                v.extend(members.iter());
            }
            Ir::EnumMember { decorators, name, value, .. } => {
                v.extend(decorators.iter());
                v.push(name);
                if let Some(val) = value { v.push(val); }
            }
            Ir::Property { decorators, type_ann, name, accessors, value, .. } => {
                v.extend(decorators.iter());
                if let Some(t) = type_ann { v.push(t); }
                v.push(name);
                v.extend(accessors.iter());
                if let Some(val) = value { v.push(val); }
            }
            Ir::Accessor { body, .. } => {
                if let Some(b) = body { v.push(b); }
            }
            Ir::Constructor { decorators, name, parameters, body, .. } => {
                v.extend(decorators.iter());
                v.push(name);
                v.extend(parameters.iter());
                v.push(body);
            }
            Ir::Using { alias, path, .. } => {
                v.push(path);
                if let Some(a) = alias { v.push(a); }
            }
            Ir::Namespace { name, children, .. } => {
                v.push(name);
                v.extend(children.iter());
            }
            Ir::Variable { type_ann, name, value, .. } => {
                if let Some(t) = type_ann { v.push(t); }
                v.push(name);
                if let Some(val) = value { v.push(val); }
            }
            Ir::Inline { children, .. } => v.extend(children.iter()),
            // Leaves and markers — no Ir children.
            Ir::Name { .. } | Ir::Int { .. } | Ir::Float { .. } | Ir::String { .. }
            | Ir::True { .. } | Ir::False { .. } | Ir::None { .. } | Ir::Null { .. }
            | Ir::Comment { .. } | Ir::PositionalSeparator { .. }
            | Ir::KeywordSeparator { .. } | Ir::Break { .. } | Ir::Continue { .. }
            | Ir::Unknown { .. } => {}
        }
        // Sort by source order so consumers (renderer, audit walker)
        // don't have to repeat. Variants whose fields are already in
        // source order pay a near-zero sort cost.
        v.sort_by_key(|c| c.range().start);
        v
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
