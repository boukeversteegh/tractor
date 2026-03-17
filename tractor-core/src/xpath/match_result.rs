//! Match result types for XPath queries

use std::sync::Arc;

// ---------------------------------------------------------------------------
// XmlNode — native IR for matched XML fragments
// ---------------------------------------------------------------------------

/// A native representation of an XML node tree, replacing serialized XML strings.
///
/// This avoids the serialize-then-reparse roundtrip: instead of calling
/// `xot.to_string(node)` and later parsing the string back, we walk the xot
/// tree once and build an `XmlNode` that downstream renderers can consume
/// directly.
#[derive(Debug, Clone)]
pub enum XmlNode {
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
}

impl XmlNode {
    /// Serialize this node back to an XML string.
    ///
    /// This is the backward-compat bridge: callers that still expect an XML
    /// string can call this until they are migrated to consume `XmlNode`
    /// directly.
    pub fn to_xml_string(&self) -> String {
        let mut out = String::new();
        self.write_xml(&mut out);
        out
    }

    fn write_xml(&self, out: &mut String) {
        match self {
            XmlNode::Element { name, attributes, children } => {
                out.push('<');
                out.push_str(name);
                for (k, v) in attributes {
                    out.push(' ');
                    out.push_str(k);
                    out.push_str("=\"");
                    out.push_str(&escape_xml_attr(v));
                    out.push('"');
                }
                if children.is_empty() {
                    out.push_str("/>");
                } else {
                    out.push('>');
                    for child in children {
                        child.write_xml(out);
                    }
                    out.push_str("</");
                    out.push_str(name);
                    out.push('>');
                }
            }
            XmlNode::Text(text) => {
                out.push_str(&escape_xml_text(text));
            }
            XmlNode::Comment(text) => {
                out.push_str("<!--");
                out.push_str(text);
                out.push_str("-->");
            }
            XmlNode::ProcessingInstruction { target, data } => {
                out.push_str("<?");
                out.push_str(target);
                if let Some(d) = data {
                    out.push(' ');
                    out.push_str(d);
                }
                out.push_str("?>");
            }
        }
    }
}

fn escape_xml_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_xml_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
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
    /// The matched XML node tree (native IR, replaces xml_fragment string)
    pub xml_node: Option<XmlNode>,
    /// The matched XML fragment string (lazy-serialized from xml_node)
    ///
    /// This is a cached version of `xml_node.to_xml_string()`. It is populated
    /// on first access via `xml_fragment()`.
    xml_fragment_cache: Option<String>,
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
            xml_fragment_cache: None,
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
            xml_fragment_cache: None,
        }
    }

    /// Set the XML node tree for this match
    pub fn with_xml_node(mut self, node: XmlNode) -> Self {
        self.xml_node = Some(node);
        self
    }

    /// Set the XML fragment string directly (backward compat for tests)
    pub fn with_xml_fragment(mut self, xml: String) -> Self {
        self.xml_fragment_cache = Some(xml);
        self
    }

    /// Get the XML fragment as an owned string, serializing from xml_node if needed.
    pub fn xml_fragment_string(&self) -> Option<String> {
        if let Some(ref cached) = self.xml_fragment_cache {
            Some(cached.clone())
        } else if let Some(ref node) = self.xml_node {
            Some(node.to_xml_string())
        } else {
            None
        }
    }

    /// Returns true if this match has an XML tree (either as XmlNode or cached string).
    pub fn has_xml(&self) -> bool {
        self.xml_node.is_some() || self.xml_fragment_cache.is_some()
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
