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
    parse_file, parse_string, detect_language, generate_xml_document,
    ParseResult, SUPPORTED_LANGUAGES,
    // New xot-based pipeline
    parse_file_to_xot, parse_string_to_xot, XotParseResult,
    // XML pass-through for testing
    load_xml, load_xml_file,
};
pub use xpath::{XPathEngine, Match};
pub use output::{OutputFormat, format_matches, OutputOptions, render_node, render_document, render_xml_string, RenderOptions};
#[cfg(feature = "native")]
pub use parallel::{process_files_parallel, expand_globs, filter_supported_files};
pub use xot_builder::XotBuilder;
