//! Rust language pipeline.
//!
//! Rust runs through `crate::ir::rust_lang` end-to-end. The legacy
//! imperative `transform` / `rules` / `transformations` modules have
//! been retired (the parser dispatches to `parse_with_ir_pipeline`
//! for Rust, so they were unreachable). Only the IR-pipeline polish
//! layer remains:
//!
//!   - [`input`]    — generated `RustKind` enum, kept as a kind-coverage
//!                    catalogue for `tests/kind_catalogue.rs`.
//!   - [`output`]   — `TractorNode` enum + spec table.
//!   - [`post_transform`] — chain inversion, use restructure, lifetime-
//!                          name normalize, list distribution — runs
//!                          after IR rendering.

use crate::output::syntax_highlight::SyntaxCategory;
use crate::transform::operators::is_operator_marker;

pub mod input;
pub mod output;
pub mod post_transform;

pub use post_transform::rust_post_transform;

/// Map a transformed element name to a syntax category for highlighting.
pub fn syntax_category(element: &str) -> SyntaxCategory {
    if let Some(spec) = output::spec(element) {
        return spec.syntax;
    }
    match element {
        "parameters" => SyntaxCategory::Keyword,
        _ if is_operator_marker(element) => SyntaxCategory::Operator,
        _ => SyntaxCategory::Default,
    }
}
