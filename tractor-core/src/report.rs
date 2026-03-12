//! Report model types.
//!
//! A `Report` is the normalized output of any tractor command. It is the
//! boundary between stage 2 (report construction) and stage 3 (serialization).
//!
//! Pipeline:
//!   source files → [stage 1: parse + query] → Vec<Match>
//!                → [stage 2: report build]  → Report
//!                → [stage 3: output]        → stdout

use serde::{Serialize, Serializer};
use serde::ser::SerializeMap;

use crate::Match;
use crate::output::normalize_path;

// ---------------------------------------------------------------------------
// Severity
// ---------------------------------------------------------------------------

/// Severity level for check violations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

// ---------------------------------------------------------------------------
// ReportMatch
// ---------------------------------------------------------------------------

/// A match enriched with report-level metadata.
/// Wraps the core `Match` and adds fields that are only meaningful in context
/// of a specific command (check reason, severity, rule identity).
#[derive(Debug, Clone)]
pub struct ReportMatch {
    pub inner: Match,

    /// Violation description — populated by `tractor check --reason`.
    pub reason: Option<String>,

    /// Violation severity — populated by `tractor check --severity`.
    pub severity: Option<Severity>,

    /// Rule identifier for multi-rule reports (future: `--rules` flag).
    pub rule_id: Option<String>,
}

impl ReportMatch {
    pub fn from_match(m: Match) -> Self {
        ReportMatch { inner: m, reason: None, severity: None, rule_id: None }
    }
}

impl Serialize for ReportMatch {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // Count fields: file, line, column, end_line, end_column, value = 6
        // + optional: reason, severity, rule_id
        let optional_count = self.reason.as_ref().map_or(0, |_| 1)
            + self.severity.as_ref().map_or(0, |_| 1)
            + self.rule_id.as_ref().map_or(0, |_| 1);
        let mut map = serializer.serialize_map(Some(6 + optional_count))?;

        map.serialize_entry("file", &normalize_path(&self.inner.file))?;
        map.serialize_entry("line", &self.inner.line)?;
        map.serialize_entry("column", &self.inner.column)?;
        map.serialize_entry("end_line", &self.inner.end_line)?;
        map.serialize_entry("end_column", &self.inner.end_column)?;
        map.serialize_entry("value", &self.inner.value)?;

        if let Some(ref reason) = self.reason {
            map.serialize_entry("reason", reason)?;
        }
        if let Some(ref severity) = self.severity {
            map.serialize_entry("severity", severity)?;
        }
        if let Some(ref rule_id) = self.rule_id {
            map.serialize_entry("rule_id", rule_id)?;
        }

        map.end()
    }
}

// ---------------------------------------------------------------------------
// Summary
// ---------------------------------------------------------------------------

/// Aggregated result summary. Present in check and test reports.
#[derive(Debug, Serialize)]
pub struct Summary {
    /// Did the command succeed (no error-severity violations / expectation met)?
    pub passed: bool,

    /// Total number of matches.
    pub total: usize,

    /// Number of distinct files that had at least one match.
    #[serde(rename = "files")]
    pub files_affected: usize,

    /// Error-severity match count (check only).
    pub errors: usize,

    /// Warning-severity match count (check only).
    pub warnings: usize,

    /// The expected value string for test assertions (`none`, `some`, or a number).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,
}

// ---------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------

/// Which command produced this report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ReportKind {
    Query,
    Check,
    Test,
}

/// The normalized output of a tractor command.
#[derive(Debug, Serialize)]
pub struct Report {
    pub kind: ReportKind,
    pub matches: Vec<ReportMatch>,

    /// Present for check and test reports; absent for query.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<Summary>,
}

impl Report {
    pub fn query(matches: Vec<ReportMatch>, summary: Summary) -> Self {
        Report { kind: ReportKind::Query, matches, summary: Some(summary) }
    }

    pub fn check(matches: Vec<ReportMatch>, summary: Summary) -> Self {
        Report { kind: ReportKind::Check, matches, summary: Some(summary) }
    }

    pub fn test(matches: Vec<ReportMatch>, summary: Summary) -> Self {
        Report { kind: ReportKind::Test, matches, summary: Some(summary) }
    }

    /// Serialize this report to pretty-printed JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn make_match(file: &str, line: u32, col: u32, value: &str) -> Match {
        Match::with_location(
            file.to_string(), line, col, line, col + value.len() as u32,
            value.to_string(), Arc::new(vec![]),
        )
    }

    #[test]
    fn test_check_report_json() {
        let m1 = ReportMatch {
            inner: make_match("src\\main.rs", 10, 5, "foo"),
            reason: Some("no foo allowed".to_string()),
            severity: Some(Severity::Error),
            rule_id: None,
        };
        let m2 = ReportMatch {
            inner: make_match("src/lib.rs", 3, 1, "bar"),
            reason: Some("no bar allowed".to_string()),
            severity: Some(Severity::Warning),
            rule_id: None,
        };
        let summary = Summary {
            passed: false,
            total: 2,
            files_affected: 2,
            errors: 1,
            warnings: 1,
            expected: None,
        };
        let report = Report::check(vec![m1, m2], summary);
        let json = report.to_json();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();

        // Summary
        assert_eq!(v["summary"]["passed"], false);
        assert_eq!(v["summary"]["total"], 2);
        assert_eq!(v["summary"]["files"], 2);
        assert_eq!(v["summary"]["errors"], 1);
        assert_eq!(v["summary"]["warnings"], 1);
        assert!(v["summary"]["expected"].is_null());

        // Matches
        assert_eq!(v["matches"].as_array().unwrap().len(), 2);
        // Backslash normalized to forward slash
        assert_eq!(v["matches"][0]["file"], "src/main.rs");
        assert_eq!(v["matches"][0]["severity"], "error");
        assert_eq!(v["matches"][1]["severity"], "warning");
    }

    #[test]
    fn test_test_report_json() {
        let m = ReportMatch::from_match(make_match("test.cs", 1, 1, "x"));
        let summary = Summary {
            passed: true,
            total: 1,
            files_affected: 1,
            errors: 0,
            warnings: 0,
            expected: Some("some".to_string()),
        };
        let report = Report::test(vec![m], summary);
        let json = report.to_json();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(v["kind"], "test");
        assert_eq!(v["summary"]["passed"], true);
        assert_eq!(v["summary"]["expected"], "some");
        // No reason/severity on plain match
        assert!(v["matches"][0].get("reason").is_none());
        assert!(v["matches"][0].get("severity").is_none());
    }
}
