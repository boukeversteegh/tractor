//! Python transform logic — thin dispatcher driven by `rules::rule`.
//!
//! The per-kind logic is split:
//!   - Pure rename / flatten / shared compositions live as data in
//!     [`super::rules::rule`].
//!   - Language-specific custom logic lives as named functions in
//!     [`super::transformations`].
//!
//! This file's job is just to look up the kind and execute its rule.

use xot::{Xot, Node as XotNode};
use crate::transform::{TransformAction, helpers::*};
use crate::transform::operators::is_operator_marker;
use crate::output::syntax_highlight::SyntaxCategory;

use super::input::PyKind;

/// Transform a Python AST node.
///
/// Dispatch is split in two:
///   1. If the node carries a `kind` attribute (set by the builder
///      from the original tree-sitter kind), look it up in `PyKind`,
///      fetch its `Rule` from `rules::rule`, and execute via the
///      shared [`crate::languages::rule::dispatch`].
///   2. Otherwise the node is a builder-inserted wrapper (e.g. the
///      `<name>` field wrapper) — dispatch by element name to a
///      transformation by the same name.
pub fn transform(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let kind_str = match get_kind(xot, node) {
        Some(k) => k,
        None => {
            let name = get_element_name(xot, node).unwrap_or_default();
            return match name.as_str() {
                "name" => super::transformations::name_wrapper(xot, node),
                _ => Ok(TransformAction::Continue),
            };
        }
    };

    let kind = match PyKind::from_str(&kind_str) {
        Some(k) => k,
        None => return Ok(TransformAction::Continue),
    };

    crate::languages::rule::dispatch(xot, node, super::rules::rule(kind))
}

/// Map a transformed element name to a syntax category for highlighting.
pub fn syntax_category(element: &str) -> SyntaxCategory {
    if let Some(spec) = super::semantic::spec(element) {
        return spec.syntax;
    }
    match element {
        // Builder-inserted wrappers / cross-cutting names not in NODES:
        "parameters" => SyntaxCategory::Keyword,
        _ if is_operator_marker(element) => SyntaxCategory::Operator,
        _ => SyntaxCategory::Default,
    }
}
