//! Shared helpers for `lower_<lang>` modules — converting [`RawNode`]
//! source positions / byte ranges into the tree's [`ByteRange`] /
//! [`Span`] types, plus borrowing source text by byte range.
//!
//! Each per-language lower module previously redeclared these same
//! three functions (`text_of`, `range_of`, `span_of`) verbatim —
//! ~15 LOC × 13 files = ~200 LOC of pure duplication. Centralizing
//! them here keeps the lower modules focused on per-grammar shape
//! decisions.
//!
//! Scalar-variant constructors (`name_at`, `int_at`, …) populate the
//! `text` field added in S13-Z1 and stamp `anchored: true` on the
//! produced node (S13-Z2). They are the parse-time path; synthetic
//! callers (tests, programmatic synthesis) build variants directly
//! with `ByteRange::synthetic*`.
//!
//! ## RawNode (S6)
//!
//! Lowerings receive a `&RawNode` rather than a `tree_sitter::Node`
//! so the same lowering can run on native (tree-sitter Rust crate) and
//! WASM (serialised AST from `web-tree-sitter`). See
//! [`crate::raw`](crate::raw) for the type and its conversion path.

use crate::raw::RawNode;

use super::types::{ByteRange, QuoteStyle, Span, SyntaxTree};

/// Source bytes covered by `node` as an owned [`String`].
pub fn text_of(node: &RawNode, source: &str) -> String {
    node.utf8_text(source).to_string()
}

/// Source bytes covered by `node` as a borrowed `&str`. Useful when
/// the caller will hash / compare without allocating.
pub fn text_borrow<'s>(node: &RawNode, source: &'s str) -> &'s str {
    node.utf8_text(source)
}

/// `node`'s byte range as the tree's compact [`ByteRange`] (u32 pair).
pub fn range_of(node: &RawNode) -> ByteRange {
    ByteRange::new(node.start_byte() as u32, node.end_byte() as u32)
}

/// `node`'s start / end source position as the tree's [`Span`]. Lines
/// and columns are 1-based to match user-visible diagnostics; the
/// tree-sitter API exposes 0-based, so we adjust.
pub fn span_of(node: &RawNode) -> Span {
    let s = node.start_position();
    let e = node.end_position();
    Span {
        line: (s.row + 1) as u32,
        column: (s.column + 1) as u32,
        end_line: (e.row + 1) as u32,
        end_column: (e.column + 1) as u32,
        id: 0,
    }
}

// -- Scalar-variant constructors (S13-Z3) ---------------------------------
//
// Each helper takes a [`RawNode`] + source and produces the
// corresponding `SyntaxTree::*` scalar variant with `text` populated
// from the node's source slice and `range.anchored = true`. Callers in
// lowering modules use these instead of the raw `SyntaxTree::* { ... }`
// struct literal so the text/anchored fields stay in sync without
// per-lowering boilerplate.

/// `SyntaxTree::Name` built from a [`RawNode`].
pub fn name_of(node: &RawNode, source: &str) -> SyntaxTree {
    SyntaxTree::Name {
        text: text_of(node, source),
        range: range_of(node),
        span: span_of(node),
    }
}

/// `SyntaxTree::Name` from explicit text + range + span (when the
/// caller already has them, e.g. after string manipulation).
pub fn name_with(text: String, range: ByteRange, span: Span) -> SyntaxTree {
    SyntaxTree::Name { text, range, span }
}

/// `SyntaxTree::Atom` with the given XML element name.
pub fn atom_of(element_name: &'static str, node: &RawNode, source: &str) -> SyntaxTree {
    SyntaxTree::Atom {
        element_name,
        text: text_of(node, source),
        range: range_of(node),
        span: span_of(node),
    }
}

/// `SyntaxTree::Int` built from a [`RawNode`].
pub fn int_of(node: &RawNode, source: &str) -> SyntaxTree {
    SyntaxTree::Int {
        text: text_of(node, source),
        range: range_of(node),
        span: span_of(node),
    }
}

/// `SyntaxTree::Float` built from a [`RawNode`].
pub fn float_of(node: &RawNode, source: &str) -> SyntaxTree {
    SyntaxTree::Float {
        text: text_of(node, source),
        range: range_of(node),
        span: span_of(node),
    }
}

/// `SyntaxTree::String` built from a [`RawNode`]. The `text` is stored
/// verbatim from the source (including surrounding quotes); the
/// per-language lowering is responsible for swapping in decoded content
/// + a `QuoteStyle` for the round-trip property when that matters
/// (S13-Z5 work). Until then this conservative form preserves the
/// source slice and lets the renderer emit it byte-for-byte in
/// anchored mode.
pub fn string_of(node: &RawNode, source: &str) -> SyntaxTree {
    SyntaxTree::String {
        text: text_of(node, source),
        quote_style: QuoteStyle::default_double(),
        range: range_of(node),
        span: span_of(node),
    }
}

/// `SyntaxTree::True` built from a [`RawNode`].
pub fn true_of(node: &RawNode, source: &str) -> SyntaxTree {
    SyntaxTree::True {
        text: text_of(node, source),
        range: range_of(node),
        span: span_of(node),
    }
}

/// `SyntaxTree::False` built from a [`RawNode`].
pub fn false_of(node: &RawNode, source: &str) -> SyntaxTree {
    SyntaxTree::False {
        text: text_of(node, source),
        range: range_of(node),
        span: span_of(node),
    }
}

/// `SyntaxTree::None` built from a [`RawNode`].
pub fn none_of(node: &RawNode, source: &str) -> SyntaxTree {
    SyntaxTree::None {
        text: text_of(node, source),
        range: range_of(node),
        span: span_of(node),
    }
}

/// `SyntaxTree::Null` built from a [`RawNode`].
pub fn null_of(node: &RawNode, source: &str) -> SyntaxTree {
    SyntaxTree::Null {
        text: text_of(node, source),
        range: range_of(node),
        span: span_of(node),
    }
}
