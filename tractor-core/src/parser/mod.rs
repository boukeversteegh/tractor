//! TreeSitter-based multi-language parser
//!
//! This module provides parsing capabilities for 22 programming languages,
//! converting source code into XML AST that can be queried with XPath.

pub mod config;
pub mod csharp;
pub mod typescript;
pub mod raw;
pub mod semantic;

use std::path::Path;
use std::fs;
use thiserror::Error;

pub use config::{LanguageConfig, DEFAULT_CONFIG};
pub use semantic::get_config;

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
];

/// Parse result containing the AST and source information
#[derive(Debug, Clone)]
pub struct ParseResult {
    /// The XML AST as a string (legacy, for compatibility)
    pub xml: String,
    /// Original source lines for location-based output
    pub source_lines: Vec<String>,
    /// File path or "<stdin>"
    pub file_path: String,
    /// Language used for parsing
    pub language: String,
}

/// Parse result with xot document for the new unified pipeline
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

/// Parse a file and return the XML AST
pub fn parse_file(path: &Path, lang_override: Option<&str>, raw_mode: bool) -> Result<ParseResult, ParseError> {
    let source = fs::read_to_string(path)?;
    let lang = lang_override.unwrap_or_else(|| detect_language(path.to_str().unwrap_or("")));
    parse_string(&source, lang, path.to_string_lossy().to_string(), raw_mode)
}

/// Parse a source string and return the XML AST
pub fn parse_string(source: &str, lang: &str, file_path: String, raw_mode: bool) -> Result<ParseResult, ParseError> {
    let language = get_tree_sitter_language(lang)?;

    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&language)
        .map_err(|e| ParseError::TreeSitter(e.to_string()))?;

    let tree = parser.parse(source, None)
        .ok_or_else(|| ParseError::Parse("Failed to parse source".to_string()))?;

    // Generate XML
    let mut xml_output = Vec::new();

    if raw_mode {
        raw::write_node(&mut xml_output, tree.root_node(), source, 0, false)
            .map_err(|e| ParseError::Parse(e.to_string()))?;
    } else {
        let config = get_config(lang);
        semantic::write_semantic_node(&mut xml_output, tree.root_node(), source, 0, false, config)
            .map_err(|e| ParseError::Parse(e.to_string()))?;
    }

    let xml = String::from_utf8(xml_output)
        .map_err(|e| ParseError::Parse(e.to_string()))?;

    Ok(ParseResult {
        xml,
        source_lines: source.lines().map(|s| s.to_string()).collect(),
        file_path,
        language: lang.to_string(),
    })
}

/// Generate full XML document with Files wrapper for multiple files
pub fn generate_xml_document(results: &[ParseResult]) -> String {
    let mut output = String::new();
    output.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    output.push('\n');
    output.push_str("<Files>\n");

    for result in results {
        output.push_str(&format!("  <File path=\"{}\">\n", escape_xml(&result.file_path)));
        // Indent each line of the XML by 4 spaces
        for line in result.xml.lines() {
            if !line.is_empty() {
                output.push_str("    ");
                output.push_str(line);
            }
            output.push('\n');
        }
        output.push_str("  </File>\n");
    }

    output.push_str("</Files>\n");
    output
}

// ============================================================================
// New xot-based pipeline
// ============================================================================

use crate::xot_builder::XotBuilder;

/// Parse a file and return an xot document (new pipeline)
pub fn parse_file_to_xot(path: &Path, lang_override: Option<&str>, raw_mode: bool) -> Result<XotParseResult, ParseError> {
    let source = fs::read_to_string(path)?;
    let lang = lang_override.unwrap_or_else(|| detect_language(path.to_str().unwrap_or("")));
    parse_string_to_xot(&source, lang, path.to_string_lossy().to_string(), raw_mode)
}

/// Parse a source string and return an xot document (new pipeline)
pub fn parse_string_to_xot(source: &str, lang: &str, file_path: String, raw_mode: bool) -> Result<XotParseResult, ParseError> {
    let language = get_tree_sitter_language(lang)?;

    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&language)
        .map_err(|e| ParseError::TreeSitter(e.to_string()))?;

    let tree = parser.parse(source, None)
        .ok_or_else(|| ParseError::Parse("Failed to parse source".to_string()))?;

    // Build xot document
    let mut builder = XotBuilder::new();

    let root = if raw_mode {
        builder.build_raw(tree.root_node(), source, &file_path)
            .map_err(|e| ParseError::Parse(e.to_string()))?
    } else {
        // TODO: Implement semantic mode for xot builder
        // For now, fall back to raw mode
        builder.build_raw(tree.root_node(), source, &file_path)
            .map_err(|e| ParseError::Parse(e.to_string()))?
    };

    Ok(XotParseResult {
        xot: builder.into_xot(),
        root,
        source_lines: source.lines().map(|s| s.to_string()).collect(),
        file_path,
        language: lang.to_string(),
    })
}

/// Escape XML special characters
pub fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
     .replace('<', "&lt;")
     .replace('>', "&gt;")
     .replace('"', "&quot;")
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
        let result = parse_string("public class Foo { }", "csharp", "<test>".to_string(), false).unwrap();
        assert!(result.xml.contains("<class"));
        assert!(result.xml.contains("Foo"));
    }
}
