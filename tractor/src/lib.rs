//! tractor: Multi-language code query library and CLI
//!
//! This library provides:
//! - TreeSitter-based parsing for 22+ languages
//! - Semantic tree transformations
//! - XPath 3.1 query execution
//! - Multiple output formats

// ---------------------------------------------------------------------------
// Module structure
// ---------------------------------------------------------------------------

// Parsing & tree construction
#[cfg(feature = "native")]
pub mod parser;
pub mod xot;
pub mod languages;

// Querying
pub mod xpath;

// Output & rendering
pub mod output;
pub mod render;

// Core data types
pub mod model;

// File discovery & path handling
pub mod glob;

// Code mutation
pub mod mutation;

// WASM bindings
#[cfg(feature = "wasm")]
pub mod wasm;

// ---------------------------------------------------------------------------
// Backward-compatible re-exports
//
// These preserve all existing `crate::` and `tractor::` import paths so that
// consumers (the binary crate, tests, etc.) don't need import changes.
// ---------------------------------------------------------------------------

// xot/ modules
pub use xot::builder as xot_builder;
pub use xot::transform as xot_transform;

// languages/ additions
pub use languages::info as language_info;

// output/ additions
pub use output::source_utils;

// model/ modules
pub use model::report;
pub use model::rule;
pub use model::tree_mode;
pub use model::normalized_xpath;

// glob/ modules
pub use glob::matching as glob_match;
pub use glob::pattern as glob_pattern;
pub use glob::normalized_path;
#[cfg(feature = "native")]
pub use glob::files;

// mutation/ modules
pub use mutation::replace;
#[cfg(feature = "native")]
pub use mutation::xpath_upsert;
#[cfg(feature = "native")]
pub use mutation::declarative_set;

// wasm/ additions
#[cfg(feature = "wasm")]
pub use wasm::ast as wasm_ast;

// ---------------------------------------------------------------------------
// Flat re-exports (convenience API)
// ---------------------------------------------------------------------------

#[cfg(feature = "native")]
pub use parser::{
    detect_language, SUPPORTED_LANGUAGES,
    parse, ParseInput, ParseOptions,
    parse_to_documents, parse_string_to_documents, parse_string_to_documents_with_options,
    load_xml_string_to_documents, load_xml_file_to_documents,
    XeeParseResult,
    get_language_abi_versions, LanguageAbiInfo,
    print_parse_timing_stats,
};
pub use xpath::{XPathEngine, Match, XmlNode, print_timing_stats, Documents, DocumentHandle};
pub use output::{render_tree_match, render_source_match, render_lines_match, render_source_precomputed, render_lines, format_message, normalize_path, render_node, render_document, render_xml_string, render_xml_node, render_query_tree_node, render_query_tree_with_source, xml_node_to_string, RenderOptions, format_schema, format_schema_tree, SchemaCollector, xml_node_to_json, extract_syntax_spans_from_xml_node};
pub use replace::{apply_replacements, apply_set_to_string, ReplaceSummary, ReplaceError};
pub use report::{Report, ReportBuilder, ReportMatch, ResultItem, Totals, Severity, DiagnosticOrigin, PATHLESS_LABEL, is_pathless_file};
pub use rule::{Rule, RuleSet};
#[cfg(feature = "native")]
pub use rule::{GlobMatcher, GlobError, CompiledRule, compile_ruleset};
#[cfg(feature = "native")]
pub use files::{expand_globs, expand_globs_checked, GlobExpansion, GlobExpansionError};
pub use xot_builder::{XotBuilder, XeeBuilder};
pub use normalized_xpath::NormalizedXpath;
pub use normalized_path::NormalizedPath;
pub use glob_pattern::GlobPattern;
pub use glob_match::CompiledPattern;
#[cfg(feature = "native")]
pub use glob_match::{expand_canonical, pattern_literal_prefix, FilePrune, GlobExpandError};
pub use tree_mode::TreeMode;
