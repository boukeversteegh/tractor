//! [`SqlTree`] → Xot rendering — variant-blind delegate.
//!
//! Thin wrapper that routes through the generic walker in
//! [`crate::tree::walker`]. Same role as the equivalent thin
//! wrappers for `SyntaxTree::to_xot` and `DataTree`'s structure-
//! mode renderer. Every per-variant rendering decision is captured
//! in the typed tree at lowering time — this module owns no per-
//! variant knowledge.
//!
//! ## Source-text recovery
//!
//! The generic walker emits gap text between source-derived
//! children, so XPath `string(rendered_root)` recovers the original
//! source bytes for anchored trees. Synthetic / round-trip trees
//! emit scalar text from each leaf's `text` field.

#![cfg(feature = "native")]

use xot::{Node as XotNode, Xot};

use super::types::SqlTree;
use crate::tree::walker::render_walker_to_xot;

/// Render a [`SqlTree`] as a child of `parent` in `xot`.
pub fn render_sql_to_xot(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SqlTree,
    source: &str,
) -> Result<XotNode, xot::Error> {
    render_walker_to_xot(xot, parent, tree, source, None)
}
