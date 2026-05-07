//! INI language module.
//!
//! INI runs through `crate::ir::ini_data` end-to-end. The legacy
//! imperative `rules.rs` / `transformations.rs` / `transform.rs`
//! modules have been retired.

pub mod input;
pub mod output;

use crate::output::syntax_highlight::SyntaxCategory;

/// Map a transformed element name to a syntax category for highlighting.
pub fn syntax_category(element: &str) -> SyntaxCategory {
    match element {
        "comment" => SyntaxCategory::Comment,
        _ => SyntaxCategory::Default,
    }
}
