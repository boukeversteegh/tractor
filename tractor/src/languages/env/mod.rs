//! .env file transform logic.
//!
//! Per-language pipeline ownership:
//!
//! ```text
//! input → rules → output
//!         ↑
//!         transformations (Custom handlers)
//! ```
//!
//! - [`input`]    — hand-curated `EnvKind` enum (NOT generated). .env
//!                  files are parsed by tree-sitter-bash, but the
//!                  transform handles only ~15 of bash's 59 kinds; the
//!                  rest fall through the orchestrator as no-ops.
//! - [`output`]   — output element-name constants. Variable names
//!                  become user-driven, so only `<document>` and
//!                  `<comment>` are closed names.
//! - [`rules`]    — `rule(EnvKind) -> Rule` exhaustive match.
//! - [`transformations`] — `Rule::Custom` handlers + value-extraction
//!                          helpers (`collect_value_text` walks the
//!                          bash subtree to reassemble concatenated /
//!                          quoted / expanded values).
//! - [`transform`] — thin orchestrator.
//!
//! Example transform:
//! ```env
//! # Database config
//! DB_HOST=localhost
//! export API_URL=https://example.com
//! ```
//! becomes:
//! ```xml
//! <document>
//!   <comment>Database config</comment>
//!   <DB_HOST>localhost</DB_HOST>
//!   <API_URL>https://example.com</API_URL>
//! </document>
//! ```

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
