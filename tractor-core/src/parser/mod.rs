//! TreeSitter-based multi-language parser
//!
//! This module provides parsing capabilities for 22 programming languages,
//! converting source code into XML AST that can be queried with XPath.

pub mod config;
pub mod raw;

// Re-export languages for compatibility
pub use crate::languages;

use std::path::Path;
use std::fs;
use thiserror::Error;

/// Supported languages and their extensions
pub static SUPPORTED_LANGUAGES: &[(&str, &[&str])] = &[
    ("csharp", &["cs"]),
    ("rust", &["rs"]),
    ("javascript", &["js", "mjs", "cjs", "jsx"]),
    ("typescript", &["ts", "tsx"]),
    ("python", &["py", "pyw", "pyi"]),
    ("go", &["go"]),
    ("java", &["java"]),
    ("ruby", &["rb", "rake", "gemspec"]),
    ("cpp", &["cpp", "cc", "cxx", "hpp", "hxx", "hh"]),
    ("c", &["c", "h"]),
    ("json", &["json"]),
    ("html", &["html", "htm"]),
    ("css", &["css"]),
    ("bash", &["sh", "bash"]),
    ("yaml", &["yml", "yaml"]),
    ("php", &["php"]),
    ("scala", &["scala", "sc"]),
    ("lua", &["lua"]),
    ("haskell", &["hs", "lhs"]),
    ("ocaml", &["ml", "mli"]),
    ("r", &["r"]),
    ("julia", &["jl"]),
    // XML pass-through (not parsed, queried directly)
    ("xml", &["xml"]),
];

/// Parse result with xot document
pub struct XotParseResult {
    /// The xot document containing the AST
    pub xot: xot::Xot,
    /// Root node of the document
    pub root: xot::Node,
    /// Original source lines for location-based output
    pub source_lines: Vec<String>,
    /// File path or "<stdin>"
    pub file_path: String,
    /// Language used for parsing
    pub language: String,
}

/// Errors that can occur during parsing
#[derive(Error, Debug)]
pub enum ParseError {
    #[error("Unsupported language: {0}")]
    UnsupportedLanguage(String),
    #[error("Failed to read file: {0}")]
    FileRead(#[from] std::io::Error),
    #[error("Failed to parse: {0}")]
    Parse(String),
    #[error("TreeSitter error: {0}")]
    TreeSitter(String),
}

/// Detect language from file path extension
pub fn detect_language(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("");
    match ext.to_lowercase().as_str() {
        "cs" => "csharp",
        "rs" => "rust",
        "js" | "mjs" | "cjs" | "jsx" => "javascript",
        "ts" | "tsx" => "typescript",
        "py" | "pyw" | "pyi" => "python",
        "go" => "go",
        "java" => "java",
        "rb" | "rake" | "gemspec" => "ruby",
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" | "hh" => "cpp",
        "c" | "h" => "c",
        "json" => "json",
        "html" | "htm" => "html",
        "css" => "css",
        "sh" | "bash" => "bash",
        "yml" | "yaml" => "yaml",
        "php" => "php",
        "scala" | "sc" => "scala",
        "lua" => "lua",
        "hs" | "lhs" => "haskell",
        "ml" | "mli" => "ocaml",
        "r" => "r",
        "jl" => "julia",
        "xml" => "xml",
        _ => "unknown",
    }
}

/// Get TreeSitter language for a language name
fn get_tree_sitter_language(lang: &str) -> Result<tree_sitter::Language, ParseError> {
    match lang {
        "csharp" | "cs" => Ok(tree_sitter_c_sharp::LANGUAGE.into()),
        "rust" | "rs" => Ok(tree_sitter_rust::LANGUAGE.into()),
        "javascript" | "js" => Ok(tree_sitter_javascript::LANGUAGE.into()),
        "typescript" | "ts" | "tsx" => Ok(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        "python" | "py" => Ok(tree_sitter_python::LANGUAGE.into()),
        "go" => Ok(tree_sitter_go::LANGUAGE.into()),
        "java" => Ok(tree_sitter_java::LANGUAGE.into()),
        "ruby" | "rb" => Ok(tree_sitter_ruby::LANGUAGE.into()),
        "cpp" | "c++" => Ok(tree_sitter_cpp::LANGUAGE.into()),
        "c" => Ok(tree_sitter_c::LANGUAGE.into()),
        "json" => Ok(tree_sitter_json::LANGUAGE.into()),
        "html" | "htm" => Ok(tree_sitter_html::LANGUAGE.into()),
        "css" => Ok(tree_sitter_css::LANGUAGE.into()),
        "bash" | "sh" => Ok(tree_sitter_bash::LANGUAGE.into()),
        "yaml" | "yml" => Ok(tree_sitter_yaml::LANGUAGE.into()),
        "php" => Ok(tree_sitter_php::LANGUAGE_PHP.into()),
        "scala" => Ok(tree_sitter_scala::LANGUAGE.into()),
        "lua" => Ok(tree_sitter_lua::LANGUAGE.into()),
        "haskell" | "hs" => Ok(tree_sitter_haskell::LANGUAGE.into()),
        "ocaml" | "ml" => Ok(tree_sitter_ocaml::LANGUAGE_OCAML.into()),
        "r" => Ok(tree_sitter_r::LANGUAGE.into()),
        "julia" | "jl" => Ok(tree_sitter_julia::LANGUAGE.into()),
        _ => Err(ParseError::UnsupportedLanguage(lang.to_string())),
    }
}

// ============================================================================
// Xot-based pipeline
// ============================================================================

use crate::xot_builder::{XotBuilder, XeeBuilder};
use xee_xpath::{Documents, DocumentHandle};

/// Parse a file and return an xot document (new pipeline)
pub fn parse_file_to_xot(path: &Path, lang_override: Option<&str>, raw_mode: bool) -> Result<XotParseResult, ParseError> {
    parse_file_to_xot_with_options(path, lang_override, raw_mode, false)
}

/// Parse a file and return an xot document with options (new pipeline)
pub fn parse_file_to_xot_with_options(
    path: &Path,
    lang_override: Option<&str>,
    raw_mode: bool,
    ignore_whitespace: bool,
) -> Result<XotParseResult, ParseError> {
    let source = fs::read_to_string(path)?;
    let lang = lang_override.unwrap_or_else(|| detect_language(path.to_str().unwrap_or("")));
    parse_string_to_xot_with_options(&source, lang, path.to_string_lossy().to_string(), raw_mode, ignore_whitespace)
}

/// Parse a source string and return an xot document (new pipeline)
pub fn parse_string_to_xot(source: &str, lang: &str, file_path: String, raw_mode: bool) -> Result<XotParseResult, ParseError> {
    parse_string_to_xot_with_options(source, lang, file_path, raw_mode, false)
}

/// Parse a source string and return an xot document with options (new pipeline)
pub fn parse_string_to_xot_with_options(
    source: &str,
    lang: &str,
    file_path: String,
    raw_mode: bool,
    ignore_whitespace: bool,
) -> Result<XotParseResult, ParseError> {
    let language = get_tree_sitter_language(lang)?;

    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&language)
        .map_err(|e| ParseError::TreeSitter(e.to_string()))?;

    let tree = parser.parse(source, None)
        .ok_or_else(|| ParseError::Parse("Failed to parse source".to_string()))?;

    // Build xot document (always start with raw tree)
    let mut builder = XotBuilder::new();
    let root = builder.build_raw_with_options(tree.root_node(), source, &file_path, ignore_whitespace)
        .map_err(|e| ParseError::Parse(e.to_string()))?;

    let mut xot = builder.into_xot();

    // Apply semantic transforms if not in raw mode
    if !raw_mode {
        let transform_fn = languages::get_transform(lang);
        crate::xot_transform::walk_transform(&mut xot, root, transform_fn)
            .map_err(|e| ParseError::Parse(e.to_string()))?;
    }

    Ok(XotParseResult {
        xot,
        root,
        source_lines: source.lines().map(|s| s.to_string()).collect(),
        file_path,
        language: lang.to_string(),
    })
}

/// Parse result for the fast query path (builds directly into Documents)
pub struct XeeParseResult {
    /// The Documents instance containing the AST
    pub documents: Documents,
    /// Handle to the document for querying
    pub doc_handle: DocumentHandle,
    /// Original source lines for location-based output
    pub source_lines: Vec<String>,
    /// File path or "<stdin>"
    pub file_path: String,
    /// Language used for parsing
    pub language: String,
}

/// Parse a source string directly into Documents for fast XPath queries
///
/// This is the fast path that avoids XML serialization/parsing roundtrip.
/// Returns an XeeParseResult that can be queried with XPathEngine::query_documents().
pub fn parse_string_to_xee(
    source: &str,
    lang: &str,
    file_path: String,
    raw_mode: bool,
) -> Result<XeeParseResult, ParseError> {
    parse_string_to_xee_with_options(source, lang, file_path, raw_mode, false)
}

/// Parse a source string directly into Documents with options
///
/// This is the fast path that avoids XML serialization/parsing roundtrip.
/// Returns an XeeParseResult that can be queried with XPathEngine::query_documents().
/// Use `ignore_whitespace=true` to strip whitespace from text nodes during tree building.
pub fn parse_string_to_xee_with_options(
    source: &str,
    lang: &str,
    file_path: String,
    raw_mode: bool,
    ignore_whitespace: bool,
) -> Result<XeeParseResult, ParseError> {
    let language = get_tree_sitter_language(lang)?;

    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&language)
        .map_err(|e| ParseError::TreeSitter(e.to_string()))?;

    let tree = parser.parse(source, None)
        .ok_or_else(|| ParseError::Parse("Failed to parse source".to_string()))?;

    // Build directly into Documents using XeeBuilder
    let mut builder = XeeBuilder::new();
    let doc_handle = builder.build_with_options(tree.root_node(), source, &file_path, lang, raw_mode, ignore_whitespace)
        .map_err(|e| ParseError::Parse(e.to_string()))?;

    let documents = builder.into_documents();

    Ok(XeeParseResult {
        documents,
        doc_handle,
        source_lines: source.lines().map(|s| s.to_string()).collect(),
        file_path,
        language: lang.to_string(),
    })
}

/// Parse a file directly into Documents for fast XPath queries
pub fn parse_file_to_xee(
    path: &Path,
    lang_override: Option<&str>,
    raw_mode: bool,
) -> Result<XeeParseResult, ParseError> {
    parse_file_to_xee_with_options(path, lang_override, raw_mode, false)
}

/// Parse a file directly into Documents with options
pub fn parse_file_to_xee_with_options(
    path: &Path,
    lang_override: Option<&str>,
    raw_mode: bool,
    ignore_whitespace: bool,
) -> Result<XeeParseResult, ParseError> {
    let source = fs::read_to_string(path)?;
    let lang = lang_override.unwrap_or_else(|| detect_language(path.to_str().unwrap_or("")));
    parse_string_to_xee_with_options(&source, lang, path.to_string_lossy().to_string(), raw_mode, ignore_whitespace)
}

// ============================================================================
// Unified parsing pipeline - always returns Documents
// ============================================================================

/// Load XML string directly into Documents for querying
///
/// This is the XML passthrough path - no TreeSitter parsing, just load the XML.
pub fn load_xml_string_to_documents(xml: &str, file_path: String) -> Result<XeeParseResult, ParseError> {
    let mut documents = Documents::new();

    // Parse XML directly into Documents
    let doc_handle = documents.add_string(
        "file:///source".try_into().unwrap(),
        xml,
    ).map_err(|e| ParseError::Parse(e.to_string()))?;

    Ok(XeeParseResult {
        documents,
        doc_handle,
        source_lines: Vec::new(), // XML passthrough doesn't have source lines
        file_path,
        language: "xml".to_string(),
    })
}

/// Load XML file directly into Documents for querying
pub fn load_xml_file_to_documents(path: &Path) -> Result<XeeParseResult, ParseError> {
    let xml = fs::read_to_string(path)?;
    load_xml_string_to_documents(&xml, path.to_string_lossy().to_string())
}

/// Unified parse function - returns Documents regardless of input type
///
/// This is the main entry point for parsing. It handles:
/// - XML files: loaded directly into Documents (passthrough)
/// - Source code: parsed with TreeSitter, built into Documents
///
/// The result can always be queried with `XPathEngine::query_documents()`.
pub fn parse_to_documents(
    path: &Path,
    lang_override: Option<&str>,
    raw_mode: bool,
    ignore_whitespace: bool,
) -> Result<XeeParseResult, ParseError> {
    let lang = lang_override.unwrap_or_else(|| detect_language(path.to_str().unwrap_or("")));

    if lang == "xml" {
        // XML passthrough: load directly into Documents
        load_xml_file_to_documents(path)
    } else {
        // Source code: TreeSitter → XeeBuilder → Documents
        parse_file_to_xee_with_options(path, lang_override, raw_mode, ignore_whitespace)
    }
}

/// Unified parse function for strings
pub fn parse_string_to_documents(
    source: &str,
    lang: &str,
    file_path: String,
    raw_mode: bool,
    ignore_whitespace: bool,
) -> Result<XeeParseResult, ParseError> {
    if lang == "xml" {
        // XML passthrough
        load_xml_string_to_documents(source, file_path)
    } else {
        // Source code: TreeSitter → XeeBuilder → Documents
        parse_string_to_xee_with_options(source, lang, file_path, raw_mode, ignore_whitespace)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_language() {
        assert_eq!(detect_language("foo.cs"), "csharp");
        assert_eq!(detect_language("foo.rs"), "rust");
        assert_eq!(detect_language("foo.py"), "python");
        assert_eq!(detect_language("foo.js"), "javascript");
        assert_eq!(detect_language("foo.unknown"), "unknown");
    }

    #[test]
    fn test_parse_simple_class() {
        use crate::output::{render_node, RenderOptions};

        let result = parse_string_to_documents(
            "public class Foo { }", "csharp", "<test>".to_string(), false, false
        ).unwrap();

        let doc_node = result.documents.document_node(result.doc_handle).unwrap();
        let xot = result.documents.xot();
        let xml: String = xot.children(doc_node)
            .map(|child| render_node(xot, child, &RenderOptions::new()))
            .collect();

        assert!(xml.contains("<class"), "Should contain class element");
        assert!(xml.contains("Foo"), "Should contain Foo");
    }
}
