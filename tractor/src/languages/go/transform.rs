//! Go transform logic — thin dispatcher driven by `semantic::rule`.
//!
//! The per-kind logic is split:
//!   - Pure rename / flatten / shared compositions live as data in
//!     [`super::semantic::rule`].
//!   - Language-specific custom logic lives as named functions in
//!     [`super::handlers`].
//!
//! This file's job is just to look up the kind and execute its rule.

use xot::{Xot, Node as XotNode};
use crate::transform::{TransformAction, helpers::*};
use crate::transform::operators::is_operator_marker;
use crate::output::syntax_highlight::SyntaxCategory;

use super::kind::GoKind;

/// Transform a Go AST node.
///
/// Dispatch is split in two:
///   1. If the node carries a `kind` attribute (set by the builder
///      from the original tree-sitter kind), look it up in `GoKind`,
///      fetch its `Rule` from `semantic::rule`, and execute via the
///      shared [`crate::languages::rule::dispatch`].
///   2. Otherwise the node is a builder-inserted wrapper (e.g. the
///      `<name>` field wrapper) — handle inline.
pub fn transform(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let kind_str = match get_kind(xot, node) {
        Some(k) => k,
        None => {
            // Builder-inserted wrapper (no `kind` attribute) — dispatch
            // by element name to a transformation by the same name.
            let name = get_element_name(xot, node).unwrap_or_default();
            return match name.as_str() {
                "name" => super::transformations::name_wrapper(xot, node),
                _ => Ok(TransformAction::Continue),
            };
        }
    };

    // Unknown kinds (parse errors, synthetic nodes) keep their kind
    // name unchanged — same behavior as the old `_` arm fallback when
    // `apply_rename` returned `None`.
    let kind = match GoKind::from_str(&kind_str) {
        Some(k) => k,
        None => return Ok(TransformAction::Continue),
    };

    crate::languages::rule::dispatch(xot, node, super::semantic::rule(kind))
}

/// Map a transformed element name to a syntax category for highlighting.
///
/// Consults the per-name NODES table first (one source of truth);
/// falls back to cross-cutting rules for names not in NODES.
pub fn syntax_category(element: &str) -> SyntaxCategory {
    if let Some(spec) = super::semantic::spec(element) {
        return spec.syntax;
    }
    match element {
        // Raw tree-sitter kinds / builder wrappers not in NODES:
        "parameters" => SyntaxCategory::Keyword,
        _ if is_operator_marker(element) => SyntaxCategory::Operator,
        _ => SyntaxCategory::Default,
    }
}
