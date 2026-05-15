//! Post-construction walker that stamps a stable [`NodeId`] onto every
//! node's `Span` (see `docs/design-editable-trees.md`).
//!
//! Invariant: every tree exiting the parser has IDs assigned. Synthetic
//! / mutated trees re-run [`assign_ids`] before any subsequent XPath
//! → typed-node lookup. Parallel subtree transforms re-stamp at their
//! merge point.
//!
//! IDs are session-scoped: assignment always starts from a fresh
//! counter at `1` and renumbers everything. They are not stable across
//! re-parses; they only need to survive the parse → project → query →
//! mutate → render cycle of one invocation.
//!
//! One walker fits all three tree families via the [`TreeNode`] trait
//! (`span_mut` + `children_mut`). Per-variant structural knowledge
//! lives in the tree's `children_mut` impl, not here — adding a new
//! variant requires no change to this file.

use super::data::DataTree;
use super::sql::SqlTree;
use super::types::{NodeId, SyntaxTree, TreeNode};

/// Stamp a fresh [`NodeId`] onto every node in a [`SyntaxTree`].
/// Counter starts at `1`; `0` remains reserved as the "unassigned"
/// sentinel.
pub fn assign_ids_syntax(tree: &mut SyntaxTree) {
    walk(tree, &mut 0);
}

/// Stamp a fresh [`NodeId`] onto every node in a [`DataTree`].
pub fn assign_ids_data(tree: &mut DataTree) {
    walk(tree, &mut 0);
}

/// Stamp a fresh [`NodeId`] onto every node in a [`SqlTree`].
pub fn assign_ids_sql(tree: &mut SqlTree) {
    walk(tree, &mut 0);
}

fn walk<T: TreeNode>(tree: &mut T, next: &mut NodeId) {
    *next += 1;
    tree.span_mut().id = *next;
    for child in tree.children_mut() {
        walk(child, next);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::types::{ByteRange, Span};

    #[test]
    fn syntax_root_gets_id_one() {
        let mut t = SyntaxTree::Name {
            text: "x".to_string(),
            range: ByteRange::synthetic_empty(),
            span: Span::point(1, 1),
        };
        assign_ids_syntax(&mut t);
        assert_eq!(t.span().id, 1);
    }

    #[test]
    fn syntax_children_get_distinct_increasing_ids() {
        let mut t = SyntaxTree::Module {
            children: vec![
                SyntaxTree::Name {
                    text: "x".to_string(),
                    range: ByteRange::synthetic_empty(),
                    span: Span::point(1, 1),
                },
                SyntaxTree::Name {
                    text: "y".to_string(),
                    range: ByteRange::synthetic_empty(),
                    span: Span::point(2, 1),
                },
            ],
            range: ByteRange::synthetic_empty(),
            span: Span::point(1, 1),
        };
        assign_ids_syntax(&mut t);
        let root_id = t.span().id;
        assert_eq!(root_id, 1);
        let children = t.children();
        assert_eq!(children.len(), 2);
        assert_eq!(children[0].span().id, 2);
        assert_eq!(children[1].span().id, 3);
    }

    #[test]
    fn data_root_gets_id_one() {
        let mut t = DataTree::Null {
            text: String::new(),
            range: ByteRange::synthetic_empty(),
            span: Span::point(1, 1),
        };
        assign_ids_data(&mut t);
        assert_eq!(t.span().id, 1);
    }

    #[test]
    fn assign_ids_is_idempotent_for_renumber() {
        let mut t = SyntaxTree::Name {
            text: "x".to_string(),
            range: ByteRange::synthetic_empty(),
            span: Span::point(1, 1),
        };
        assign_ids_syntax(&mut t);
        let first = t.span().id;
        assign_ids_syntax(&mut t);
        let second = t.span().id;
        assert_eq!(first, second, "fresh-counter assignment is deterministic");
    }

    #[test]
    fn every_node_gets_nonzero_id() {
        // Guards against a `children_mut` impl forgetting to list a
        // child slot: any missed subtree would keep id == 0.
        let mut t = SyntaxTree::Module {
            children: vec![
                SyntaxTree::Binary {
                    op_text: "+".to_string(),
                    op_marker: "add",
                    op_range: ByteRange::synthetic_empty(),
                    left: Box::new(SyntaxTree::Name {
                        text: "a".to_string(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(1, 1),
                    }),
                    right: Box::new(SyntaxTree::Name {
                        text: "b".to_string(),
                        range: ByteRange::synthetic_empty(),
                        span: Span::point(1, 5),
                    }),
                    range: ByteRange::synthetic_empty(),
                    span: Span::point(1, 1),
                },
            ],
            range: ByteRange::synthetic_empty(),
            span: Span::point(1, 1),
        };
        assign_ids_syntax(&mut t);
        check_all_nonzero(&t);
    }

    fn check_all_nonzero(tree: &SyntaxTree) {
        assert_ne!(tree.span().id, 0, "node has unassigned id: {:?}", tree);
        for c in tree.children() {
            check_all_nonzero(c);
        }
    }
}
