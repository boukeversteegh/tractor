//! Rust language pipeline.
//!
//! Rust runs through `crate::tree::rust_lang` end-to-end.
//!
//!   - [`input`]    — generated `RustKind` enum, kept as a kind-coverage
//!                    catalogue for `tests/kind_catalogue.rs`.
//!   - [`output`]   — `TractorNode` enum + spec table.

use crate::output::syntax_highlight::SyntaxCategory;
use crate::transform::operators::is_operator_marker;

pub mod lower;
#[cfg(feature = "native")]
pub mod render_source;
pub mod kinds;
pub mod output;

pub use lower::{lower_rust_root, lower_rust_node};

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
