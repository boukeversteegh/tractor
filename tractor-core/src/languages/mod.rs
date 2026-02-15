//! Language-specific transform modules
//!
//! Each language owns its complete transform logic.
//! The shared infrastructure (xot_transform) provides only the walker and helpers.

pub mod typescript;
pub mod csharp;
pub mod python;
pub mod go;
pub mod rust_lang;
pub mod java;
pub mod ruby;
pub mod json;
pub mod yaml;
pub mod toml;
pub mod ini;
pub mod env;
pub mod markdown;

use xot::{Xot, Node as XotNode};
use crate::xot_transform::TransformAction;
use crate::output::syntax_highlight::SyntaxCategory;

/// Type alias for language transform functions
pub type TransformFn = fn(&mut Xot, XotNode) -> Result<TransformAction, xot::Error>;

/// Type alias for syntax category mapping functions
/// Maps a transformed element name to a syntax category for highlighting
pub type SyntaxCategoryFn = fn(&str) -> SyntaxCategory;

/// Get the transform function for a language (single-branch transform)
///
/// For data-aware languages (JSON, YAML), prefer `get_data_transforms()` which
/// returns separate AST and data transforms for dual-branch output.
pub fn get_transform(lang: &str) -> TransformFn {
    match lang {
        "typescript" | "ts" | "tsx" | "javascript" | "js" | "jsx" => typescript::transform,
        "csharp" | "cs" => csharp::transform,
        "python" | "py" => python::transform,
        "go" => go::transform,
        "rust" | "rs" => rust_lang::transform,
        "java" => java::transform,
        "ruby" | "rb" => ruby::transform,
        "json" => json::data_transform,
        "yaml" | "yml" => yaml::data_transform,
        "toml" => toml::transform,
        "ini" => ini::transform,
        "env" => env::transform,
        "markdown" | "md" | "mdx" => markdown::transform,
        // Default: passthrough (no transforms)
        _ => passthrough_transform,
    }
}

/// Get dual-branch transform functions for data-aware languages.
///
/// Returns `Some((format, syntax_transform, data_transform))` for languages
/// that produce both a `/syntax` and `/data` branch, or `None` for other languages.
pub fn get_data_transforms(lang: &str) -> Option<(&'static str, TransformFn, TransformFn)> {
    match lang {
        "json" => Some(("json", json::ast_transform, json::data_transform)),
        "yaml" | "yml" => Some(("yaml", yaml::ast_transform, yaml::data_transform)),
        _ => None,
    }
}

/// Get the syntax category function for a language
/// This maps transformed element names to syntax categories for highlighting
pub fn get_syntax_category(lang: &str) -> SyntaxCategoryFn {
    match lang {
        "typescript" | "ts" | "tsx" | "javascript" | "js" | "jsx" => typescript::syntax_category,
        "csharp" | "cs" => csharp::syntax_category,
        "python" | "py" => python::syntax_category,
        "go" => go::syntax_category,
        "rust" | "rs" => rust_lang::syntax_category,
        "java" => java::syntax_category,
        "ruby" | "rb" => ruby::syntax_category,
        "json" => json::syntax_category,
        "yaml" | "yml" => yaml::syntax_category,
        "toml" => toml::syntax_category,
        "ini" => ini::syntax_category,
        "env" => env::syntax_category,
        "markdown" | "md" | "mdx" => markdown::syntax_category,
        // Default: generic fallback
        _ => default_syntax_category,
    }
}

/// Default passthrough transform - just continues without changes
fn passthrough_transform(_xot: &mut Xot, _node: XotNode) -> Result<TransformAction, xot::Error> {
    Ok(TransformAction::Continue)
}

/// Default syntax category - generic fallback for unknown languages
fn default_syntax_category(element: &str) -> SyntaxCategory {
    // Fallback to the generic mapping in syntax_highlight.rs
    SyntaxCategory::from_element_name(element)
}
