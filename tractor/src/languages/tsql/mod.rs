//! T-SQL language module.
//!
//!   - [`lower`]  — CST → `SqlTree` lowering.
//!   - [`kinds`]  — generated `TsqlKind` enum (the input vocabulary).
//!   - [`output`] — semantic-name constants and `NODES`.

pub mod lower;
pub mod kinds;
pub mod output;

pub use lower::lower_sql_root;

use crate::output::syntax_highlight::SyntaxCategory;
use crate::transform::operators::is_operator_marker;

/// Map a transformed element name to a syntax category for highlighting.
pub fn syntax_category(element: &str) -> SyntaxCategory {
    if let Some(spec) = output::spec(element) {
        return spec.syntax;
    }
    match element {
        "order_by" | "group_by" => SyntaxCategory::Keyword,
        "create_table" | "create_function" => SyntaxCategory::Keyword,
        _ if is_operator_marker(element) => SyntaxCategory::Operator,
        _ => SyntaxCategory::Default,
    }
}
