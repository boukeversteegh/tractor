//! Diagnostic builder for constructing error/info `ReportMatch` items.
//!
//! Diagnostics are `ReportMatch` items with `Fatal` or `Info` severity,
//! representing tractor's own feedback rather than user-defined findings.
//!
//! # Examples
//!
//! ```ignore
//! // Fatal error with hint
//! let report = Diagnostic::fatal("invalid XPath syntax")
//!     .command("check")
//!     .hint("did you mean: //function")
//!     .into_report();
//!
//! // Info feedback
//! let rm = Diagnostic::info("0 matches for //funciton")
//!     .command("query")
//!     .hint("did you mean: //function")
//!     .build();
//!
//! // With CLI source highlighting
//! let report = Diagnostic::fatal("invalid XPath syntax")
//!     .command("check")
//!     .cli_source()
//!     .cli_highlight("//[invalid")
//!     .hint("check syntax near position 3")
//!     .into_report();
//!
//! // With file source
//! let report = Diagnostic::fatal("invalid XPath in rule")
//!     .command("check")
//!     .file("tractor.yaml")
//!     .location(5, 10, 5, 25)
//!     .source_lines(vec!["  xpath: //[invalid".to_string()])
//!     .into_report();
//! ```

use crate::report::{ReportMatch, Report, Severity, DiagnosticOrigin};

// ---------------------------------------------------------------------------
// DiagnosticError — wraps a Report for propagation through Box<dyn Error>
// ---------------------------------------------------------------------------

/// An error that carries a pre-built diagnostic `Report`.
///
/// Used to propagate structured errors through `Result<T, Box<dyn Error>>`
/// while preserving the full report for format-aware rendering in `main()`.
pub struct DiagnosticError(pub Report);

impl std::fmt::Display for DiagnosticError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Extract the first reason for the Display impl (used as fallback)
        if let Some(m) = self.0.all_matches().first() {
            if let Some(ref reason) = m.reason {
                return write!(f, "{}", reason);
            }
        }
        write!(f, "diagnostic error")
    }
}
impl std::fmt::Debug for DiagnosticError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DiagnosticError({})", self)
    }
}
impl std::error::Error for DiagnosticError {}

/// Builder for constructing diagnostic `ReportMatch` items.
pub struct Diagnostic {
    severity: Severity,
    reason: String,
    hint: Option<String>,
    origin: Option<DiagnosticOrigin>,
    command: String,
    file: String,
    line: u32,
    column: u32,
    end_line: u32,
    end_column: u32,
    source: Option<String>,
    lines: Option<Vec<String>>,
}

impl Diagnostic {
    /// Create a fatal diagnostic (tractor broke).
    pub fn fatal(reason: &str) -> Self {
        Self::new(Severity::Fatal, reason)
    }

    /// Create an info diagnostic (helpful feedback).
    pub fn info(reason: &str) -> Self {
        Self::new(Severity::Info, reason)
    }

    fn new(severity: Severity, reason: &str) -> Self {
        Diagnostic {
            severity,
            reason: reason.to_string(),
            hint: None,
            origin: None,
            command: String::new(),
            file: String::new(),
            line: 0,
            column: 0,
            end_line: 0,
            end_column: 0,
            source: None,
            lines: None,
        }
    }

    /// Set the command that was intended (e.g. "check", "query").
    pub fn command(mut self, cmd: &str) -> Self {
        self.command = cmd.to_string();
        self
    }

    /// Set the diagnostic origin (what input was being processed).
    pub fn origin(mut self, origin: DiagnosticOrigin) -> Self {
        self.origin = Some(origin);
        self
    }

    /// Set a suggested fix or hint.
    pub fn hint(mut self, hint: &str) -> Self {
        self.hint = Some(hint.to_string());
        self
    }

    /// Set the source file path.
    pub fn file(mut self, file: &str) -> Self {
        self.file = file.to_string();
        self
    }

    /// Set the source location (1-based line/column).
    pub fn location(mut self, line: u32, col: u32, end_line: u32, end_col: u32) -> Self {
        self.line = line;
        self.column = col;
        self.end_line = end_line;
        self.end_column = end_col;
        self
    }

    /// Set a source snippet for highlighting.
    pub fn source(mut self, source: &str) -> Self {
        self.source = Some(source.to_string());
        self
    }

    /// Set source lines for multi-line highlighting.
    pub fn source_lines(mut self, lines: Vec<String>) -> Self {
        self.lines = Some(lines);
        self
    }

    /// Set source to the reconstructed CLI invocation.
    /// File stays empty — the CLI invocation is not a real file.
    pub fn cli_source(mut self) -> Self {
        self.source = Some(cli_invocation());
        self.line = 1;
        self.end_line = 1;
        self
    }

    /// Highlight a substring within the current source (e.g. a bad CLI argument).
    /// Sets column/end_column to span the first occurrence of `needle`.
    pub fn cli_highlight(mut self, needle: &str) -> Self {
        if let Some(ref src) = self.source {
            if let Some((col, end_col)) = find_span(src, needle) {
                self.column = col;
                self.end_column = end_col;
            }
        }
        self
    }

    /// Build the diagnostic into a `ReportMatch`.
    pub fn build(self) -> ReportMatch {
        ReportMatch {
            file: self.file,
            line: self.line,
            column: self.column,
            end_line: self.end_line,
            end_column: self.end_column,
            command: self.command,
            tree: None,
            value: None,
            source: self.source,
            lines: self.lines,
            reason: Some(self.reason),
            severity: Some(self.severity),
            message: None,
            hint: self.hint,
            origin: self.origin,
            rule_id: None,
            status: None,
            output: None,
        }
    }

    /// Build the diagnostic and wrap it in a single-item `Report`.
    pub fn into_report(self) -> Report {
        let rm = self.build();
        Report::from_diagnostics(vec![rm])
    }
}

// ---------------------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------------------

/// Find the 1-based (column, end_column) span of `needle` within `haystack`.
pub fn find_span(haystack: &str, needle: &str) -> Option<(u32, u32)> {
    haystack.find(needle).map(|offset| {
        let col = offset as u32 + 1; // 1-based
        let end_col = col + needle.len() as u32;
        (col, end_col)
    })
}

/// Convert a byte offset in a multi-line string to 1-based (line, column).
pub fn offset_to_location(content: &str, byte_offset: usize) -> (u32, u32) {
    let mut line = 1u32;
    let mut col = 1u32;
    for (i, ch) in content.char_indices() {
        if i >= byte_offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

/// Reconstruct the CLI invocation string from `std::env::args()`.
pub fn cli_invocation() -> String {
    std::env::args().collect::<Vec<_>>().join(" ")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::Severity;

    #[test]
    fn test_diagnostic_fatal_build() {
        let rm = Diagnostic::fatal("invalid XPath")
            .command("check")
            .hint("did you mean: //function")
            .build();

        assert_eq!(rm.reason.as_deref(), Some("invalid XPath"));
        assert_eq!(rm.severity, Some(Severity::Fatal));
        assert_eq!(rm.hint.as_deref(), Some("did you mean: //function"));
        assert_eq!(rm.command, "check");
        assert!(rm.file.is_empty());
        assert_eq!(rm.line, 0);
    }

    #[test]
    fn test_diagnostic_info_build() {
        let rm = Diagnostic::info("0 matches")
            .command("query")
            .build();

        assert_eq!(rm.severity, Some(Severity::Info));
        assert!(rm.hint.is_none());
    }

    #[test]
    fn test_diagnostic_with_file_location() {
        let rm = Diagnostic::fatal("parse error")
            .file("config.yaml")
            .location(5, 10, 5, 25)
            .source_lines(vec!["  xpath: //[invalid".to_string()])
            .build();

        assert_eq!(rm.file, "config.yaml");
        assert_eq!(rm.line, 5);
        assert_eq!(rm.column, 10);
        assert_eq!(rm.end_line, 5);
        assert_eq!(rm.end_column, 25);
        assert!(rm.lines.is_some());
    }

    #[test]
    fn test_diagnostic_into_report() {
        let report = Diagnostic::fatal("broken")
            .command("check")
            .into_report();

        assert_eq!(report.success, Some(false));
        let totals = report.totals.as_ref().unwrap();
        assert_eq!(totals.fatals, 1);
        assert_eq!(totals.errors, 0);
        assert_eq!(totals.results, 1);
    }

    #[test]
    fn test_diagnostic_info_report_success() {
        let report = Diagnostic::info("helpful note")
            .into_report();

        // Info-only reports should succeed
        assert_eq!(report.success, Some(true));
        let totals = report.totals.as_ref().unwrap();
        assert_eq!(totals.infos, 1);
        assert_eq!(totals.fatals, 0);
    }

    #[test]
    fn test_find_span() {
        assert_eq!(find_span("tractor check -x '//['", "//["), Some((19, 22)));
        assert_eq!(find_span("hello world", "xyz"), None);
        assert_eq!(find_span("abc", "abc"), Some((1, 4)));
    }

    #[test]
    fn test_offset_to_location() {
        assert_eq!(offset_to_location("hello\nworld", 0), (1, 1));
        assert_eq!(offset_to_location("hello\nworld", 5), (1, 6));
        assert_eq!(offset_to_location("hello\nworld", 6), (2, 1));
        assert_eq!(offset_to_location("hello\nworld", 8), (2, 3));
    }
}
