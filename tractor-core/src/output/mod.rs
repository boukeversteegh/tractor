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

mod formatter;
mod colors;

pub use formatter::{OutputFormat, format_matches, OutputOptions};
pub use colors::{should_use_color, colorize_xml, colorize_xml_with_highlights};

use crate::xpath::Match;

/// Format a list of matches according to the specified format
pub fn format_output(
    matches: &[Match],
    format: OutputFormat,
    options: &OutputOptions,
) -> String {
    format_matches(matches, format, options)
}
