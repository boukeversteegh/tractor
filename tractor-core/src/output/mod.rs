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

mod colors;
mod formatter;
mod schema;
pub mod syntax_highlight;
pub mod xml_renderer;
pub mod xml_to_json;

pub use colors::should_use_color;
pub use formatter::{format_message, normalize_path};
pub use formatter::{render_lines, render_source_precomputed};
pub use formatter::{render_lines_match, render_source_match, render_tree_match};
pub use schema::{format_schema, SchemaCollector, SchemaNode};
pub use syntax_highlight::extract_syntax_spans_from_xml_node;
pub use xml_renderer::{
    render_document, render_node, render_xml_node, render_xml_string, xml_node_to_string,
    RenderOptions,
};
pub use xml_to_json::xml_node_to_json;
