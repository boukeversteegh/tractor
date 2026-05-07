//! TypeScript / JavaScript / TSX / JSX language pipeline.
//!
//! All four flavours run through `crate::ir::typescript` end-to-end.
//! The legacy imperative `transform` / `rules` / `transformations`
//! modules have been retired (the parser dispatches to
//! `parse_with_ir_pipeline` for ts/js/tsx/jsx, so they were
//! unreachable). Only the IR-pipeline polish layer remains:
//!
//!   - [`input`]    — generated `TsKind` enum (union of typescript +
//!                    tsx grammars), kept as a kind-coverage
//!                    catalogue for `tests/kind_catalogue.rs`.
//!   - [`output`]   — `TractorNode` enum + spec table.
//!   - [`post_transform`] — list-distribute + chain-inversion + slot-
//!                          wrapping passes that run after IR rendering.

use crate::output::syntax_highlight::SyntaxCategory;
use crate::transform::operators::is_operator_marker;

pub mod input;
pub mod output;
pub mod post_transform;

pub use post_transform::typescript_post_transform;

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
