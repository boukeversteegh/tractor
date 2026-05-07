//! Shared helpers for `lower_<lang>` modules — converting tree-sitter
//! [`Node`](tree_sitter::Node) source positions / byte ranges into
//! the IR's [`ByteRange`] / [`Span`] types, plus borrowing source
//! text by byte range.
//!
//! Each per-language lower module previously redeclared these same
//! three functions (`text_of`, `range_of`, `span_of`) verbatim —
//! ~15 LOC × 13 files = ~200 LOC of pure duplication. Centralizing
//! them here keeps the lower modules focused on per-grammar shape
//! decisions.

#![cfg(feature = "native")]

use tree_sitter::Node as TsNode;

use super::types::{ByteRange, Span};

/// Source bytes covered by `node` as an owned [`String`]. Uses
/// `node.utf8_text()` rather than byte-slicing so an invalid
/// byte range (shouldn't happen for tree-sitter output, but
/// defensively) returns an empty string instead of panicking.
pub fn text_of(node: TsNode<'_>, source: &str) -> String {
    node.utf8_text(source.as_bytes())
        .map(|s| s.to_string())
        .unwrap_or_default()
}

/// Source bytes covered by `node` as a borrowed `&str`. Useful when
/// the caller will hash / compare without allocating.
pub fn text_borrow<'s>(node: TsNode<'_>, source: &'s str) -> &'s str {
    node.utf8_text(source.as_bytes()).unwrap_or("")
}

/// `node`'s byte range as the IR's compact [`ByteRange`] (u32 pair).
pub fn range_of(node: TsNode<'_>) -> ByteRange {
    let r = node.byte_range();
    ByteRange::new(r.start as u32, r.end as u32)
}

/// `node`'s start / end source position as the IR's [`Span`]. Lines
/// and columns are 1-based to match user-visible diagnostics; the
/// tree-sitter API exposes 0-based, so we adjust.
pub fn span_of(node: TsNode<'_>) -> Span {
    let s = node.start_position();
    let e = node.end_position();
    Span {
        line: (s.row + 1) as u32,
        column: (s.column + 1) as u32,
        end_line: (e.row + 1) as u32,
        end_column: (e.column + 1) as u32,
    }
}
