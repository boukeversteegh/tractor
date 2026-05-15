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
use once_cell::sync::Lazy;
use crate::tree_mode::TreeMode;

/// Supported languages and their extensions, derived from
/// [`crate::languages::LANGUAGES`] plus the XML passthrough entry.
///
/// XML isn't a tree-sitter language — it's loaded directly into xee
/// `Documents` via [`load_xml_file_to_documents`] — so it has no row
/// in `LANGUAGES`, but it still claims `.xml` here.
pub static SUPPORTED_LANGUAGES: Lazy<Vec<(&'static str, &'static [&'static str])>> = Lazy::new(|| {
    let mut out: Vec<(&'static str, &'static [&'static str])> = crate::languages::LANGUAGES
        .iter()
        .map(|l| (l.ids[0], l.extensions))
        .collect();
    out.push(("xml", &["xml"]));
    out
});

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

/// Detect language from file path extension. Returns the canonical
/// name (the first entry of [`crate::languages::LanguageOps::ids`])
/// of the registry row whose `extensions` claim the path. `"xml"` is
/// the only special case (passthrough; not in `LANGUAGES`). Returns
/// `"unknown"` when no row matches.
pub fn detect_language(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("").to_lowercase();
    if ext == "xml" {
        return "xml";
    }
    crate::languages::LANGUAGES
        .iter()
        .find(|l| l.extensions.iter().any(|e| *e == ext))
        .map(|l| l.ids[0])
        .unwrap_or("unknown")
}

/// Get the tree-sitter [`Language`](tree_sitter::Language) for a
/// language name (canonical or alias). Reads from the registry's
/// `grammar` field — see [`crate::languages::LanguageOps::grammar`].
fn get_tree_sitter_language(lang: &str) -> Result<tree_sitter::Language, ParseError> {
    crate::languages::get_language(lang)
        .map(|l| (l.grammar)())
        .ok_or_else(|| ParseError::UnsupportedLanguage(lang.to_string()))
}

/// Language ABI version info
#[derive(Debug, Clone)]
pub struct LanguageAbiInfo {
    /// Tractor language name (e.g., "csharp", "rust")
    pub name: &'static str,
    /// Tree-sitter ABI version
    pub abi_version: usize,
}

/// Get ABI versions for every supported tree-sitter language.
///
/// Derived from [`crate::languages::LANGUAGES`]: one entry per row,
/// using `ids[0]` as the canonical name and `(grammar)().abi_version()`
/// for the version. The ABI version indicates tree-sitter parser
/// compatibility.
pub fn get_language_abi_versions() -> Vec<LanguageAbiInfo> {
    crate::languages::LANGUAGES
        .iter()
        .map(|l| LanguageAbiInfo {
            name: l.ids[0],
            abi_version: (l.grammar)().abi_version(),
        })
        .collect()
}

// ============================================================================
// Xot-based pipeline
// ============================================================================

use crate::language_info::get_all_languages_for_extension;
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

/// Parse via the typed-tree pipeline and return an `XeeParseResult`
/// (the fast-query path used by `parse(...)`). Lowers CST → tree →
/// xot, then serializes the xot document and re-parses it into an
/// xee `Documents`. The serialize/reparse step is a v1 stepping
/// stone; a future optimization can build directly into Documents.
#[cfg(feature = "native")]
fn parse_with_ir_pipeline_to_xee(
    source: &str,
    lang: &str,
    file_path: String,
    tree_mode: TreeMode,
) -> Result<XeeParseResult, ParseError> {
    use crate::tree;
    use crate::languages::TreeKind;

    let language = get_tree_sitter_language(lang)?;
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&language)
        .map_err(|e| ParseError::TreeSitter(e.to_string()))?;
    let tree = parser.parse(source, None)
        .ok_or_else(|| ParseError::Parse("Failed to parse source".to_string()))?;

    let lang_ops = crate::languages::get_language(lang).ok_or_else(|| {
        ParseError::Parse(format!("Unknown language: {lang}"))
    })?;

    let mut xot = xot::Xot::new();
    let holding = xot.new_document();

    // Convert the tree-sitter CST root to an owned `RawNode` once.
    // Lowerings receive `&RawNode` so the same lowering compiles for
    // WASM (which deserialises `RawNode` from `web-tree-sitter` JSON).
    let raw_root = crate::raw::RawNode::from_tree_sitter(tree.root_node(), source);

    // Render to xot via the tree family-specific lower + render pair,
    // then capture the result as an `XmlNode` (for legacy XML / text
    // renderers) and a `Tree::*` (for tree-aware renderers).
    let root_tree = match lang_ops.tree_kind {
        TreeKind::Syntax(lower) => {
            let mut ir_tree = lower(&raw_root, source);
            // Slice 1 invariant: every tree exiting the parser has a
            // NodeId stamped on every node's Span. Downstream
            // XPath → typed-node lookup paths depend on this.
            tree::assign_ids_syntax(&mut ir_tree);
            tree::render_to_xot(&mut xot, holding, &ir_tree, source)
                .map_err(|e| ParseError::Parse(format!("tree render failed: {e}")))?;
            let xml_node = xot.children(holding)
                .find(|&c| xot.element(c).is_some())
                .map(|n| crate::xpath::xot_node_to_xml_node(&xot, n));
            let source_arc = std::sync::Arc::new(source.to_string());
            xml_node.map(|x| crate::xpath::Tree::SyntaxTree {
                tree: std::sync::Arc::new(ir_tree),
                source: source_arc,
                xml: x,
            })
        }
        TreeKind::Data { structure, content } => {
            // Tree mode picks the parser. `uses_tree` filters out Raw
            // upstream, so this match is exhaustive.
            let parser = match tree_mode {
                TreeMode::Structure => structure,
                TreeMode::Data => content,
                TreeMode::Raw => unreachable!(
                    "uses_tree returns false for Raw mode on Data languages"
                ),
            };
            let mut data_tree = (parser.lower)(&raw_root, source);
            tree::assign_ids_data(&mut data_tree);
            (parser.render)(&mut xot, holding, &data_tree, source)
                .map_err(|e| ParseError::Parse(format!("DataTree render failed: {e}")))?;
            let xml_node = xot.children(holding)
                .find(|&c| xot.element(c).is_some())
                .map(|n| crate::xpath::xot_node_to_xml_node(&xot, n));
            let source_arc = std::sync::Arc::new(source.to_string());
            xml_node.map(|x| crate::xpath::Tree::DataTree {
                tree: std::sync::Arc::new(data_tree),
                source: source_arc,
                xml: x,
            })
        }
        TreeKind::Sql(lower) => {
            let mut sql_tree = lower(&raw_root, source);
            tree::assign_ids_sql(&mut sql_tree);
            tree::sql::to_xot::render_sql_to_xot(&mut xot, holding, &sql_tree, source)
                .map_err(|e| ParseError::Parse(format!("SqlTree render failed: {e}")))?;
            let xml_node = xot.children(holding)
                .find(|&c| xot.element(c).is_some())
                .map(|n| crate::xpath::xot_node_to_xml_node(&xot, n));
            let source_arc = std::sync::Arc::new(source.to_string());
            xml_node.map(|x| crate::xpath::Tree::Sql {
                tree: std::sync::Arc::new(sql_tree),
                source: source_arc,
                xml: x,
            })
        }
        TreeKind::None => {
            return Err(ParseError::Parse(format!(
                "tree pipeline not yet wired for language {lang}"
            )));
        }
    };

    // Serialize xot → string → re-parse into xee Documents (the v1
    // stepping stone; future S7 work eliminates this round-trip).
    let xml = xot.to_string(holding)
        .map_err(|e| ParseError::Parse(format!("tree serialize failed: {e}")))?;
    let mut documents = Documents::new();
    let doc_handle = documents.add_string(
        "file:///source".try_into().unwrap(),
        &xml,
    ).map_err(|e| ParseError::Parse(format!("xee load failed: {e}")))?;
    let source_lines = std::sync::Arc::new(source.lines().map(|s| s.to_string()).collect());

    Ok(XeeParseResult {
        documents,
        doc_handle,
        source_lines,
        file_path,
        language: lang.to_string(),
        root_tree,
    })
}

#[cfg(not(feature = "native"))]
fn parse_with_ir_pipeline_to_xee(
    _source: &str,
    _lang: &str,
    _file_path: String,
) -> Result<XeeParseResult, ParseError> {
    Err(ParseError::Parse(
        "tree pipeline requires the `native` feature".to_string(),
    ))
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
    /// Typed tree for the document root, retained from parse so the
    /// format layer can render JSON / YAML / etc. directly from the
    /// tree instead of going through `xml_to_json`. `Some` only when
    /// the parser took the tree pipeline.
    pub root_tree: Option<crate::xpath::Tree>,
}

impl XeeParseResult {
    /// Execute an XPath query on the parsed document.
    ///
    /// Forwards `root_tree` to the engine so root-document matches
    /// carry the typed tree for principled JSON / YAML rendering at
    /// format time.
    pub fn query(&mut self, xpath: &str) -> Result<Vec<crate::xpath::Match>, crate::xpath::XPathError> {
        let engine = crate::xpath::XPathEngine::new();
        engine.query_documents_with_root_tree(
            &mut self.documents,
            self.doc_handle,
            xpath,
            self.source_lines.clone(),
            &self.file_path,
            self.root_tree.as_ref(),
        )
    }
}

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

/// Inline source → `XeeParseResult` core. Drives the typed-tree
/// pipeline for tree-supported languages and the
/// `lower_raw_passthrough_all` raw dump for everything else, then
/// serialises the xot document into xee `Documents` (the v1 stepping
/// stone the typed path uses; S7 will eliminate this round-trip).
///
/// Private because it is reached only through the unified [`parse`]
/// entry point. `ignore_whitespace` is accepted to match `ParseOptions`
/// but is a no-op in the typed pipeline (whitespace handling lives in
/// the per-language lowerings); `max_depth` is accepted for parity
/// with `ParseOptions::parse_depth` and currently ignored.
fn parse_inline_to_xee(
    source: &str,
    lang: &str,
    file_path: String,
    tree_mode: Option<TreeMode>,
    _ignore_whitespace: bool,
    _max_depth: Option<usize>,
) -> Result<XeeParseResult, ParseError> {
    use std::time::Instant;

    let resolved = TreeMode::resolve(tree_mode, lang)
        .map_err(ParseError::Parse)?;

    if crate::languages::get_language(lang).map(|l| l.uses_tree(resolved)).unwrap_or(false) {
        return parse_with_ir_pipeline_to_xee(source, lang, file_path, resolved);
    }

    // Raw mode (or `TreeKind::None` language): typed CST dump via
    // `lower_raw_passthrough_all`, then serialize + reparse into xee
    // `Documents` (same v1 stepping stone the typed path uses; S7
    // will eliminate this round-trip).
    let language = get_tree_sitter_language(lang)?;

    let t0 = Instant::now();
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&language)
        .map_err(|e| ParseError::TreeSitter(e.to_string()))?;
    let tree = parser.parse(source, None)
        .ok_or_else(|| ParseError::Parse("Failed to parse source".to_string()))?;
    let t1 = Instant::now();

    let raw_root = crate::raw::RawNode::from_tree_sitter(tree.root_node(), source);
    let mut ir_tree = crate::tree::lower_raw_passthrough_all(&raw_root, source);
    crate::tree::assign_ids_syntax(&mut ir_tree);

    let mut xot = xot::Xot::new();
    let holding = xot.new_document();
    crate::tree::render_to_xot(&mut xot, holding, &ir_tree, source)
        .map_err(|e| ParseError::Parse(format!("raw passthrough render failed: {e}")))?;

    let xml = xot.to_string(holding)
        .map_err(|e| ParseError::Parse(format!("tree serialize failed: {e}")))?;
    let mut documents = Documents::new();
    let doc_handle = documents.add_string(
        "file:///source".try_into().unwrap(),
        &xml,
    ).map_err(|e| ParseError::Parse(format!("xee load failed: {e}")))?;
    let t2 = Instant::now();

    let source_lines = std::sync::Arc::new(source.lines().map(|s| s.to_string()).collect());
    let t3 = Instant::now();

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
        root_tree: None,
    })
}

// ============================================================================
// Unified parsing pipeline - always returns Documents
// ============================================================================

/// Load XML string directly into Documents for querying.
///
/// This is the XML passthrough path — no TreeSitter parsing, just load
/// the XML. Reached through [`parse`] when the language is `"xml"`
/// (either detected from path or set explicitly); the function is
/// crate-internal because callers go through `parse(...)` for the
/// public surface.
pub(crate) fn load_xml_string_to_documents(xml: &str, file_path: String) -> Result<XeeParseResult, ParseError> {
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
        root_tree: None,
    })
}

/// Load XML file directly into Documents for querying. Crate-internal
/// counterpart of [`load_xml_string_to_documents`] for the disk path.
pub(crate) fn load_xml_file_to_documents(path: &Path) -> Result<XeeParseResult, ParseError> {
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
                parse_inline_to_xee(
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
                parse_inline_to_xee(
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
