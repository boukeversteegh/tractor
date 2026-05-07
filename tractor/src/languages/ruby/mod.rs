//! Ruby language pipeline.
//!
//! Ruby runs through `crate::ir::ruby` end-to-end. The legacy
//! imperative `transform` / `rules` / `transformations` modules have
//! been retired (the parser dispatches to `parse_with_ir_pipeline`
//! for Ruby, so they were unreachable). Only the IR-pipeline polish
//! layer remains:
//!
//!   - [`input`]    — generated `RubyKind` enum, kept as a kind-coverage
//!                    catalogue for `tests/kind_catalogue.rs`.
//!   - [`output`]   — `TractorNode` enum + spec table.
//!   - [`post_transform`] — list-distribute + chain-inversion + slot-
//!                          wrapping passes that run after IR rendering.

use crate::output::syntax_highlight::SyntaxCategory;

pub mod input;
pub mod output;
pub mod post_transform;

pub use post_transform::ruby_post_transform;

/// Map a transformed element name to a syntax category for highlighting.
pub fn syntax_category(element: &str) -> SyntaxCategory {
    if let Some(spec) = output::spec(element) {
        return spec.syntax;
    }
    match element {
        "type" => SyntaxCategory::Type,
        "raise" | "return" => SyntaxCategory::Keyword,
        "def" | "end" | "super" => SyntaxCategory::Keyword,
        _ => SyntaxCategory::Default,
    }
}
