//! Ruby transform logic — thin dispatcher driven by `rules::rule`.

use xot::{Xot, Node as XotNode};
use crate::transform::{TransformAction, helpers::*};
use crate::output::syntax_highlight::SyntaxCategory;

use super::input::RubyKind;

/// Transform a Ruby AST node.
///
/// Dispatch is split in two:
///   1. If the node carries a `kind` attribute, look it up in
///      `RubyKind`, fetch its `Rule` from `rules::rule`, and execute
///      via the shared [`crate::languages::rule::dispatch`].
///   2. Otherwise the node is a builder-inserted wrapper — dispatch
///      by element name to a transformation function.
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

    let kind = match RubyKind::from_str(&kind_str) {
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
        "type" => SyntaxCategory::Type,
        "raise" | "return" => SyntaxCategory::Keyword,
        "def" | "end" | "super" => SyntaxCategory::Keyword,
        _ => SyntaxCategory::Default,
    }
}
