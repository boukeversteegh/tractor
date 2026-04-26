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
use crate::tree_mode::TreeMode;

/// Supported languages and their extensions
pub static SUPPORTED_LANGUAGES: &[(&str, &[&str])] = &[
    ("csharp", &["cs"]),
    ("rust", &["rs"]),
    ("javascript", &["js", "mjs", "cjs", "jsx"]),
    ("typescript", &["ts"]),
    ("tsx", &["tsx"]),
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
    ("env", &["env"]),
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
    // SQL dialects
    ("tsql", &["sql"]),
];

/// Parse result with xot document
pub struct XotParseResult {
    /// The xot document containing the AST
    pub xot: xot::Xot,
    /// Root node of the document
    pub root: xot::Node,
    /// Original source lines for location-based output
    pub source_lines: Vec<String>,
    /// File path or [`PATHLESS_LABEL`](crate::PATHLESS_LABEL) for pathless input
    pub file_path: String,
    /// Language used for parsing
    pub language: String,
}

/// Errors that can occur during parsing
#[derive(Error, Debug)]
pub enum ParseError {
    #[error("Unsupported language: {0}")]
    UnsupportedLanguage(String),
    #[error("Ambiguous file extension '.{extension}': multiple languages match ({languages}). Use --lang to specify which language to use (e.g. --lang {first}).")]
    AmbiguousLanguage {
        extension: String,
        languages: String,
        first: String,
    },
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
        "ts" => "typescript",
        "tsx" => "tsx",
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
        "env" => "env",
        "php" => "php",
        "scala" | "sc" => "scala",
        "lua" => "lua",
        "hs" | "lhs" => "haskell",
        "ml" | "mli" => "ocaml",
        "r" => "r",
        "jl" => "julia",
        "md" | "markdown" | "mdx" => "markdown",
        "xml" => "xml",
        "sql" => "tsql",
        _ => "unknown",
    }
}

/// Get TreeSitter language for a language name
fn get_tree_sitter_language(lang: &str) -> Result<tree_sitter::Language, ParseError> {
    match lang {
        "csharp" | "cs" => Ok(tree_sitter_c_sharp::LANGUAGE.into()),
        "rust" | "rs" => Ok(tree_sitter_rust::LANGUAGE.into()),
        "javascript" | "js" => Ok(tree_sitter_javascript::LANGUAGE.into()),
        "typescript" | "ts" => Ok(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        "tsx" => Ok(tree_sitter_typescript::LANGUAGE_TSX.into()),
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
        "env" => Ok(tree_sitter_bash::LANGUAGE.into()),
        "php" => Ok(tree_sitter_php::LANGUAGE_PHP.into()),
        "scala" => Ok(tree_sitter_scala::LANGUAGE.into()),
        "lua" => Ok(tree_sitter_lua::LANGUAGE.into()),
        "haskell" | "hs" => Ok(tree_sitter_haskell::LANGUAGE.into()),
        "ocaml" | "ml" => Ok(tree_sitter_ocaml::LANGUAGE_OCAML.into()),
        "r" => Ok(tree_sitter_r::LANGUAGE.into()),
        "julia" | "jl" => Ok(tree_sitter_julia::LANGUAGE.into()),
        "markdown" | "md" | "mdx" => Ok(tree_sitter_md::LANGUAGE.into()),
        "tsql" | "mssql" => Ok(tree_sitter_sequel_tsql::LANGUAGE.into()),
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
            name: "env",
            abi_version: tree_sitter::Language::from(tree_sitter_bash::LANGUAGE).abi_version(),
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
            name: "tsx",
            abi_version: tree_sitter::Language::from(tree_sitter_typescript::LANGUAGE_TSX).abi_version(),
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
        LanguageAbiInfo {
            name: "tsql",
            abi_version: tree_sitter::Language::from(tree_sitter_sequel_tsql::LANGUAGE).abi_version(),
        },
    ]
}

// ============================================================================
// Xot-based pipeline
// ============================================================================

use crate::language_info::get_all_languages_for_extension;
use crate::transform::builder::{XotBuilder, XeeBuilder};
use xee_xpath::{Documents, DocumentHandle};

/// Check if a file extension is ambiguous (multiple languages claim it).
/// Returns Ok(()) if the extension is unambiguous, or an error if it is ambiguous.
fn check_ambiguous_extension(path: &Path) -> Result<(), ParseError> {
    let ext = path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    if ext.is_empty() {
        return Ok(());
    }
    let langs = get_all_languages_for_extension(ext);
    if langs.len() > 1 {
        let lang_names: Vec<&str> = langs.iter().map(|l| l.name).collect();
        return Err(ParseError::AmbiguousLanguage {
            extension: ext.to_string(),
            languages: lang_names.join(", "),
            first: lang_names[0].to_string(),
        });
    }
    Ok(())
}

/// Parse a file and return an xot document (new pipeline)
pub fn parse_file_to_xot(path: &Path, lang_override: Option<&str>, tree_mode: Option<TreeMode>) -> Result<XotParseResult, ParseError> {
    parse_file_to_xot_with_options(path, lang_override, tree_mode, false)
}

/// Parse a file and return an xot document with options (new pipeline)
pub fn parse_file_to_xot_with_options(
    path: &Path,
    lang_override: Option<&str>,
    tree_mode: Option<TreeMode>,
    ignore_whitespace: bool,
) -> Result<XotParseResult, ParseError> {
    if lang_override.is_none() {
        check_ambiguous_extension(path)?;
    }
    let source = fs::read_to_string(path)?;
    let lang = lang_override.unwrap_or_else(|| detect_language(path.to_str().unwrap_or("")));
    parse_string_to_xot_with_options(&source, lang, path.to_string_lossy().to_string(), tree_mode, ignore_whitespace)
}

/// Parse a source string and return an xot document (new pipeline)
pub fn parse_string_to_xot(source: &str, lang: &str, file_path: String, tree_mode: Option<TreeMode>) -> Result<XotParseResult, ParseError> {
    parse_string_to_xot_with_options(source, lang, file_path, tree_mode, false)
}

/// Parse a source string and return an xot document with options (new pipeline)
///
/// Note: the Xot pipeline only supports Raw and Structure modes (no dual-branch).
/// For data-aware languages, Structure uses the syntax transform (ast_transform).
pub fn parse_string_to_xot_with_options(
    source: &str,
    lang: &str,
    file_path: String,
    tree_mode: Option<TreeMode>,
    ignore_whitespace: bool,
) -> Result<XotParseResult, ParseError> {
    let resolved = TreeMode::resolve(tree_mode, lang)
        .map_err(ParseError::Parse)?;

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

    // Apply semantic transforms based on tree mode
    if resolved != TreeMode::Raw {
        // Per-language field wrapping (turns `<identifier field="name">` into
        // `<name><identifier field="identifier"></identifier></name>` etc.)
        let wrappings = languages::get_field_wrappings(lang);
        crate::transform::apply_field_wrappings(&mut xot, root, wrappings)
            .map_err(|e| ParseError::Parse(e.to_string()))?;

        let transform_fn = languages::get_transform(lang);
        crate::transform::walk_transform(&mut xot, root, transform_fn)
            .map_err(|e| ParseError::Parse(e.to_string()))?;

        // Post-walk structural rewrites (e.g. flat conditional shape).
        if let Some(post_fn) = languages::get_post_transform(lang) {
            post_fn(&mut xot, root)
                .map_err(|e| ParseError::Parse(e.to_string()))?;
        }
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
    /// File path or [`PATHLESS_LABEL`](crate::PATHLESS_LABEL) for pathless input
    pub file_path: String,
    /// Language used for parsing
    pub language: String,
}

impl XeeParseResult {
    /// Execute an XPath query on the parsed document.
    ///
    /// This is a convenience method that creates an XPathEngine and calls
    /// `query_documents`, avoiding the need to destructure the parse result.
    pub fn query(&mut self, xpath: &str) -> Result<Vec<crate::xpath::Match>, crate::xpath::XPathError> {
        let engine = crate::xpath::XPathEngine::new();
        engine.query_documents(
            &mut self.documents,
            self.doc_handle,
            xpath,
            self.source_lines.clone(),
            &self.file_path,
        )
    }
}

/// Parse a source string directly into Documents for fast XPath queries
///
/// This is the fast path that avoids XML serialization/parsing roundtrip.
/// Returns an XeeParseResult that can be queried with XPathEngine::query_documents().
pub fn parse_string_to_xee(
    source: &str,
    lang: &str,
    file_path: String,
    tree_mode: Option<TreeMode>,
) -> Result<XeeParseResult, ParseError> {
    parse_string_to_xee_with_options(source, lang, file_path, tree_mode, false, None)
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
    tree_mode: Option<TreeMode>,
    ignore_whitespace: bool,
    max_depth: Option<usize>,
) -> Result<XeeParseResult, ParseError> {
    use std::time::Instant;

    let resolved = TreeMode::resolve(tree_mode, lang)
        .map_err(ParseError::Parse)?;
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
    let doc_handle = builder.build_with_options(tree.root_node(), source, &file_path, lang, resolved, ignore_whitespace, max_depth)
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
    tree_mode: Option<TreeMode>,
) -> Result<XeeParseResult, ParseError> {
    parse_file_to_xee_with_options(path, lang_override, tree_mode, false)
}

/// Parse a file directly into Documents with options
pub fn parse_file_to_xee_with_options(
    path: &Path,
    lang_override: Option<&str>,
    tree_mode: Option<TreeMode>,
    ignore_whitespace: bool,
) -> Result<XeeParseResult, ParseError> {
    if lang_override.is_none() {
        check_ambiguous_extension(path)?;
    }
    let source = fs::read_to_string(path)?;
    let lang = lang_override.unwrap_or_else(|| detect_language(path.to_str().unwrap_or("")));
    parse_string_to_xee_with_options(&source, lang, path.to_string_lossy().to_string(), tree_mode, ignore_whitespace, None)
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

/// Parse `source` with the tree-sitter grammar for `lang` and return
/// the set of distinct named-node kinds present in the raw parse tree
/// (BEFORE any tractor transform). Used by the kind-catalogue lint
/// test to detect tree-sitter kinds the language's transform doesn't
/// know about.
///
/// Returns kinds in deterministic insertion order (sorted on the way
/// out is the caller's job).
pub fn raw_kinds(lang: &str, source: &str) -> Result<Vec<String>, ParseError> {
    let language = get_tree_sitter_language(lang)?;
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&language)
        .map_err(|e| ParseError::TreeSitter(e.to_string()))?;
    let tree = parser.parse(source, None)
        .ok_or_else(|| ParseError::Parse("Failed to parse source".to_string()))?;

    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut cursor = tree.root_node().walk();
    fn walk(
        cursor: &mut tree_sitter::TreeCursor<'_>,
        seen: &mut std::collections::BTreeSet<String>,
    ) {
        let node = cursor.node();
        if node.is_named() {
            seen.insert(node.kind().to_string());
        }
        if cursor.goto_first_child() {
            loop {
                walk(cursor, seen);
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
            cursor.goto_parent();
        }
    }
    walk(&mut cursor, &mut seen);
    Ok(seen.into_iter().collect())
}

/// Where the bytes to parse come from.
///
/// The input kind is an explicit parameter of [`parse`] rather than being
/// encoded in which-function-you-called. `Disk` paths are read lazily and
/// subject to ambiguous-extension checks when the language is auto-detected;
/// `Inline` paths carry a user-supplied `file_label` that propagates through
/// to diagnostics and query results.
pub enum ParseInput<'a> {
    /// Read the source from a file on disk.
    Disk { path: &'a Path },
    /// Parse in-memory content, labelling it with `file_label` (virtual path
    /// or sentinel) for diagnostics.
    Inline { content: &'a str, file_label: &'a str },
}

/// Parse knobs shared by every entry point.
///
/// Keeping these in one struct means adding a new knob (e.g. `parse_depth`)
/// doesn't force yet another `*_with_options` function to appear; it's just
/// another field.
#[derive(Default, Clone, Copy)]
pub struct ParseOptions<'a> {
    /// Explicit language override. If `None`, [`parse`] auto-detects from the
    /// path (disk) or treats the absence as an error for inline inputs — the
    /// old inline entry points required a non-optional language, which this
    /// struct mirrors by requiring `Inline` callers to populate this field.
    pub language: Option<&'a str>,
    /// Tree-building mode. `None` defers to per-language defaults.
    pub tree_mode: Option<TreeMode>,
    /// Collapse whitespace-only text nodes during tree building.
    pub ignore_whitespace: bool,
    /// Cap tree-building depth (skip deeper nodes for speed).
    pub parse_depth: Option<usize>,
}

/// The one principled parse entry point.
///
/// This is the primary library-level parse function. It handles both on-disk
/// files and in-memory content uniformly, dispatching internally on
/// [`ParseInput`]:
///
/// - `Disk`: runs ambiguous-extension checks when the language was
///   auto-detected, reads the file, then routes XML to the passthrough loader
///   and everything else to TreeSitter + `XeeBuilder`.
/// - `Inline`: routes XML to the string passthrough and everything else to
///   TreeSitter, carrying `file_label` through to diagnostics.
///
/// This is the single public parse entry point; callers build a `ParseInput`
/// and a `ParseOptions` explicitly rather than picking between overloaded
/// convenience wrappers.
pub fn parse(
    input: ParseInput<'_>,
    options: ParseOptions<'_>,
) -> Result<XeeParseResult, ParseError> {
    match input {
        ParseInput::Disk { path } => {
            if options.language.is_none() {
                check_ambiguous_extension(path)?;
            }
            let lang = options
                .language
                .unwrap_or_else(|| detect_language(path.to_str().unwrap_or("")));

            if lang == "xml" {
                // XML passthrough: load directly into Documents
                load_xml_file_to_documents(path)
            } else {
                // Source code: TreeSitter → XeeBuilder → Documents
                let source = fs::read_to_string(path)?;
                parse_string_to_xee_with_options(
                    &source,
                    lang,
                    path.to_string_lossy().to_string(),
                    options.tree_mode,
                    options.ignore_whitespace,
                    options.parse_depth,
                )
            }
        }
        ParseInput::Inline { content, file_label } => {
            // Inline entry points always had a non-optional language; preserve
            // that invariant by requiring `options.language` to be populated.
            // Auto-detection from a virtual label would be meaningless.
            let lang = options
                .language
                .unwrap_or_else(|| detect_language(file_label));

            if lang == "xml" {
                load_xml_string_to_documents(content, file_label.to_string())
            } else {
                parse_string_to_xee_with_options(
                    content,
                    lang,
                    file_label.to_string(),
                    options.tree_mode,
                    options.ignore_whitespace,
                    options.parse_depth,
                )
            }
        }
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
        assert_eq!(detect_language("foo.ts"), "typescript");
        assert_eq!(detect_language("foo.tsx"), "tsx");
        assert_eq!(detect_language("foo.jsx"), "javascript");
        assert_eq!(detect_language("foo.sql"), "tsql");
        assert_eq!(detect_language("foo.unknown"), "unknown");
    }

    #[test]
    fn test_parse_simple_class() {
        use crate::output::{render_node, RenderOptions};

        let result = parse(
            ParseInput::Inline {
                content: "public class Foo { }",
                file_label: "<test>",
            },
            ParseOptions {
                language: Some("csharp"),
                tree_mode: None,
                ignore_whitespace: false,
                parse_depth: None,
            },
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
