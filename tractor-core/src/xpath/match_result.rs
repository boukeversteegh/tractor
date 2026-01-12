//! Match result types for XPath queries

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
    /// Original source lines for location-based output
    pub source_lines: Vec<String>,
    /// The matched XML fragment (for XML output)
    pub xml_fragment: Option<String>,
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
            source_lines: Vec::new(),
            xml_fragment: None,
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
        source_lines: Vec<String>,
    ) -> Self {
        Match {
            file,
            line,
            column,
            end_line,
            end_column,
            value,
            source_lines,
            xml_fragment: None,
        }
    }

    /// Set the XML fragment for this match
    pub fn with_xml_fragment(mut self, xml: String) -> Self {
        self.xml_fragment = Some(xml);
        self
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
