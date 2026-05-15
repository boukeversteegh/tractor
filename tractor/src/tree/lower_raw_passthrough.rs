//! Typed passthrough lowering for languages without a semantic
//! `lower_<lang>_root`.
//!
//! ## Why this exists (S6R)
//!
//! After S6, the typed pipeline covers `TreeKind::Syntax` and
//! `TreeKind::Sql` languages. Eleven languages (HTML, CSS, C, C++,
//! bash, scala, lua, haskell, ocaml, r, julia) still had no typed
//! lowering and routed through the imperative `walk_transform` +
//! `XotBuilder::build_raw_from_serialized` path. That's the last
//! consumer of the imperative machinery; until it's gone, S6B-Retire
//! can't delete it.
//!
//! Rather than hand-write a `lower_<lang>_root` for each, we register
//! a single generic passthrough: walk the named children of every
//! `RawNode` and wrap each one in [`SyntaxTree::Raw`]. The XML output
//! is `<{kind}>{children…}</{kind}>` — the bare CST kind hierarchy
//! with no field-wrapping, no marker injection, no per-language
//! shape decisions.
//!
//! This is the user's stated stance (2026-05-14): "stick as close to
//! raw as possible, only introduce types when we're explicitly
//! modeling structure." Languages that need semantic shape get a
//! real `lower_<lang>_root`; everything else flows through this.
//!
//! ## Conscious behaviour change
//!
//! The imperative path ran `apply_field_wrappings` on these
//! languages (wrapping certain children in `<name>`/`<body>`/etc.).
//! The passthrough deliberately omits that — field-wrappings are a
//! per-language semantic shape decision that belongs in a real
//! lowering, not in a generic catch-all. XML output for
//! HTML/CSS/C/C++/etc. drops the field-wrap layer; queries that
//! relied on it have to migrate to the bare kind hierarchy.

use crate::raw::RawNode;

use super::lower_helpers::{range_of, span_of};
use super::types::SyntaxTree;

/// Lower a `RawNode` tree to `SyntaxTree::Raw` recursively. Only
/// named children make it into `children`; anonymous nodes
/// (punctuation, keyword tokens) are dropped. The source-byte range
/// is preserved on every node, so callers that need verbatim text
/// can recover it via `tree.range().slice(source)`.
///
/// Used as the [`crate::languages::TreeKind::Syntax`] lowering for
/// languages without a semantic `lower_<lang>_root` (currently
/// none registered; placeholder for HTML/CSS/C/C++/etc.).
pub fn lower_raw_passthrough(node: &RawNode, source: &str) -> SyntaxTree {
    lower_inner(node, source, /*include_anonymous=*/ false)
}

/// Variant that keeps anonymous children too. Used for `TreeMode::Raw`
/// — the user-visible "untransformed CST" dump — where every
/// tree-sitter node (named or anonymous, including punctuation and
/// keywords) is preserved as-is.
///
/// This replaces the old imperative `XotBuilder::build_raw_from_serialized`
/// + `build_raw_with_options` path: a RawNode → `SyntaxTree::Raw`
/// tree, then `render_to_xot` emits `<{kind}>{children…}</{kind}>`
/// recursively. No name cache, no kind attributes, no field-wrapping
/// — just the bare CST shape.
pub fn lower_raw_passthrough_all(node: &RawNode, source: &str) -> SyntaxTree {
    lower_inner(node, source, /*include_anonymous=*/ true)
}

fn lower_inner(node: &RawNode, source: &str, include_anonymous: bool) -> SyntaxTree {
    let kind = node.kind().to_string();
    let range = range_of(node);
    let span = span_of(node);
    let is_named = node.is_named();
    let children: Vec<SyntaxTree> = node
        .children()
        .filter(|c| include_anonymous || c.is_named())
        .map(|c| lower_inner(c, source, include_anonymous))
        .collect();
    SyntaxTree::Raw { kind, is_named, children, range, span }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(kind: &str, start_byte: usize, end_byte: usize) -> RawNode {
        RawNode {
            kind: kind.to_string(),
            is_named: true,
            start_row: 0,
            start_col: start_byte,
            end_row: 0,
            end_col: end_byte,
            start_byte,
            end_byte,
            field_name: None,
            children: vec![],
        }
    }

    #[test]
    fn leaf_lowers_to_empty_raw() {
        let n = leaf("identifier", 0, 3);
        let tree = lower_raw_passthrough(&n, "abc");
        match tree {
            SyntaxTree::Raw { kind, children, is_named: _, .. } => {
                assert_eq!(kind, "identifier");
                assert!(children.is_empty());
            }
            _ => panic!("expected Raw"),
        }
    }

    #[test]
    fn nested_named_children_preserved() {
        let mut parent = leaf("element", 0, 10);
        parent.children = vec![leaf("name", 1, 4), leaf("attribute", 5, 9)];
        let tree = lower_raw_passthrough(&parent, "<html>1234</html>");
        match tree {
            SyntaxTree::Raw { kind, children, is_named: _, .. } => {
                assert_eq!(kind, "element");
                assert_eq!(children.len(), 2);
                match &children[0] {
                    SyntaxTree::Raw { kind, .. } => assert_eq!(kind, "name"),
                    _ => panic!("expected Raw"),
                }
                match &children[1] {
                    SyntaxTree::Raw { kind, .. } => assert_eq!(kind, "attribute"),
                    _ => panic!("expected Raw"),
                }
            }
            _ => panic!("expected Raw"),
        }
    }

    #[test]
    fn anonymous_children_dropped() {
        // Anonymous children (e.g. tree-sitter punctuation) should not
        // appear in the lowered tree.
        let mut parent = leaf("call", 0, 10);
        let mut paren_open = leaf("(", 4, 5);
        paren_open.is_named = false;
        let arg = leaf("argument", 5, 8);
        let mut paren_close = leaf(")", 8, 9);
        paren_close.is_named = false;
        parent.children = vec![paren_open, arg, paren_close];
        let tree = lower_raw_passthrough(&parent, "foo(bar);");
        match tree {
            SyntaxTree::Raw { children, is_named: _, .. } => {
                assert_eq!(children.len(), 1);
                match &children[0] {
                    SyntaxTree::Raw { kind, .. } => assert_eq!(kind, "argument"),
                    _ => panic!("expected Raw"),
                }
            }
            _ => panic!("expected Raw"),
        }
    }
}
