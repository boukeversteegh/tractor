//! INI transform logic.
//!
//! Per-language pipeline ownership:
//!
//! ```text
//! input → rules → output
//!         ↑
//!         transformations (Custom handlers)
//! ```
//!
//! - [`input`]    — generated `IniKind` enum (the input vocabulary).
//!                  Regenerate via `task gen:kinds`.
//! - [`output`]   — output element-name constants. Section/setting
//!                  names are user-driven (open vocabulary), so only
//!                  `<comment>` is named here.
//! - [`rules`]    — `rule(IniKind) -> Rule` exhaustive match.
//! - [`transformations`] — `Rule::Custom` handlers.
//! - [`transform`] — thin orchestrator.
//!
//! Maps the INI data structure to XML elements:
//! ```ini
//! [database]
//! host = localhost
//! ```
//! becomes:
//! ```xml
//! <database>
//!   <host>localhost</host>
//! </database>
//! ```
//! Queryable as: `//database/host[.='localhost']`.

pub mod input;
pub mod output;
pub mod rules;
pub mod transformations;
pub mod transform;

pub use transform::transform;

use crate::output::syntax_highlight::SyntaxCategory;

/// Map a transformed element name to a syntax category for highlighting.
pub fn syntax_category(element: &str) -> SyntaxCategory {
    match element {
        "comment" => SyntaxCategory::Comment,
        _ => SyntaxCategory::Default,
    }
}
