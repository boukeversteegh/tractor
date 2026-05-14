//! `RawNode` — owned tree-sitter-shaped CST node, the lowering input on
//! both native and WASM.
//!
//! ## Why this type exists (S6 context)
//!
//! The typed-tree pipeline's lowering functions
//! (`lower_<lang>_root(node, source)`) originally took
//! [`tree_sitter::Node`](tree_sitter::Node) directly. That worked
//! natively but broke the WASM build, because `tree_sitter` (the Rust
//! crate) doesn't compile to WASM — the WASM target relies on
//! `web-tree-sitter` (the JS package) running in the browser, which
//! can't hand a `tree_sitter::Node` back across the FFI boundary.
//!
//! `RawNode` restores the "tree-sitter is one producer" property. On
//! native, [`RawNode::from_tree_sitter`] converts a live
//! `tree_sitter::Node` into an owned `RawNode`. On WASM, the JS side
//! serialises its `SyntaxNode` to JSON, and serde deserialises it into
//! the same `RawNode`. Both targets call `lower_*_root(&raw, source)`
//! with the same input type.
//!
//! ## API surface
//!
//! Hand-mirrors the narrow tree-sitter `Node` API the lowerings
//! actually call: [`RawNode::kind`], [`RawNode::is_named`],
//! [`RawNode::byte_range`], [`RawNode::start_byte`] /
//! [`RawNode::end_byte`], [`RawNode::start_position`] /
//! [`RawNode::end_position`], [`RawNode::utf8_text`],
//! [`RawNode::children`], [`RawNode::named_children`],
//! [`RawNode::child_by_field_name`], [`RawNode::field_name`].
//!
//! Differences from `tree_sitter::Node`:
//! - `RawNode` is owned; children-iterating methods return
//!   `&RawNode`, not `Copy`'d nodes.
//! - No cursor parameter on `children` / `named_children` — the
//!   children are an owned `Vec`, so a plain slice iterator suffices.
//! - `utf8_text(source)` returns `&str` directly (no `Result`) — for
//!   well-formed tree-sitter output the byte range is always valid
//!   UTF-8, and out-of-range byte offsets indicate a bug rather than
//!   something to handle.
//! - No `parent()` method. The typed pipeline threads any needed
//!   ancestor context through the descent (S6 refactored the few
//!   callers that walked upward).

use serde::{Deserialize, Serialize};

/// A 0-indexed source position (row, column) matching tree-sitter's
/// [`Point`](tree_sitter::Point) shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    pub row: usize,
    pub column: usize,
}

/// Owned tree-sitter-shaped CST node.
///
/// See module docs for the role this type plays in the lowering
/// pipeline. The JSON shape (camelCase field names) matches what
/// `web/src/parser.ts:serializeNode` emits, so JS → Rust JSON round-
/// trips without a translation step.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawNode {
    /// The grammar's kind name (e.g. `"function_declaration"`,
    /// `"identifier"`).
    pub kind: String,

    /// `true` for named nodes in the grammar (most syntactic
    /// constructs). `false` for anonymous nodes (punctuation, keywords
    /// when not exposed as named).
    pub is_named: bool,

    /// 0-indexed start row.
    pub start_row: usize,
    /// 0-indexed start column (bytes within the row).
    pub start_col: usize,
    /// 0-indexed end row (exclusive).
    pub end_row: usize,
    /// 0-indexed end column (exclusive).
    pub end_col: usize,

    /// Inclusive start byte offset into the source.
    pub start_byte: usize,
    /// Exclusive end byte offset into the source.
    pub end_byte: usize,

    /// Field name this node fills under its parent, if any. Set by the
    /// parent during construction; the root has `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field_name: Option<String>,

    /// Direct children (both named and anonymous, in source order).
    #[serde(default)]
    pub children: Vec<RawNode>,
}

impl RawNode {
    /// The grammar's kind name for this node.
    #[inline]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// Whether this node is a named (vs anonymous) grammar node.
    #[inline]
    pub fn is_named(&self) -> bool {
        self.is_named
    }

    /// The half-open byte range `[start_byte, end_byte)` covered by
    /// this node.
    #[inline]
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.start_byte..self.end_byte
    }

    /// Inclusive start byte offset.
    #[inline]
    pub fn start_byte(&self) -> usize {
        self.start_byte
    }

    /// Exclusive end byte offset.
    #[inline]
    pub fn end_byte(&self) -> usize {
        self.end_byte
    }

    /// 0-indexed start position.
    #[inline]
    pub fn start_position(&self) -> Point {
        Point { row: self.start_row, column: self.start_col }
    }

    /// 0-indexed end position.
    #[inline]
    pub fn end_position(&self) -> Point {
        Point { row: self.end_row, column: self.end_col }
    }

    /// Source text covered by this node, borrowed from `source`.
    ///
    /// Returns an empty string if the byte range is out of bounds
    /// (defensive; this shouldn't happen for tree-sitter output).
    #[inline]
    pub fn utf8_text<'s>(&self, source: &'s str) -> &'s str {
        source.get(self.start_byte..self.end_byte).unwrap_or("")
    }

    /// All direct children (named + anonymous), in source order.
    #[inline]
    pub fn children(&self) -> std::slice::Iter<'_, RawNode> {
        self.children.iter()
    }

    /// Named direct children only, in source order.
    #[inline]
    pub fn named_children(&self) -> impl Iterator<Item = &RawNode> {
        self.children.iter().filter(|c| c.is_named)
    }

    /// The first child whose [`field_name`](Self::field_name) equals
    /// `name`, if any.
    #[inline]
    pub fn child_by_field_name(&self, name: &str) -> Option<&RawNode> {
        self.children.iter().find(|c| c.field_name.as_deref() == Some(name))
    }

    /// Field name this node fills under its parent, if any.
    #[inline]
    pub fn field_name(&self) -> Option<&str> {
        self.field_name.as_deref()
    }

    /// Whether this is a leaf node (no children).
    #[inline]
    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    /// Pointer-based identity for this node. Two `&RawNode` references
    /// compare equal iff they point to the same node in the same owned
    /// tree. Used by lowerings that needed `tree_sitter::Node::id` to
    /// dedupe / cross-reference children. Stable as long as the tree
    /// isn't moved; lowerings hold the tree by reference for the
    /// duration of the lowering, so this is safe.
    #[inline]
    pub fn id(&self) -> usize {
        self as *const RawNode as usize
    }

    /// The field name of the child at index `child_index`, if any.
    /// Mirrors `tree_sitter::Node::field_name_for_child`. The index is
    /// over *all* children (named and anonymous), matching tree-sitter.
    #[inline]
    pub fn field_name_for_child(&self, child_index: u32) -> Option<&str> {
        self.children.get(child_index as usize).and_then(|c| c.field_name())
    }
}

// --- Native conversion from `tree_sitter::Node` --------------------------

#[cfg(feature = "native")]
impl RawNode {
    /// Convert a live `tree_sitter::Node` (plus its source) into an
    /// owned `RawNode`. Walks the CST once via a cursor so each
    /// child's `field_name` is captured (the cursor exposes field
    /// names; `tree_sitter::Node::children` alone does not).
    ///
    /// The `source` parameter is currently unused — `RawNode` stores
    /// byte offsets and re-slices `source` on demand via
    /// [`RawNode::utf8_text`]. It's accepted in the signature so that
    /// the conversion stays a one-call API even if a future change
    /// needs the source bytes at construction time (e.g. for inline
    /// text caching).
    pub fn from_tree_sitter(node: tree_sitter::Node<'_>, _source: &str) -> Self {
        let mut cursor = node.walk();
        Self::from_ts_with_cursor(&mut cursor, None)
    }

    /// Recursive helper that uses a single shared cursor to walk the
    /// CST in pre-order, capturing per-child field names.
    fn from_ts_with_cursor(
        cursor: &mut tree_sitter::TreeCursor<'_>,
        field_name: Option<String>,
    ) -> Self {
        let node = cursor.node();
        let start = node.start_position();
        let end = node.end_position();

        let mut children: Vec<RawNode> = Vec::new();
        if cursor.goto_first_child() {
            loop {
                let child_field = cursor.field_name().map(|s| s.to_string());
                let child = Self::from_ts_with_cursor(cursor, child_field);
                children.push(child);
                if !cursor.goto_next_sibling() { break; }
            }
            cursor.goto_parent();
        }

        RawNode {
            kind: node.kind().to_string(),
            is_named: node.is_named(),
            start_row: start.row,
            start_col: start.column,
            end_row: end.row,
            end_col: end.column,
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            field_name,
            children,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_basic_node() {
        let json = r#"{
            "kind": "identifier",
            "isNamed": true,
            "startRow": 0,
            "startCol": 0,
            "endRow": 0,
            "endCol": 3,
            "startByte": 0,
            "endByte": 3,
            "children": []
        }"#;
        let node: RawNode = serde_json::from_str(json).unwrap();
        assert_eq!(node.kind(), "identifier");
        assert!(node.is_named());
        assert!(node.is_leaf());
        assert_eq!(node.byte_range(), 0..3);
        assert_eq!(node.start_position(), Point { row: 0, column: 0 });
        assert_eq!(node.field_name(), None);
    }

    #[test]
    fn deserialize_with_field_name() {
        let json = r#"{
            "kind": "identifier",
            "isNamed": true,
            "startRow": 0, "startCol": 0,
            "endRow": 0, "endCol": 3,
            "startByte": 0, "endByte": 3,
            "fieldName": "name",
            "children": []
        }"#;
        let node: RawNode = serde_json::from_str(json).unwrap();
        assert_eq!(node.field_name(), Some("name"));
    }

    #[test]
    fn utf8_text_slices_source() {
        let node = RawNode {
            kind: "identifier".to_string(),
            is_named: true,
            start_row: 0, start_col: 3,
            end_row: 0, end_col: 6,
            start_byte: 3, end_byte: 6,
            field_name: None,
            children: vec![],
        };
        let source = "fn foo() {}";
        assert_eq!(node.utf8_text(source), "foo");
    }

    #[test]
    fn utf8_text_out_of_bounds_returns_empty() {
        let node = RawNode {
            kind: "x".to_string(),
            is_named: true,
            start_row: 0, start_col: 0,
            end_row: 0, end_col: 0,
            start_byte: 100, end_byte: 200,
            field_name: None,
            children: vec![],
        };
        assert_eq!(node.utf8_text("short"), "");
    }

    #[test]
    fn named_children_filters_anonymous() {
        let node = RawNode {
            kind: "parent".to_string(),
            is_named: true,
            start_row: 0, start_col: 0, end_row: 0, end_col: 10,
            start_byte: 0, end_byte: 10,
            field_name: None,
            children: vec![
                RawNode {
                    kind: "a".to_string(), is_named: true,
                    start_row: 0, start_col: 0, end_row: 0, end_col: 1,
                    start_byte: 0, end_byte: 1, field_name: None, children: vec![],
                },
                RawNode {
                    kind: ",".to_string(), is_named: false,
                    start_row: 0, start_col: 1, end_row: 0, end_col: 2,
                    start_byte: 1, end_byte: 2, field_name: None, children: vec![],
                },
                RawNode {
                    kind: "b".to_string(), is_named: true,
                    start_row: 0, start_col: 2, end_row: 0, end_col: 3,
                    start_byte: 2, end_byte: 3, field_name: None, children: vec![],
                },
            ],
        };
        let kinds: Vec<&str> = node.named_children().map(|c| c.kind()).collect();
        assert_eq!(kinds, vec!["a", "b"]);
        let all_kinds: Vec<&str> = node.children().map(|c| c.kind()).collect();
        assert_eq!(all_kinds, vec!["a", ",", "b"]);
    }

    #[test]
    fn child_by_field_name_finds_first_match() {
        let node = RawNode {
            kind: "decl".to_string(),
            is_named: true,
            start_row: 0, start_col: 0, end_row: 0, end_col: 10,
            start_byte: 0, end_byte: 10,
            field_name: None,
            children: vec![
                RawNode {
                    kind: "x".to_string(), is_named: true,
                    start_row: 0, start_col: 0, end_row: 0, end_col: 1,
                    start_byte: 0, end_byte: 1,
                    field_name: Some("type".to_string()), children: vec![],
                },
                RawNode {
                    kind: "y".to_string(), is_named: true,
                    start_row: 0, start_col: 2, end_row: 0, end_col: 3,
                    start_byte: 2, end_byte: 3,
                    field_name: Some("name".to_string()), children: vec![],
                },
            ],
        };
        assert_eq!(node.child_by_field_name("name").map(|c| c.kind()), Some("y"));
        assert!(node.child_by_field_name("missing").is_none());
    }

    #[cfg(feature = "native")]
    #[test]
    fn from_tree_sitter_round_trip() {
        let mut parser = tree_sitter::Parser::new();
        let language: tree_sitter::Language = tree_sitter_json::LANGUAGE.into();
        parser.set_language(&language).unwrap();
        let source = r#"{"name": "alice", "age": 30}"#;
        let tree = parser.parse(source, None).unwrap();
        let raw = RawNode::from_tree_sitter(tree.root_node(), source);
        assert_eq!(raw.kind(), "document");
        assert!(raw.is_named());
        assert_eq!(raw.byte_range(), 0..source.len());
        // document has an object child
        let object = raw.named_children().next().unwrap();
        assert_eq!(object.kind(), "object");
        // first pair has fields "key" and "value"
        let first_pair = object.named_children().next().unwrap();
        assert_eq!(first_pair.kind(), "pair");
        let key = first_pair.child_by_field_name("key").unwrap();
        assert_eq!(key.utf8_text(source), "\"name\"");
        let value = first_pair.child_by_field_name("value").unwrap();
        assert_eq!(value.utf8_text(source), "\"alice\"");
    }
}
