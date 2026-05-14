//! Find a typed-tree node by its [`NodeId`] (Slice 2 of
//! editable-trees; see `docs/design-editable-trees.md`).
//!
//! The xot projection stamps `@id` on every element after
//! [`crate::tree::assign_ids`] runs (Slice 1). An XPath match against
//! the xot tree reads that attribute and then calls one of the
//! `find_by_id_*` walkers in this module to recover the underlying
//! typed-tree node.
//!
//! Walkers are mutable: the locator's job is to deliver a `&mut` so
//! the mutation pipeline can replace text / subtrees in place before
//! re-rendering.
//!
//! One walker fits all three tree families via the [`TreeNode`] trait
//! (`span` + `children_mut`); the per-tree public entry points are
//! thin wrappers.

#![cfg(feature = "native")]

use super::types::{NodeId, TreeNode};

#[cfg(test)]
use super::data::DataTree;
#[cfg(test)]
use super::types::SyntaxTree;

/// Parse an `@id` attribute value into a [`NodeId`]. Returns `None`
/// for missing / unparseable attributes (e.g. xot elements created
/// before `assign_ids` ran).
pub fn parse_id_attr(value: &str) -> Option<NodeId> {
    value.parse::<NodeId>().ok().filter(|id| *id != 0)
}

/// Pre-order search for the node whose `span.id` matches `id`.
/// Generic over any [`TreeNode`].
pub fn find_by_id<T: TreeNode>(tree: &mut T, id: NodeId) -> Option<&mut T> {
    if tree.span().id == id {
        return Some(tree);
    }
    for c in tree.children_mut() {
        if let Some(f) = find_by_id(c, id) {
            return Some(f);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::assign_ids::{assign_ids_data, assign_ids_syntax};
    use crate::tree::types::{ByteRange, Span};

    #[test]
    fn find_root_by_id() {
        let mut t = SyntaxTree::Name {
            text: "x".to_string(),
            range: ByteRange::synthetic_empty(),
            span: Span::point(1, 1),
        };
        assign_ids_syntax(&mut t);
        let id = t.span().id;
        let found = find_by_id(&mut t, id).expect("root should be findable");
        assert_eq!(found.span().id, id);
    }

    #[test]
    fn find_child_by_id() {
        let mut t = SyntaxTree::Module {
            element_name: "module",
            children: vec![
                SyntaxTree::Name {
                    text: "a".to_string(),
                    range: ByteRange::synthetic_empty(),
                    span: Span::point(1, 1),
                },
                SyntaxTree::Name {
                    text: "b".to_string(),
                    range: ByteRange::synthetic_empty(),
                    span: Span::point(2, 1),
                },
            ],
            range: ByteRange::synthetic_empty(),
            span: Span::point(1, 1),
        };
        assign_ids_syntax(&mut t);
        let found_2 = find_by_id(&mut t, 2).expect("id 2 should resolve");
        match found_2 {
            SyntaxTree::Name { text, .. } => assert_eq!(text, "a"),
            _ => panic!("expected Name for id 2"),
        }
    }

    #[test]
    fn find_returns_none_for_unknown_id() {
        let mut t = SyntaxTree::Name {
            text: "x".to_string(),
            range: ByteRange::synthetic_empty(),
            span: Span::point(1, 1),
        };
        assign_ids_syntax(&mut t);
        assert!(find_by_id(&mut t, 999).is_none());
    }

    #[test]
    fn find_in_data_tree() {
        let mut t = DataTree::Mapping {
            pairs: vec![DataTree::Pair {
                key: Box::new(DataTree::String {
                    value: "k".to_string(),
                    quote_style: crate::tree::types::QuoteStyle::Double,
                    range: ByteRange::synthetic_empty(),
                    span: Span::point(1, 1),
                }),
                value: Box::new(DataTree::Number {
                    text: "42".to_string(),
                    range: ByteRange::synthetic_empty(),
                    span: Span::point(1, 5),
                }),
                range: ByteRange::synthetic_empty(),
                span: Span::point(1, 1),
            }],
            range: ByteRange::synthetic_empty(),
            span: Span::point(1, 1),
        };
        assign_ids_data(&mut t);
        let number_id = if let DataTree::Mapping { pairs, .. } = &t {
            if let DataTree::Pair { value, .. } = &pairs[0] {
                if let DataTree::Number { span, .. } = value.as_ref() {
                    span.id
                } else {
                    panic!("expected Number")
                }
            } else {
                panic!("expected Pair")
            }
        } else {
            panic!("expected Mapping")
        };
        let found = find_by_id(&mut t, number_id).expect("number id resolves");
        assert!(matches!(found, DataTree::Number { .. }));
    }

    #[test]
    fn parse_id_attr_accepts_valid_numbers() {
        assert_eq!(parse_id_attr("42"), Some(42));
        assert_eq!(parse_id_attr("0"), None);
        assert_eq!(parse_id_attr("notanumber"), None);
        assert_eq!(parse_id_attr(""), None);
    }
}
