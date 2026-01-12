//! XPath 3.1 query engine implementation

use super::{Match, XPathError};
use regex::Regex;
use xee_xpath::Query;  // Import the Query trait for execute method

/// XPath query engine using xee-xpath
pub struct XPathEngine {
    verbose: bool,
}

impl XPathEngine {
    /// Create a new XPath engine
    pub fn new() -> Self {
        XPathEngine { verbose: false }
    }

    /// Enable verbose mode for debugging
    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Execute an XPath query against XML and return matches
    pub fn query(
        &self,
        xml: &str,
        xpath: &str,
        source_lines: &[String],
        file_path: &str,
    ) -> Result<Vec<Match>, XPathError> {
        use xee_xpath::{Documents, Queries};

        // Load XML into xee-xpath
        let mut documents = Documents::new();
        let doc = documents
            .add_string(
                "file:///query".try_into().unwrap(),
                xml,
            )
            .map_err(|e| XPathError::XmlParse(e.to_string()))?;

        // Compile and execute the query
        let queries = Queries::default();
        let query = queries
            .sequence(xpath)
            .map_err(|e| XPathError::Compile(e.to_string()))?;

        let results = query
            .execute(&mut documents, doc)
            .map_err(|e| XPathError::Execute(e.to_string()))?;

        // Convert results to Match objects
        let mut matches = Vec::new();

        for item in results.iter() {
            match item {
                xee_xpath::Item::Node(node) => {
                    // Get the XML fragment using Xot's to_string
                    let xot = documents.xot();
                    let xml_fragment = xot.to_string(node).unwrap_or_default();

                    let (line, col, end_line, end_col) = self.extract_location(&xml_fragment);
                    let value = xot.string_value(node);
                    let actual_file = self.extract_file_path_from_xml(&xml_fragment, file_path);

                    let m = Match::with_location(
                        actual_file,
                        line,
                        col,
                        end_line,
                        end_col,
                        value,
                        source_lines.to_vec(),
                    ).with_xml_fragment(xml_fragment);

                    matches.push(m);
                }
                xee_xpath::Item::Atomic(atomic) => {
                    // Atomic value (string, number, boolean, etc.)
                    let value = atomic.to_string().unwrap_or_default();
                    matches.push(Match::new(file_path.to_string(), value));
                }
                xee_xpath::Item::Function(_) => {
                    // Function items - skip for now
                }
            }
        }

        Ok(matches)
    }

    /// Extract location from XML fragment attributes
    fn extract_location(&self, xml: &str) -> (u32, u32, u32, u32) {
        let mut line = 1u32;
        let mut col = 1u32;
        let mut end_line = 1u32;
        let mut end_col = 1u32;

        // Try compact format: start="line:col" end="line:col"
        let start_re = Regex::new(r#"start="(\d+):(\d+)""#).unwrap();
        let end_re = Regex::new(r#"end="(\d+):(\d+)""#).unwrap();

        if let Some(caps) = start_re.captures(xml) {
            line = caps.get(1).and_then(|m| m.as_str().parse().ok()).unwrap_or(1);
            col = caps.get(2).and_then(|m| m.as_str().parse().ok()).unwrap_or(1);
        }

        if let Some(caps) = end_re.captures(xml) {
            end_line = caps.get(1).and_then(|m| m.as_str().parse().ok()).unwrap_or(line);
            end_col = caps.get(2).and_then(|m| m.as_str().parse().ok()).unwrap_or(col);
        }

        // Fallback to legacy format: startLine, startCol, endLine, endCol
        if line == 1 && col == 1 {
            let legacy_re = Regex::new(r#"startLine="(\d+)"\s+startCol="(\d+)"\s+endLine="(\d+)"\s+endCol="(\d+)""#).unwrap();
            if let Some(caps) = legacy_re.captures(xml) {
                line = caps.get(1).and_then(|m| m.as_str().parse().ok()).unwrap_or(1);
                col = caps.get(2).and_then(|m| m.as_str().parse().ok()).unwrap_or(1);
                end_line = caps.get(3).and_then(|m| m.as_str().parse().ok()).unwrap_or(1);
                end_col = caps.get(4).and_then(|m| m.as_str().parse().ok()).unwrap_or(1);
            }
        }

        (line, col, end_line, end_col)
    }

    /// Extract file path from XML context (looks for ancestor File element)
    fn extract_file_path_from_xml(&self, _xml: &str, default: &str) -> String {
        // For now, return the default
        // Could parse XML to find File/@path attribute if needed
        default.to_string()
    }

    /// Strip location metadata from XML
    pub fn strip_location_metadata(xml: &str) -> String {
        let re = Regex::new(r#"\s*(start|end|startLine|startCol|endLine|endCol)="[^"]*""#).unwrap();
        re.replace_all(xml, "").to_string()
    }
}

impl Default for XPathEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_location_compact() {
        let engine = XPathEngine::new();
        let xml = r#"<class start="5:10" end="10:2">Foo</class>"#;
        let (line, col, end_line, end_col) = engine.extract_location(xml);
        assert_eq!(line, 5);
        assert_eq!(col, 10);
        assert_eq!(end_line, 10);
        assert_eq!(end_col, 2);
    }

    #[test]
    fn test_strip_location_metadata() {
        let xml = r#"<class start="1:1" end="5:2">Foo</class>"#;
        let stripped = XPathEngine::strip_location_metadata(xml);
        assert_eq!(stripped, "<class>Foo</class>");
    }
}
