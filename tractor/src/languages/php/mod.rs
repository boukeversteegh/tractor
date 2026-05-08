//! PHP language pipeline.
//!
//! PHP flows through `crate::ir::php` end-to-end.
//!
//!   - [`input`]    — generated `PhpKind` enum, kept as a kind-coverage
//!                    catalogue for `tests/kind_catalogue.rs`.
//!   - [`output`]   — `TractorNode` enum + spec table.

use crate::output::syntax_highlight::SyntaxCategory;
use crate::transform::operators::is_operator_marker;

pub mod input;
pub mod output;

/// Map a transformed element name to a syntax category for highlighting.
pub fn syntax_category(element: &str) -> SyntaxCategory {
    if let Some(spec) = output::spec(element) {
        return spec.syntax;
    }
    match element {
        // Builder wrappers / non-spec names that still need a category.
        "parameters" | "arguments" => SyntaxCategory::Keyword,
        _ if is_operator_marker(element) => SyntaxCategory::Operator,
        _ => SyntaxCategory::Default,
    }
}
