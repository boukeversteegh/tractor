//! tractor-core: Multi-language code query library
//!
//! This library provides:
//! - TreeSitter-based parsing for 22 languages
//! - Semantic tree transformations
//! - XPath 3.1 query execution
//! - Multiple output formats

#[cfg(feature = "native")]
pub mod parser;
pub mod xpath;
pub mod output;
pub mod render;
pub mod replace;
pub mod source_utils;
pub mod language_info;
#[cfg(feature = "native")]
pub mod parallel;
pub mod xot_builder;
pub mod xot_transform;
#[cfg(feature = "native")]
pub mod xpath_upsert;
#[cfg(feature = "native")]
pub mod declarative_set;
pub mod report;
pub mod diagnostic;
pub mod rule;
pub mod tree_mode;

// Language transforms - available for both native and WASM
pub mod languages;

// WASM support
#[cfg(feature = "wasm")]
pub mod wasm;
#[cfg(feature = "wasm")]
pub mod wasm_ast;

#[cfg(feature = "native")]
pub use parser::{
    detect_language, SUPPORTED_LANGUAGES,
    // Unified parsing pipeline (always returns Documents)
    parse_to_documents, parse_string_to_documents,
    load_xml_string_to_documents, load_xml_file_to_documents,
    XeeParseResult,
    // Version info
    get_language_abi_versions, LanguageAbiInfo,
    // Timing stats
    print_parse_timing_stats,
};
pub use xpath::{XPathEngine, Match, XmlNode, print_timing_stats, Documents, DocumentHandle};
pub use output::{render_tree_match, render_source_match, render_lines_match, render_source_precomputed, render_lines, format_message, normalize_path, render_node, render_document, render_xml_string, render_xml_node, xml_node_to_string, RenderOptions, format_schema, SchemaCollector, xml_node_to_json, extract_syntax_spans_from_xml_node};
pub use replace::{apply_replacements, apply_set_to_string, compute_set_output, ReplaceSummary, ReplaceError};
pub use report::{Report, ReportMatch, ResultItem, Totals, Severity};
pub use diagnostic::{Diagnostic, DiagnosticError};
pub use rule::{Rule, RuleSet};
#[cfg(feature = "native")]
pub use rule::{GlobMatcher, GlobError};
#[cfg(feature = "native")]
pub use parallel::{expand_globs, filter_supported_files};
pub use xot_builder::{XotBuilder, XeeBuilder};
pub use tree_mode::TreeMode;
