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

/// Parse a file and return the XML AST
pub fn parse_file(path: &Path, lang_override: Option<&str>, raw_mode: bool) -> Result<ParseResult, ParseError> {
    let source = fs::read_to_string(path)?;
    let lang = lang_override.unwrap_or_else(|| detect_language(path.to_str().unwrap_or("")));
    parse_string(&source, lang, path.to_string_lossy().to_string(), raw_mode)
}

/// Parse a source string and return the XML AST
///
/// This function now uses the xot-based pipeline internally:
/// AST → xot tree → transform → render to string
pub fn parse_string(source: &str, lang: &str, file_path: String, raw_mode: bool) -> Result<ParseResult, ParseError> {
    // Use the xot-based pipeline
    let result = parse_string_to_xot(source, lang, file_path.clone(), raw_mode)?;

    // Find the actual content node (skip Files/File wrappers)
    let content_node = find_content_root(&result.xot, result.root);

    // Render compact XML (no formatting whitespace) to preserve source whitespace
    // text nodes. Display code will re-render with pretty_print=true as needed.
    let options = crate::output::RenderOptions::new()
        .with_locations(true)
        .with_pretty_print(false);
    let xml = crate::output::render_node(&result.xot, content_node, &options);

    Ok(ParseResult {
        xml,
        source_lines: result.source_lines,
        file_path: result.file_path,
        language: result.language,
    })
}

/// Find the actual content root, skipping Files/File wrappers
fn find_content_root(xot: &xot::Xot, node: xot::Node) -> xot::Node {
    use crate::xot_transform::helpers::get_element_name;

    // If this is a document node, get the document element
    if xot.is_document(node) {
        if let Ok(elem) = xot.document_element(node) {
            return find_content_root(xot, elem);
        }
    }

    // Check if this is a Files or File wrapper
    if let Some(name) = get_element_name(xot, node) {
        if name == "Files" || name == "File" {
            // Return the first element child
            for child in xot.children(node) {
                if xot.element(child).is_some() {
                    return find_content_root(xot, child);
                }
            }
        }
    }

    node
}

/// Generate full XML document with Files wrapper for multiple files.
///
/// When `pretty_print` is true, includes indentation and newlines for readability.
/// When false, generates compact XML suitable for XPath queries where
/// formatting whitespace would corrupt string-value comparisons.
pub fn generate_xml_document(results: &[ParseResult], pretty_print: bool) -> String {
    let mut output = String::new();
    output.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    if pretty_print {
        output.push('\n');
    }

    if pretty_print {
        output.push_str("<Files>\n");
    } else {
        output.push_str("<Files>");
    }

    for result in results {
        if pretty_print {
            output.push_str(&format!("  <File path=\"{}\">\n", escape_xml(&result.file_path)));
            // Re-render the compact XML with pretty printing
            let pretty_xml = crate::output::render_xml_string(
                &result.xml,
                &crate::output::RenderOptions::new()
                    .with_locations(true)
                    .with_pretty_print(true),
            );
            // Indent each line by 4 spaces
            for line in pretty_xml.lines() {
                if !line.is_empty() {
                    output.push_str("    ");
                    output.push_str(line);
                }
                output.push('\n');
            }
            output.push_str("  </File>\n");
        } else {
            output.push_str(&format!("<File path=\"{}\">", escape_xml(&result.file_path)));
            // Use compact XML directly (already has no formatting whitespace)
            output.push_str(&result.xml);
            output.push_str("</File>");
        }
    }

    if pretty_print {
        output.push_str("</Files>\n");
    } else {
        output.push_str("</Files>");
    }
    output
}

// ============================================================================
// New xot-based pipeline
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

/// Load XML directly as a ParseResult (pass-through mode)
///
/// This function allows you to load pre-generated XML (e.g., snapshots)
/// and query them with XPath, just like parsed source files.
///
/// The XML is passed through without parsing source code.
/// XML declarations are stripped to avoid conflicts when wrapping.
pub fn load_xml(xml: String, file_path: String) -> ParseResult {
    // Strip XML declaration to avoid nested declarations when wrapping
    let xml = strip_xml_declaration(&xml);

    // Try to extract source lines from the XML if they're embedded
    // For now, just use empty source lines
    let source_lines = Vec::new();

    ParseResult {
        xml,
        source_lines,
        file_path,
        language: "xml".to_string(),
    }
}

/// Strip XML declaration (<?xml ...?>) from the beginning of an XML string
fn strip_xml_declaration(xml: &str) -> String {
    let trimmed = xml.trim_start();
    if trimmed.starts_with("<?xml") {
        if let Some(end) = trimmed.find("?>") {
            return trimmed[end + 2..].trim_start().to_string();
        }
    }
    xml.to_string()
}

/// Load XML from a file as a ParseResult (pass-through mode)
pub fn load_xml_file(path: &Path) -> Result<ParseResult, ParseError> {
    let xml = fs::read_to_string(path)?;
    Ok(load_xml(xml, path.to_string_lossy().to_string()))
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
