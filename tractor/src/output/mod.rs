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
mod query_tree_renderer;
pub mod xml_renderer;
pub mod syntax_highlight;
mod schema;
pub mod xml_to_json;

pub use formatter::{render_tree_match, render_source_match, render_lines_match};
pub use formatter::{render_source_precomputed, render_lines};
pub use formatter::{format_message, normalize_path};
pub use query_tree_renderer::{render_query_tree_node, render_query_tree_with_source};
pub use colors::should_use_color;
pub use xml_renderer::{render_node, render_document, render_xml_string, render_xml_node, xml_node_to_string, RenderOptions};
pub use schema::{format_schema, SchemaCollector, SchemaNode};
pub use xml_to_json::xml_node_to_json;
pub use syntax_highlight::extract_syntax_spans_from_xml_node;
