//! T-SQL transform logic — thin dispatcher driven by `rules::rule`.

use xot::{Xot, Node as XotNode};
use crate::transform::{TransformAction, helpers::*};
use crate::transform::operators::is_operator_marker;
use crate::output::syntax_highlight::SyntaxCategory;

use super::input::TsqlKind;

/// Transform a T-SQL AST node.
///
/// Dispatch is split in two:
///   1. If the node carries a `kind` attribute, look it up in
///      `TsqlKind`, fetch its `Rule` from `rules::rule`, and execute
///      via the shared [`crate::languages::rule::dispatch`].
///   2. Otherwise the node is a builder-inserted field wrapper —
///      dispatch by element name. T-SQL has several:
///        - `<name>` → identifier-flavored handler with bracket
///          stripping and @var detection.
///        - `<value>` / `<left>` / `<right>` → field wrappers tsql
///          doesn't need around expressions; skip so the inner
///          expression bubbles up.
pub fn transform(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let kind_str = match get_kind(xot, node) {
        Some(k) => k,
        None => {
            let name = get_element_name(xot, node).unwrap_or_default();
            return match name.as_str() {
                "value" | "left" | "right" => super::transformations::skip(xot, node),
                "name" => super::transformations::name_wrapper(xot, node),
                _ => Ok(TransformAction::Continue),
            };
        }
    };

    let kind = match TsqlKind::from_str(&kind_str) {
        Some(k) => k,
        None => return Ok(TransformAction::Continue),
    };

    crate::languages::rule::dispatch(xot, node, super::rules::rule(kind))
}

/// Map a transformed element name to a syntax category for highlighting.
pub fn syntax_category(element: &str) -> SyntaxCategory {
    if let Some(spec) = super::output::spec(element) {
        return spec.syntax;
    }
    match element {
        "order_by" | "group_by" => SyntaxCategory::Keyword,
        "create_table" | "create_function" => SyntaxCategory::Keyword,
        _ if is_operator_marker(element) => SyntaxCategory::Operator,
        _ => SyntaxCategory::Default,
    }
}
