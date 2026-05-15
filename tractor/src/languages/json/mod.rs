//! JSON language module.
//!
//! After S6B-Retire, this module only carries the bits the typed
//! pipeline still consults: the kind enum (used by the kind-coverage
//! catalogue test), the output element-name table, and the syntax-
//! highlighting category fn. Lowering happens through
//! `crate::tree::data::lower_json`; rendering through
//! `crate::tree::data::to_xot`. The legacy imperative `syntax_transform`
//! / `data_transform` / `rules` / `transformations` modules were
//! deleted alongside `walk_transform`.

pub mod kinds;
pub mod output;

use crate::output::syntax_highlight::SyntaxCategory;

/// Map element names to syntax categories for highlighting
pub fn syntax_category(element: &str) -> SyntaxCategory {
    match element {
        "object" | "array" => SyntaxCategory::Keyword,
        "string" => SyntaxCategory::String,
        "number" => SyntaxCategory::Number,
        "bool" | "null" => SyntaxCategory::Keyword,
        "property" | "key" | "value" => SyntaxCategory::Default,
        "item" => SyntaxCategory::Keyword,
        _ => SyntaxCategory::Default,
    }
}
