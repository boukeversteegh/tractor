//! Markdown transform logic.
//!
//! Per-language pipeline ownership:
//!
//! ```text
//! input → rules → output
//!         ↑
//!         transformations (Custom handlers)
//! ```
//!
//! - [`input`]    — generated `MdKind` enum (the input vocabulary).
//!                  Union of the markdown block + inline grammars
//!                  (TS+TSX pattern). Regenerate via `task gen:kinds`.
//! - [`output`]   — output element-name constants (closed
//!                  vocabulary: `<heading>`, `<list>`, `<link>`, …).
//! - [`rules`]    — `rule(MdKind) -> Rule` exhaustive match. Most
//!                  kinds are `Rename` or `Detach`; only a handful
//!                  need `Custom`.
//! - [`transformations`] — `Custom` handlers (heading-level
//!                          detection, list-type detection, language
//!                          extraction).
//! - [`transform`] — thin orchestrator.

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
        // Headings
        "heading" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => SyntaxCategory::Keyword,

        // Code
        "code_block" | "code" | "language" => SyntaxCategory::String,

        // Emphasis
        "emphasis" | "strong" | "strikethrough" => SyntaxCategory::Identifier,

        // Links and images
        "link" | "image" => SyntaxCategory::Function,
        "destination" => SyntaxCategory::String,
        "label" | "title" => SyntaxCategory::String,

        // Lists
        "list" | "item" | "ordered" | "unordered" => SyntaxCategory::Default,
        "checked" | "unchecked" => SyntaxCategory::Keyword,

        // Block elements
        "blockquote" => SyntaxCategory::Comment,
        "hr" => SyntaxCategory::Operator,

        // Tables
        "table" | "thead" | "row" | "cell" => SyntaxCategory::Default,

        // HTML
        "html" => SyntaxCategory::Type,

        // Frontmatter
        "frontmatter" => SyntaxCategory::Comment,

        // LaTeX
        "latex" => SyntaxCategory::String,

        // Escape and entity
        "escape" | "entity" => SyntaxCategory::Operator,

        // Comments
        "comment" => SyntaxCategory::Comment,

        // Structural elements
        _ => SyntaxCategory::Default,
    }
}
