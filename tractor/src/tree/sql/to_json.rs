//! [`SqlTree`] → `serde_json::Value` — variant-blind delegate.
//!
//! Thin wrapper around the generic walker in
//! [`crate::tree::walker`]. Same role as [`crate::tree::syntax::to_json`]
//! and [`crate::tree::data::to_json`]: a `$type`-discriminated
//! projection of the tree's *structure*, driven entirely by the
//! variant-blind metadata. No per-variant rendering knowledge lives
//! in this module.

#![cfg(feature = "native")]

use serde_json::Value;

use super::types::SqlTree;
use crate::tree::walker::render_walker_to_json;

/// Render a [`SqlTree`] tree to its variant-blind JSON projection.
pub fn sql_to_json(tree: &SqlTree, source: &str) -> Value {
    render_walker_to_json(tree, source, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raw::RawNode;
    use crate::languages::tsql::lower_sql_root;
    use tree_sitter::Parser;

    fn render(src: &str) -> Value {
        let language = tree_sitter_sequel_tsql::LANGUAGE.into();
        let mut parser = Parser::new();
        parser.set_language(&language).unwrap();
        let cst = parser.parse(src, None).unwrap();
        let tree = lower_sql_root(&RawNode::from_tree_sitter(cst.root_node(), src), src);
        sql_to_json(&tree, src)
    }

    #[test]
    fn select_renders_as_typed_object() {
        let v = render("SELECT 1");
        // File wrapper holds a statement which holds the select.
        let s = v.to_string();
        assert!(s.contains("\"$type\""));
        assert!(s.contains("select"));
    }
}
