//! TOML language module.
//!
//! TOML runs through `crate::ir::toml_data` end-to-end. The legacy
//! imperative `rules.rs` / `transformations.rs` / `transform.rs`
//! modules have been retired.
//!
//!   - [`input`]   — generated `TomlKind` enum, kept as a kind-coverage
//!                   catalogue.
//!   - [`output`]  — element-name vocabulary used by syntax_category.

pub mod input;
pub mod output;

use crate::output::syntax_highlight::SyntaxCategory;

/// Map a transformed element name to a syntax category for highlighting.
pub fn syntax_category(element: &str) -> SyntaxCategory {
    match element {
        "item" => SyntaxCategory::Keyword,
        _ => SyntaxCategory::Default,
    }
}
