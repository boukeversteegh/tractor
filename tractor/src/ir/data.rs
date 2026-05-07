//! Data-language IR — a format-agnostic typed shape for JSON / YAML /
//! TOML / INI.
//!
//! ## Why a separate type from [`crate::ir::Ir`]
//!
//! The programming-language IR ([`crate::ir::Ir`]) is built around
//! constructs like `Class`, `Function`, `If` — it carries semantic
//! type information specific to programming-language ASTs.
//!
//! Data languages have a much smaller, simpler universe — mappings,
//! sequences, scalars. Reusing the programming-language IR would
//! introduce noise; a focused [`DataIr`] type lets each variant
//! carry only what data languages need.
//!
//! ## Format-agnostic
//!
//! A single [`DataIr`] tree can be rendered to any of:
//! - `<object>/<array>/<property>` XML (JSON syntax branch shape)
//! - `<mapping>/<sequence>/<pair>` XML (YAML syntax branch shape)
//! - data-branch XML where keys become element names (`{a: 1}` →
//!   `<a>1</a>`)
//! - canonical JSON / YAML / TOML text
//!
//! The renderer chooses element names + serialization conventions;
//! the IR is purely structural. Mirrors how Xot serves as a shared
//! container today, but with type-checked variants instead of
//! string-keyed elements.
//!
//! ## Invariants (parsed mode)
//!
//! 1. **Round-trip identity** — `to_source(data_ir, source) ==
//!    source`. The renderer that targets the *original* format
//!    preserves bytes verbatim via `range`-anchored gap text.
//!    Cross-format render (e.g. JSON → YAML) breaks round-trip by
//!    construction; the invariant only holds for same-format
//!    renders.
//! 2. **Source attributes** — every variant carries `range:
//!    ByteRange` and `span: Span` for line/column reporting.
//! 3. **No silent drops** — un-handled CST kinds fall through to
//!    [`DataIr::Unknown`] (visible `<unknown kind="…"/>`).

#![cfg(feature = "native")]

use super::types::{ByteRange, Span};

/// Format-agnostic data-language IR.
#[derive(Debug, Clone)]
pub enum DataIr {
    /// Top-level document. The CST root.
    Document {
        children: Vec<DataIr>,
        range: ByteRange,
        span: Span,
    },

    /// Object / mapping / dict / table — collection of pairs.
    /// Renders as `<object>` (JSON) / `<mapping>` (YAML) / `<table>`
    /// (TOML) / `<section>` (INI) per format choice.
    Mapping {
        pairs: Vec<DataIr>, // each is DataIr::Pair (or Comment)
        range: ByteRange,
        span: Span,
    },

    /// Array / list / sequence — ordered values.
    /// Renders as `<array>` (JSON) / `<sequence>` (YAML).
    Sequence {
        items: Vec<DataIr>,
        range: ByteRange,
        span: Span,
    },

    /// Key-value pair. Key is typically [`DataIr::Scalar`] with a
    /// string value (object keys in JSON are quoted strings; YAML
    /// keys can be other scalars). Value is any [`DataIr`].
    /// Renders as `<property><key>...</key><value>...</value></property>`
    /// in syntax mode; data mode lifts the key string to the
    /// element name.
    Pair {
        key: Box<DataIr>,
        value: Box<DataIr>,
        range: ByteRange,
        span: Span,
    },

    /// Section header (INI / TOML). Distinct from a Pair containing
    /// a Mapping because the source bytes for `[name]` need to be
    /// preserved.
    Section {
        name: Box<DataIr>, // typically Scalar(String)
        children: Vec<DataIr>,
        range: ByteRange,
        span: Span,
    },

    /// String scalar. `value` is the *parsed* string (escape
    /// sequences resolved). Source bytes (with quotes / escapes)
    /// recoverable via `range.slice(source)`.
    String {
        value: String,
        range: ByteRange,
        span: Span,
    },

    /// Numeric scalar. We keep the *raw text* rather than parsing
    /// to f64/i64 to avoid lossy round-trip (`1.0` ≠ `1` in YAML
    /// even though both parse to the same number).
    Number {
        text: String,
        range: ByteRange,
        span: Span,
    },

    /// Boolean scalar. The raw text (`"true"` / `"false"` / YAML's
    /// `yes`/`no`/...) is recoverable via the range.
    Bool {
        value: bool,
        range: ByteRange,
        span: Span,
    },

    /// Null / nil literal. JSON `null`, YAML `null` / `~` / empty,
    /// TOML omits but YAML+JSON5 have it.
    Null {
        range: ByteRange,
        span: Span,
    },

    /// Comment (YAML / TOML / INI / JSON5). Text excludes the
    /// leading delimiter (`#` / `//`) — recover it via the range
    /// when the original delimiter matters. `leading` / `trailing`
    /// are inferred during lowering: a comment immediately after a
    /// value on the same line is `trailing: true`.
    Comment {
        text: String,
        leading: bool,
        trailing: bool,
        range: ByteRange,
        span: Span,
    },

    /// Last-resort hatch for unhandled kinds. Renders as
    /// `<unknown kind="…">…</unknown>` so XPath queries can still
    /// traverse and source-text recovery still holds.
    Unknown {
        kind: String,
        range: ByteRange,
        span: Span,
    },
}

impl DataIr {
    /// Source byte range covered by this node.
    pub fn range(&self) -> ByteRange {
        match self {
            DataIr::Document { range, .. }
            | DataIr::Mapping { range, .. }
            | DataIr::Sequence { range, .. }
            | DataIr::Pair { range, .. }
            | DataIr::Section { range, .. }
            | DataIr::String { range, .. }
            | DataIr::Number { range, .. }
            | DataIr::Bool { range, .. }
            | DataIr::Null { range, .. }
            | DataIr::Comment { range, .. }
            | DataIr::Unknown { range, .. } => *range,
        }
    }

    /// Source-location span (line / column).
    pub fn span(&self) -> Span {
        match self {
            DataIr::Document { span, .. }
            | DataIr::Mapping { span, .. }
            | DataIr::Sequence { span, .. }
            | DataIr::Pair { span, .. }
            | DataIr::Section { span, .. }
            | DataIr::String { span, .. }
            | DataIr::Number { span, .. }
            | DataIr::Bool { span, .. }
            | DataIr::Null { span, .. }
            | DataIr::Comment { span, .. }
            | DataIr::Unknown { span, .. } => *span,
        }
    }

    /// Round-trip helper: the original source slice covered by this
    /// node. Equivalent to `self.range().slice(source)`.
    pub fn to_source<'a>(&self, source: &'a str) -> &'a str {
        self.range().slice(source)
    }
}
