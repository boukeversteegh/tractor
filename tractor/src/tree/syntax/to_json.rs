//! Tree → JSON projection — thin wrapper.
//!
//! Pre-refactor, this file held a ~170 LOC variant-blind walker
//! hard-coded for `SyntaxTree`. The generic walker in
//! [`crate::tree::walker::render_walker_to_json`] now serves all
//! three tree families. This module remains as the public entry
//! point for `SyntaxTree`-shaped JSON rendering — same signature
//! as before so existing callers keep compiling.

#![cfg(feature = "native")]

use serde_json::Value;

use super::types::SyntaxTree;
use crate::tree::walker::render_walker_to_json;

/// Entry point. Render `tree` (with its `source`) to a JSON Value.
/// `lang` selects per-language element naming; `None` uses universal
/// variant tags throughout (useful for invertible JSON round-trips).
pub fn tree_to_json(tree: &SyntaxTree, source: &str, lang: Option<&str>) -> Value {
    render_walker_to_json(tree, source, lang)
}
