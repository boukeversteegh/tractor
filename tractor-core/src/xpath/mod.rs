//! XPath 3.1 query engine using xee-xpath
//!
//! This module provides XPath query capabilities for the parsed XML AST.

mod engine;
mod match_result;

pub use engine::{XPathEngine, print_timing_stats};
pub use match_result::Match;

// Re-export xee-xpath types needed for direct query API
pub use xee_xpath::{Documents, DocumentHandle};

use serde::Serialize;
use thiserror::Error;

/// Errors that can occur during XPath evaluation
#[derive(Error, Debug)]
pub enum XPathError {
    #[error("Failed to compile XPath: {0}")]
    Compile(String),
    #[error("Failed to execute XPath: {0}")]
    Execute(String),
    #[error("Failed to parse XML: {0}")]
    XmlParse(String),
}

/// Result of validating an XPath expression
#[derive(Debug, Clone, Serialize)]
pub struct ValidationResult {
    /// Whether the XPath is valid
    pub valid: bool,
    /// Error message if invalid
    pub error: Option<String>,
    /// Error position (start index in the query string)
    pub error_start: Option<usize>,
    /// Error end position (end index in the query string)
    pub error_end: Option<usize>,
    /// Warnings (non-fatal issues)
    pub warnings: Vec<String>,
}

impl ValidationResult {
    /// Create a valid result
    pub fn ok() -> Self {
        ValidationResult {
            valid: true,
            error: None,
            error_start: None,
            error_end: None,
            warnings: Vec::new(),
        }
    }

    /// Create an invalid result with error
    pub fn err(message: String) -> Self {
        ValidationResult {
            valid: false,
            error: Some(message),
            error_start: None,
            error_end: None,
            warnings: Vec::new(),
        }
    }

    /// Create an invalid result with error and position
    pub fn err_at(message: String, start: usize, end: usize) -> Self {
        ValidationResult {
            valid: false,
            error: Some(message),
            error_start: Some(start),
            error_end: Some(end),
            warnings: Vec::new(),
        }
    }

    /// Add a warning
    pub fn with_warning(mut self, warning: String) -> Self {
        self.warnings.push(warning);
        self
    }
}

/// Validate an XPath expression without executing it
///
/// Returns a ValidationResult indicating whether the expression is valid
/// and any errors or warnings.
pub fn validate_xpath(xpath: &str) -> ValidationResult {
    use xee_xpath::Queries;

    // Empty query check
    let trimmed = xpath.trim();
    if trimmed.is_empty() {
        return ValidationResult::err("XPath expression is empty".to_string());
    }

    // Try to compile the query
    let queries = Queries::default();
    match queries.sequence(xpath) {
        Ok(_) => {
            let mut result = ValidationResult::ok();

            // Add warnings for common issues
            if trimmed.starts_with('/') && !trimmed.starts_with("//") {
                // Absolute path - might not match if not starting from root
                // This is not an error, just informational
            }

            if trimmed.contains("text()") {
                result = result.with_warning(
                    "text() selects text nodes; use string() to get text value".to_string()
                );
            }

            result
        }
        Err(e) => {
            let error_msg = e.to_string();
            let (message, start, end) = parse_xpath_error(&error_msg);
            match (start, end) {
                (Some(s), Some(e)) => ValidationResult::err_at(message, s, e),
                _ => ValidationResult::err(message),
            }
        }
    }
}

/// Parse XPath error message and extract position info
///
/// xee-xpath errors look like: "XPST0003 Parse error. (8..8)"
/// Returns: (clean_message, start_pos, end_pos)
fn parse_xpath_error(error: &str) -> (String, Option<usize>, Option<usize>) {
    // Extract position range like (8..8) or (0..18)
    let (start_pos, end_pos) = extract_position_range(error);

    // Remove the error code (XPST0003, etc.)
    let msg = if let Some(space_pos) = error.find(' ') {
        if error[..space_pos].starts_with("XP") {
            error[space_pos + 1..].to_string()
        } else {
            error.to_string()
        }
    } else {
        error.to_string()
    };

    // Remove the position suffix like (8..8)
    let msg_clean = if let Some(paren_pos) = msg.rfind(" (") {
        msg[..paren_pos].trim_end_matches('.').to_string()
    } else {
        msg.trim_end_matches('.').to_string()
    };

    (msg_clean, start_pos, end_pos)
}

/// Extract position range from error string like "(8..8)" -> (Some(8), Some(8))
fn extract_position_range(error: &str) -> (Option<usize>, Option<usize>) {
    // Find pattern like (8..8) at the end
    let start = match error.rfind('(') {
        Some(s) => s,
        None => return (None, None),
    };
    let end = match error.rfind(')') {
        Some(e) => e,
        None => return (None, None),
    };
    if end <= start {
        return (None, None);
    }

    let range_str = &error[start + 1..end];
    let parts: Vec<&str> = range_str.split("..").collect();
    if parts.len() != 2 {
        return (None, None);
    }

    let start_pos: usize = match parts[0].parse() {
        Ok(p) => p,
        Err(_) => return (None, None),
    };
    let end_pos: usize = match parts[1].parse() {
        Ok(p) => p,
        Err(_) => return (None, None),
    };

    (Some(start_pos), Some(end_pos))
}

#[cfg(test)]
mod validation_tests {
    use super::*;

    #[test]
    fn test_valid_xpath() {
        let result = validate_xpath("//class");
        assert!(result.valid);
        assert!(result.error.is_none());
    }

    #[test]
    fn test_valid_xpath_with_predicate() {
        let result = validate_xpath("//method[@name='test']");
        assert!(result.valid);
    }

    #[test]
    fn test_valid_xpath_function() {
        let result = validate_xpath("count(//class)");
        assert!(result.valid);
    }

    #[test]
    fn test_invalid_xpath_syntax() {
        let result = validate_xpath("//class[");
        assert!(!result.valid);
        assert!(result.error.is_some());
        // Check the error message is human-readable
        let err = result.error.unwrap();
        assert!(err.contains("Parse error"), "Error should mention parse error: {}", err);
        // Check position is captured
        assert!(result.error_start.is_some(), "Should have error start position");
        assert!(result.error_end.is_some(), "Should have error end position");
    }

    #[test]
    fn test_empty_xpath() {
        let result = validate_xpath("");
        assert!(!result.valid);
        assert!(result.error.unwrap().contains("empty"));
    }

    #[test]
    fn test_text_warning() {
        let result = validate_xpath("//name/text()");
        assert!(result.valid);
        assert!(!result.warnings.is_empty());
    }

    #[test]
    fn test_parse_xpath_error() {
        let (msg, start, end) = parse_xpath_error("XPST0003 Parse error. (8..8)");
        assert_eq!(msg, "Parse error");
        assert_eq!(start, Some(8));
        assert_eq!(end, Some(8));

        let (msg, start, end) = parse_xpath_error("XPST0017 Type error: incorrect function name or number of arguments. (0..18)");
        assert_eq!(msg, "Type error: incorrect function name or number of arguments");
        assert_eq!(start, Some(0));
        assert_eq!(end, Some(18));
    }
}
