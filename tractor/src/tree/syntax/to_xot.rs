//! Tree → XML projection — variant-blind walk.
//!
//! Thin wrapper that delegates to the **generic** walker in
//! [`crate::tree::walker`]. Pre-refactor, this file held a
//! 230 LOC walker hard-coded for `SyntaxTree`; the generic walker
//! now serves all three tree families (`SyntaxTree`, `DataTree`,
//! `SqlTree`) from a single implementation. This module remains as
//! the public entry point for `SyntaxTree`-shaped rendering — same
//! signature as before so existing callers keep compiling.

#![cfg(feature = "native")]

use xot::{Node as XotNode, Xot};

use super::types::SyntaxTree;
use crate::tree::walker::render_walker_to_xot;

/// Render `tree` under `parent` and return the appended wrapper
/// element. `lang` selects the per-language element-name vocabulary
/// (Python "module" / C# "unit" / etc.); `None` keeps the universal
/// variant-tag names.
pub fn render_to_xot(
    xot: &mut Xot,
    parent: XotNode,
    tree: &SyntaxTree,
    source: &str,
    lang: Option<&str>,
) -> Result<XotNode, xot::Error> {
    render_walker_to_xot(xot, parent, tree, source, lang)
}
