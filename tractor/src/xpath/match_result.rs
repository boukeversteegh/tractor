//! Match result types for XPath queries

use std::sync::Arc;

// ---------------------------------------------------------------------------
// XmlNode — native IR for matched XML fragments and XPath data types
// ---------------------------------------------------------------------------

/// A native representation of an XML node tree or XPath value.
///
/// For XML nodes this avoids the serialize-then-reparse roundtrip: instead of
/// calling `xot.to_string(node)` and later parsing the string back, we walk
/// the xot tree once and build an `XmlNode` that downstream renderers can
/// consume directly.
///
/// For XPath maps and arrays, the structured data is represented natively
/// rather than stored as a JSON string — enabling renderers to work with
/// real typed data without deferred parsing.
#[derive(Debug, Clone, PartialEq)]
pub enum XmlNode {
    // --- XML node variants ---

    /// An XML element with tag name, attributes, and children.
    Element {
        name: String,
        attributes: Vec<(String, String)>,
        children: Vec<XmlNode>,
    },
    /// A text node.
    Text(String),
    /// A comment node.
    Comment(String),
    /// A processing instruction.
    ProcessingInstruction {
        target: String,
        data: Option<String>,
    },

    // --- XPath data variants (maps, arrays, scalars) ---

    /// An XPath map: ordered sequence of key–value pairs.
    /// Keys are always strings (from XPath map constructors).
    Map { entries: Vec<(String, XmlNode)> },
    /// An XPath array: ordered sequence of values.
    Array { items: Vec<XmlNode> },
    /// A numeric value (integer or float).
    Number(f64),
    /// A boolean value.
    Boolean(bool),
    /// An explicit null / empty-sequence value.
    Null,
}

// ---------------------------------------------------------------------------
// Match
// ---------------------------------------------------------------------------

/// A single match from an XPath query
#[derive(Debug, Clone)]
pub struct Match {
    /// File path where the match was found
    pub file: String,
    /// Start line (1-based)
    pub line: u32,
    /// Start column (1-based)
    pub column: u32,
    /// End line (1-based)
    pub end_line: u32,
    /// End column (1-based)
    pub end_column: u32,
    /// The matched value (text content or source snippet)
    pub value: String,
    /// Original source lines for location-based output (Arc for cheap cloning)
    pub source_lines: Arc<Vec<String>>,
    /// The matched XML node tree or XPath structured data (map/array).
    pub xml_node: Option<XmlNode>,
}

impl Match {
    /// Create a new match with minimal information
    pub fn new(file: String, value: String) -> Self {
        Match {
            file,
            line: 1,
            column: 1,
            end_line: 1,
            end_column: 1,
            value,
            source_lines: Arc::new(Vec::new()),
            xml_node: None,
        }
    }

    /// Create a match with full location information
    pub fn with_location(
        file: String,
        line: u32,
        column: u32,
        end_line: u32,
        end_column: u32,
        value: String,
        source_lines: Arc<Vec<String>>,
    ) -> Self {
        Match {
            file,
            line,
            column,
            end_line,
            end_column,
            value,
            source_lines,
            xml_node: None,
        }
    }

    /// Set the XML node tree for this match
    pub fn with_xml_node(mut self, node: XmlNode) -> Self {
        self.xml_node = Some(node);
        self
    }

    /// Returns `true` when this match's file is the pathless sentinel —
    /// i.e. the match came from inline input (`-s`/stdin) with no
    /// meaningful path to display or write back to.
    pub fn is_pathless(&self) -> bool {
        crate::model::report::is_pathless_file(&self.file)
    }

    /// Extract source snippet from source lines based on location
    pub fn extract_source_snippet(&self) -> String {
        if self.source_lines.is_empty() || self.line == 0 {
            return self.value.clone();
        }

        let start_line = (self.line as usize).saturating_sub(1);
        let end_line = (self.end_line as usize).min(self.source_lines.len());

        if start_line >= self.source_lines.len() {
            return self.value.clone();
        }

        let mut result = String::new();

        for (i, line_idx) in (start_line..end_line).enumerate() {
            let line = &self.source_lines[line_idx];
            let trimmed = line.trim_end_matches('\r');

            let start_col = if i == 0 {
                (self.column as usize).saturating_sub(1).min(trimmed.len())
            } else {
                0
            };

            let end_col = if line_idx == end_line - 1 {
                (self.end_column as usize).saturating_sub(1).min(trimmed.len())
            } else {
                trimmed.len()
            };

            if end_col > start_col {
                result.push_str(&trimmed[start_col..end_col]);
            }

            if line_idx < end_line - 1 {
                result.push('\n');
            }
        }

        result
    }

    /// Get the full source lines for the match range
    pub fn get_source_lines_range(&self) -> Vec<&str> {
        if self.source_lines.is_empty() || self.line == 0 {
            return Vec::new();
        }

        let start = (self.line as usize).saturating_sub(1);
        let end = (self.end_line as usize).min(self.source_lines.len());

        self.source_lines[start..end]
            .iter()
            .map(|s| s.as_str())
            .collect()
    }
}
