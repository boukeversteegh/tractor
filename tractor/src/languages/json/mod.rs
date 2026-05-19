//! JSON language module.
//!
//! Symmetric with code-language modules (python/csharp/…): hosts the
//! kind enum, element-name vocabulary, CST → DataTree lowering, and
//! the DataTree → JSON source-text renderer. Cross-format rendering
//! to xot/serde_json::Value lives shared at `crate::tree::data::*`.

pub mod kinds;
pub mod output;
#[cfg(feature = "native")]
pub mod lower;
#[cfg(feature = "native")]
pub mod render_source;

#[cfg(feature = "native")]
pub use lower::lower_json_data_root;

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
