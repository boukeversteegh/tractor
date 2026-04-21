//! Language-specific transform modules and metadata.
//!
//! Each language owns its complete transform logic.
//! The shared infrastructure (xot_transform) provides only the walker and helpers.

pub mod info;
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
pub mod tsql;

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
        "tsql" | "mssql" => tsql::transform,
        // Default: passthrough (no transforms)
        _ => passthrough_transform,
    }
}

// /specs/tractor-parse/dual-view/supported-languages.md: Supported Languages
/// Get dual-branch transform functions for data-aware languages.
///
/// Returns `Some((syntax_transform, data_transform))` for languages
/// that produce both a `/syntax` and `/data` branch, or `None` for other languages.
pub fn get_data_transforms(lang: &str) -> Option<(TransformFn, TransformFn)> {
    match lang {
        "json" => Some((json::ast_transform, json::data_transform)),
        "yaml" | "yml" => Some((yaml::ast_transform, yaml::data_transform)),
        _ => None,
    }
}

/// Check whether a language supports the data tree projection.
pub fn supports_data_tree(lang: &str) -> bool {
    matches!(lang, "json" | "yaml" | "yml")
}

/// True for programming languages (as opposed to data/config languages).
/// Used to gate post-transforms like identifier-role marking that only
/// make sense when the tree has declaration/reference semantics.
pub fn is_programming_language(lang: &str) -> bool {
    matches!(
        lang,
        "typescript" | "ts" | "tsx"
            | "javascript" | "js" | "jsx"
            | "csharp" | "cs"
            | "python" | "py"
            | "go"
            | "rust" | "rs"
            | "java"
            | "ruby" | "rb"
            | "tsql" | "mssql"
    )
}

/// Default field wrappings shared by most programming-language grammars.
/// Each language opts in (and can add language-specific entries) via
/// `get_field_wrappings`.
const COMMON_FIELD_WRAPPINGS: &[(&str, &str)] = &[
    ("name", "name"),
    ("value", "value"),
    ("left", "left"),
    ("right", "right"),
    ("body", "body"),
    ("condition", "condition"),
    ("consequence", "consequence"),
    ("alternative", "alternative"),
];

const TS_FIELD_WRAPPINGS: &[(&str, &str)] = &[
    ("name", "name"),
    ("value", "value"),
    ("left", "left"),
    ("right", "right"),
    ("body", "body"),
    ("condition", "condition"),
    ("consequence", "consequence"),
    ("alternative", "alternative"),
    ("return_type", "returns"),
];

const RUST_FIELD_WRAPPINGS: &[(&str, &str)] = &[
    ("name", "name"),
    ("value", "value"),
    ("left", "left"),
    ("right", "right"),
    ("body", "body"),
    ("condition", "condition"),
    ("consequence", "consequence"),
    ("alternative", "alternative"),
    ("return_type", "returns"),
];

const GO_FIELD_WRAPPINGS: &[(&str, &str)] = &[
    ("name", "name"),
    ("value", "value"),
    ("left", "left"),
    ("right", "right"),
    ("body", "body"),
    ("condition", "condition"),
    ("consequence", "consequence"),
    ("alternative", "alternative"),
    ("result", "returns"),
];

const CSHARP_FIELD_WRAPPINGS: &[(&str, &str)] = &[
    ("name", "name"),
    ("value", "value"),
    ("left", "left"),
    ("right", "right"),
    ("body", "body"),
    ("condition", "condition"),
    ("consequence", "consequence"),
    ("alternative", "alternative"),
    ("returns", "returns"),
];

/// Field wrappings for the given language — applied after the raw
/// builder pass, before the per-language transform. Programming
/// languages with language-specific mappings override; everything else
/// (including data/config formats) gets the common defaults, since
/// JSON/YAML/TOML data transforms still rely on the `<value>` wrapper
/// for pair values.
pub fn get_field_wrappings(lang: &str) -> &'static [(&'static str, &'static str)] {
    match lang {
        "typescript" | "ts" | "tsx" | "javascript" | "js" | "jsx" => TS_FIELD_WRAPPINGS,
        "rust" | "rs" => RUST_FIELD_WRAPPINGS,
        "go" => GO_FIELD_WRAPPINGS,
        "csharp" | "cs" => CSHARP_FIELD_WRAPPINGS,
        _ => COMMON_FIELD_WRAPPINGS,
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
        "tsql" | "mssql" => tsql::syntax_category,
        // Default: generic fallback
        _ => default_syntax_category,
    }
}

/// Get the singleton wrapper list for a language.
///
/// Returns the list of wrapper element names that should have their single
/// child annotated with `field` for JSON property lifting.
/// Data-aware languages (JSON, YAML) return an empty list.
pub fn get_singleton_wrappers(lang: &str) -> &'static [&'static str] {
    use crate::xot_transform::helpers::DEFAULT_SINGLETON_WRAPPERS;
    match lang {
        // Data languages don't have singleton wrappers
        "json" | "yaml" | "yml" | "toml" | "ini" | "env" | "markdown" | "md" | "mdx" => &[],
        // All programming languages use the default list
        _ => DEFAULT_SINGLETON_WRAPPERS,
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
