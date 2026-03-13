//! Output formatting for query results
//!
//! Supports multiple output formats:
//! - xml: XML of matched nodes
//! - lines: Full source lines containing the match
//! - source: Exact matched source (column-precise)
//! - value: Text content of matched node
//! - gcc: GCC-style file:line:col: message
//! - json: JSON array with match details
//! - count: Number of matches
//! - schema: Merged tree of unique element paths

mod formatter;
mod colors;
pub mod xml_renderer;
pub mod syntax_highlight;
mod schema;
pub mod xml_to_json;

pub use formatter::{TextViewMode, format_matches, OutputOptions, format_message};
pub use formatter::normalize_path;
pub use colors::should_use_color;
pub use xml_renderer::{render_node, render_document, render_xml_string, RenderOptions};
pub use schema::{format_schema, SchemaCollector, SchemaNode};
pub use xml_to_json::xml_fragment_to_json;

use crate::xpath::Match;

/// Format a list of matches according to the specified format
pub fn format_output(
    matches: &[Match],
    format: TextViewMode,
    options: &OutputOptions,
) -> String {
    format_matches(matches, format, options)
}
