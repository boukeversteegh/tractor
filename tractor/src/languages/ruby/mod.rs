//! Ruby language pipeline.
//!
//! Ruby runs through `crate::tree::ruby` end-to-end.
//!
//!   - [`input`]    — generated `RubyKind` enum, kept as a kind-coverage
//!                    catalogue for `tests/kind_catalogue.rs`.
//!   - [`output`]   — `TractorNode` enum + spec table.

use crate::output::syntax_highlight::SyntaxCategory;

#[cfg(feature = "native")]
pub mod lower;
#[cfg(feature = "native")]
pub mod render_source;
pub mod kinds;
pub mod output;

#[cfg(feature = "native")]
pub use lower::{lower_ruby_root, lower_ruby_node};

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
