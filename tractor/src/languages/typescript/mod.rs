//! TypeScript / JavaScript / TSX / JSX language pipeline.
//!
//! All four flavours run through `crate::ir::typescript` end-to-end.
//!
//!   - [`input`]    — generated `TsKind` enum (union of typescript +
//!                    tsx grammars), kept as a kind-coverage
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
        "parameters" => SyntaxCategory::Keyword,
        "ref" => SyntaxCategory::Identifier,
        "true" | "false" => SyntaxCategory::Keyword,
        "do" => SyntaxCategory::Keyword,
        "typeof" => SyntaxCategory::Type,
        _ if is_operator_marker(element) => SyntaxCategory::Operator,
        _ => SyntaxCategory::Default,
    }
}
