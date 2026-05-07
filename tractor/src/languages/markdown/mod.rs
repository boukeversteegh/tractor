//! Markdown language module.
//!
//! Markdown runs through `crate::ir::markdown_data` end-to-end. The
//! legacy imperative `rules.rs` / `transformations.rs` /
//! `transform.rs` modules have been retired.
//!
//!   - [`input`]   — generated `MdKind` enum, kept as a kind-coverage
//!                   catalogue.
//!   - [`output`]  — element-name vocabulary used by syntax_category.

pub mod input;
pub mod output;

use crate::output::syntax_highlight::SyntaxCategory;

/// Map a transformed element name to a syntax category for highlighting.
pub fn syntax_category(element: &str) -> SyntaxCategory {
    match element {
        // Headings
        "heading" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => SyntaxCategory::Keyword,

        // Code
        "codeblock" | "code" | "language" => SyntaxCategory::String,

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
