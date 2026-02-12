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
    ("toml", &["toml"]),
    ("ini", &["ini", "cfg", "inf"]),
    ("php", &["php"]),
    ("scala", &["scala", "sc"]),
    ("lua", &["lua"]),
    ("haskell", &["hs", "lhs"]),
    ("ocaml", &["ml", "mli"]),
    ("r", &["r"]),
    ("julia", &["jl"]),
    ("markdown", &["md", "markdown", "mdx"]),
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
        "toml" => "toml",
        "ini" | "cfg" | "inf" => "ini",
        "php" => "php",
        "scala" | "sc" => "scala",
        "lua" => "lua",
        "hs" | "lhs" => "haskell",
        "ml" | "mli" => "ocaml",
        "r" => "r",
        "jl" => "julia",
        "md" | "markdown" | "mdx" => "markdown",
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
        "toml" => Ok(tree_sitter_toml_ng::LANGUAGE.into()),
        "ini" => Ok(tree_sitter_ini::LANGUAGE.into()),
        "php" => Ok(tree_sitter_php::LANGUAGE_PHP.into()),
        "scala" => Ok(tree_sitter_scala::LANGUAGE.into()),
        "lua" => Ok(tree_sitter_lua::LANGUAGE.into()),
        "haskell" | "hs" => Ok(tree_sitter_haskell::LANGUAGE.into()),
        "ocaml" | "ml" => Ok(tree_sitter_ocaml::LANGUAGE_OCAML.into()),
        "r" => Ok(tree_sitter_r::LANGUAGE.into()),
        "julia" | "jl" => Ok(tree_sitter_julia::LANGUAGE.into()),
        "markdown" | "md" | "mdx" => Ok(tree_sitter_md::LANGUAGE.into()),
        _ => Err(ParseError::UnsupportedLanguage(lang.to_string())),
    }
}

/// Language ABI version info
#[derive(Debug, Clone)]
pub struct LanguageAbiInfo {
    /// Tractor language name (e.g., "csharp", "rust")
    pub name: &'static str,
    /// Tree-sitter ABI version
    pub abi_version: usize,
}

/// Get ABI versions for all supported tree-sitter languages
///
/// Returns a list of (language_name, abi_version) for all languages.
/// The ABI version indicates tree-sitter parser compatibility.
pub fn get_language_abi_versions() -> Vec<LanguageAbiInfo> {
    vec![
        LanguageAbiInfo {
            name: "bash",
            abi_version: tree_sitter::Language::from(tree_sitter_bash::LANGUAGE).abi_version(),
        },
        LanguageAbiInfo {
            name: "c",
            abi_version: tree_sitter::Language::from(tree_sitter_c::LANGUAGE).abi_version(),
        },
        LanguageAbiInfo {
            name: "csharp",
            abi_version: tree_sitter::Language::from(tree_sitter_c_sharp::LANGUAGE).abi_version(),
        },
        LanguageAbiInfo {
            name: "cpp",
            abi_version: tree_sitter::Language::from(tree_sitter_cpp::LANGUAGE).abi_version(),
        },
        LanguageAbiInfo {
            name: "css",
            abi_version: tree_sitter::Language::from(tree_sitter_css::LANGUAGE).abi_version(),
        },
        LanguageAbiInfo {
            name: "go",
            abi_version: tree_sitter::Language::from(tree_sitter_go::LANGUAGE).abi_version(),
        },
        LanguageAbiInfo {
            name: "haskell",
            abi_version: tree_sitter::Language::from(tree_sitter_haskell::LANGUAGE).abi_version(),
        },
        LanguageAbiInfo {
            name: "html",
            abi_version: tree_sitter::Language::from(tree_sitter_html::LANGUAGE).abi_version(),
        },
        LanguageAbiInfo {
            name: "java",
            abi_version: tree_sitter::Language::from(tree_sitter_java::LANGUAGE).abi_version(),
        },
        LanguageAbiInfo {
            name: "javascript",
            abi_version: tree_sitter::Language::from(tree_sitter_javascript::LANGUAGE).abi_version(),
        },
        LanguageAbiInfo {
            name: "json",
            abi_version: tree_sitter::Language::from(tree_sitter_json::LANGUAGE).abi_version(),
        },
        LanguageAbiInfo {
            name: "julia",
            abi_version: tree_sitter::Language::from(tree_sitter_julia::LANGUAGE).abi_version(),
        },
        LanguageAbiInfo {
            name: "lua",
            abi_version: tree_sitter::Language::from(tree_sitter_lua::LANGUAGE).abi_version(),
        },
        LanguageAbiInfo {
            name: "markdown",
            abi_version: tree_sitter::Language::from(tree_sitter_md::LANGUAGE).abi_version(),
        },
        LanguageAbiInfo {
            name: "ocaml",
            abi_version: tree_sitter::Language::from(tree_sitter_ocaml::LANGUAGE_OCAML).abi_version(),
        },
        LanguageAbiInfo {
            name: "php",
            abi_version: tree_sitter::Language::from(tree_sitter_php::LANGUAGE_PHP).abi_version(),
        },
        LanguageAbiInfo {
            name: "python",
            abi_version: tree_sitter::Language::from(tree_sitter_python::LANGUAGE).abi_version(),
        },
        LanguageAbiInfo {
            name: "r",
            abi_version: tree_sitter::Language::from(tree_sitter_r::LANGUAGE).abi_version(),
        },
        LanguageAbiInfo {
            name: "ruby",
            abi_version: tree_sitter::Language::from(tree_sitter_ruby::LANGUAGE).abi_version(),
        },
        LanguageAbiInfo {
            name: "rust",
            abi_version: tree_sitter::Language::from(tree_sitter_rust::LANGUAGE).abi_version(),
        },
        LanguageAbiInfo {
            name: "scala",
            abi_version: tree_sitter::Language::from(tree_sitter_scala::LANGUAGE).abi_version(),
        },
        LanguageAbiInfo {
            name: "typescript",
            abi_version: tree_sitter::Language::from(tree_sitter_typescript::LANGUAGE_TYPESCRIPT).abi_version(),
        },
        LanguageAbiInfo {
            name: "toml",
            abi_version: tree_sitter::Language::from(tree_sitter_toml_ng::LANGUAGE).abi_version(),
        },
        LanguageAbiInfo {
            name: "ini",
            abi_version: tree_sitter::Language::from(tree_sitter_ini::LANGUAGE).abi_version(),
        },
        LanguageAbiInfo {
            name: "yaml",
            abi_version: tree_sitter::Language::from(tree_sitter_yaml::LANGUAGE).abi_version(),
        },
    ]
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
    /// Original source lines for location-based output (Arc for cheap cloning)
    pub source_lines: std::sync::Arc<Vec<String>>,
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
    parse_string_to_xee_with_options(source, lang, file_path, raw_mode, false, None)
}

/// Parse a source string directly into Documents with options
///
/// This is the fast path that avoids XML serialization/parsing roundtrip.
/// Returns an XeeParseResult that can be queried with XPathEngine::query_documents().
/// Use `ignore_whitespace=true` to strip whitespace from text nodes during tree building.
// Timing stats for profiling (in microseconds)
use std::sync::atomic::{AtomicU64, Ordering};
static TIMING_TS_PARSE: AtomicU64 = AtomicU64::new(0);
static TIMING_XOT_BUILD: AtomicU64 = AtomicU64::new(0);
static TIMING_SOURCE_LINES: AtomicU64 = AtomicU64::new(0);
static TIMING_PARSE_COUNT: AtomicU64 = AtomicU64::new(0);

/// Print parsing timing stats
pub fn print_parse_timing_stats() {
    let count = TIMING_PARSE_COUNT.load(Ordering::Relaxed);
    if count == 0 {
        return;
    }
    let ts_parse = TIMING_TS_PARSE.load(Ordering::Relaxed);
    let xot_build = TIMING_XOT_BUILD.load(Ordering::Relaxed);
    let source_lines = TIMING_SOURCE_LINES.load(Ordering::Relaxed);

    eprintln!("\n=== Parse Timing Stats ({} files) ===", count);
    eprintln!("TreeSitter parse: {:>8.2}ms ({:.2}ms/file)",
        ts_parse as f64 / 1000.0, ts_parse as f64 / 1000.0 / count as f64);
    eprintln!("Xot building:     {:>8.2}ms ({:.2}ms/file)",
        xot_build as f64 / 1000.0, xot_build as f64 / 1000.0 / count as f64);
    eprintln!("Source lines:     {:>8.2}ms ({:.2}ms/file)",
        source_lines as f64 / 1000.0, source_lines as f64 / 1000.0 / count as f64);
    eprintln!("Total parsing:    {:>8.2}ms ({:.2}ms/file)",
        (ts_parse + xot_build + source_lines) as f64 / 1000.0,
        (ts_parse + xot_build + source_lines) as f64 / 1000.0 / count as f64);
}

/// Parse a source string directly into Documents with all options
///
/// This is the fast path that avoids XML serialization/parsing roundtrip.
/// Returns an XeeParseResult that can be queried with XPathEngine::query_documents().
/// Use `ignore_whitespace=true` to strip whitespace from text nodes during tree building.
/// Use `max_depth` to limit tree building depth (skip deeper nodes for speed).
pub fn parse_string_to_xee_with_options(
    source: &str,
    lang: &str,
    file_path: String,
    raw_mode: bool,
    ignore_whitespace: bool,
    max_depth: Option<usize>,
) -> Result<XeeParseResult, ParseError> {
    use std::time::Instant;

    let language = get_tree_sitter_language(lang)?;

    let t0 = Instant::now();
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&language)
        .map_err(|e| ParseError::TreeSitter(e.to_string()))?;

    let tree = parser.parse(source, None)
        .ok_or_else(|| ParseError::Parse("Failed to parse source".to_string()))?;
    let t1 = Instant::now();

    // Build directly into Documents using XeeBuilder
    let mut builder = XeeBuilder::new();
    let doc_handle = builder.build_with_options(tree.root_node(), source, &file_path, lang, raw_mode, ignore_whitespace, max_depth)
        .map_err(|e| ParseError::Parse(e.to_string()))?;

    let documents = builder.into_documents();
    let t2 = Instant::now();

    let source_lines = std::sync::Arc::new(source.lines().map(|s| s.to_string()).collect());
    let t3 = Instant::now();

    // Record timing stats
    TIMING_TS_PARSE.fetch_add((t1 - t0).as_micros() as u64, Ordering::Relaxed);
    TIMING_XOT_BUILD.fetch_add((t2 - t1).as_micros() as u64, Ordering::Relaxed);
    TIMING_SOURCE_LINES.fetch_add((t3 - t2).as_micros() as u64, Ordering::Relaxed);
    TIMING_PARSE_COUNT.fetch_add(1, Ordering::Relaxed);

    Ok(XeeParseResult {
        documents,
        doc_handle,
        source_lines,
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
    parse_string_to_xee_with_options(&source, lang, path.to_string_lossy().to_string(), raw_mode, ignore_whitespace, None)
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
        source_lines: std::sync::Arc::new(Vec::new()), // XML passthrough doesn't have source lines
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
/// Use `max_depth` to limit tree building depth (skip deeper nodes for speed).
pub fn parse_to_documents(
    path: &Path,
    lang_override: Option<&str>,
    raw_mode: bool,
    ignore_whitespace: bool,
    max_depth: Option<usize>,
) -> Result<XeeParseResult, ParseError> {
    let lang = lang_override.unwrap_or_else(|| detect_language(path.to_str().unwrap_or("")));

    if lang == "xml" {
        // XML passthrough: load directly into Documents
        load_xml_file_to_documents(path)
    } else {
        // Source code: TreeSitter → XeeBuilder → Documents
        let source = fs::read_to_string(path)?;
        parse_string_to_xee_with_options(&source, lang, path.to_string_lossy().to_string(), raw_mode, ignore_whitespace, max_depth)
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
        parse_string_to_xee_with_options(source, lang, file_path, raw_mode, ignore_whitespace, None)
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
