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
pub mod replace;
pub mod source_utils;
pub mod language_info;
#[cfg(feature = "native")]
pub mod parallel;
pub mod xot_builder;
pub mod xot_transform;

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
pub use xpath::{XPathEngine, Match, print_timing_stats, Documents, DocumentHandle};
pub use output::{OutputFormat, format_matches, OutputOptions, render_node, render_document, render_xml_string, RenderOptions, format_schema, SchemaCollector};
pub use replace::{apply_replacements, ReplaceSummary, ReplaceError};
#[cfg(feature = "native")]
pub use parallel::{expand_globs, filter_supported_files};
pub use xot_builder::{XotBuilder, XeeBuilder};
