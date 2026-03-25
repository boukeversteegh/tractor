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

use crate::output::{normalize_path, xml_node_to_string};
use crate::xpath::XmlNode;

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

/// A match with view-selected content fields.
///
/// Core identity fields (file, line, column) are always populated.
/// Content fields are Some only when the corresponding ViewField was in the
/// resolved ViewSet at report-build time.
#[derive(Debug, Clone)]
pub struct ReportMatch {
    // Core identity — always present; used for sorting, grouping, gcc/github templates
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub end_line: u32,
    pub end_column: u32,

    // Content fields — Some only if selected by resolved ViewSet
    /// Native XML node tree; renderers convert directly (text → pretty-print, json → object).
    pub tree:     Option<XmlNode>,
    /// XPath string value of the matched node.
    pub value:    Option<String>,
    /// Pre-computed column-precise source snippet (plain text; coloring in renderer).
    pub source:   Option<String>,
    /// Pre-computed source lines spanning the match (trailing \r stripped).
    pub lines:    Option<Vec<String>>,
    pub reason:   Option<String>,
    pub severity: Option<Severity>,
    pub message:  Option<String>,
    /// Rule identifier for multi-rule reports (future: `--rules` flag).
    pub rule_id:  Option<String>,
    /// Set-command status: "updated" or "unchanged".
    pub status:   Option<String>,
    /// Full modified file content, used by the set command's stdout mode.
    pub output:   Option<String>,
}

impl Serialize for ReportMatch {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let optional_count = self.tree.as_ref().map_or(0, |_| 1)
            + self.value.as_ref().map_or(0, |_| 1)
            + self.source.as_ref().map_or(0, |_| 1)
            + self.lines.as_ref().map_or(0, |_| 1)
            + self.reason.as_ref().map_or(0, |_| 1)
            + self.severity.as_ref().map_or(0, |_| 1)
            + self.message.as_ref().map_or(0, |_| 1)
            + self.rule_id.as_ref().map_or(0, |_| 1)
            + self.status.as_ref().map_or(0, |_| 1)
            + self.output.as_ref().map_or(0, |_| 1);
        let has_file = !self.file.is_empty();
        let core_count = if has_file { 5 } else { 4 };
        let mut map = serializer.serialize_map(Some(core_count + optional_count))?;

        if has_file {
            map.serialize_entry("file", &normalize_path(&self.file))?;
        }
        map.serialize_entry("line", &self.line)?;
        map.serialize_entry("column", &self.column)?;
        map.serialize_entry("end_line", &self.end_line)?;
        map.serialize_entry("end_column", &self.end_column)?;

        if let Some(ref v) = self.tree     { map.serialize_entry("tree", &xml_node_to_string(v))?; }
        if let Some(ref v) = self.value    { map.serialize_entry("value", v)?; }
        if let Some(ref v) = self.source   { map.serialize_entry("source", v)?; }
        if let Some(ref v) = self.lines    { map.serialize_entry("lines", v)?; }
        if let Some(ref v) = self.reason   { map.serialize_entry("reason", v)?; }
        if let Some(ref v) = self.severity { map.serialize_entry("severity", v)?; }
        if let Some(ref v) = self.message  { map.serialize_entry("message", v)?; }
        if let Some(ref v) = self.rule_id  { map.serialize_entry("rule_id", v)?; }
        if let Some(ref v) = self.status   { map.serialize_entry("status", v)?; }
        if let Some(ref v) = self.output   { map.serialize_entry("output", v)?; }

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

    /// The XPath query as received by tractor (set when `-v query` is used).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
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
    Set,
    Run,
}

impl ReportKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ReportKind::Query => "query",
            ReportKind::Check => "check",
            ReportKind::Test => "test",
            ReportKind::Set => "set",
            ReportKind::Run => "run",
        }
    }
}

/// Matches grouped by source file.
#[derive(Debug, Clone, Serialize)]
pub struct FileGroup {
    pub file: String,
    pub matches: Vec<ReportMatch>,
    /// Full modified file content for set stdout mode — placed at group (file) level.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
}

/// The normalized output of a tractor command.
#[derive(Debug, Serialize)]
pub struct Report {
    pub kind: ReportKind,
    pub matches: Vec<ReportMatch>,

    /// Present for check and test reports; absent for query.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<Summary>,

    /// Optional pre-grouped structure. Populated by `with_groups()`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub groups: Option<Vec<FileGroup>>,

    /// Sub-reports for `ReportKind::Run`. Each entry is a complete report
    /// from one operation in the config file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operations: Option<Vec<Report>>,
}

impl Report {
    pub fn set(matches: Vec<ReportMatch>, summary: Summary) -> Self {
        Report { kind: ReportKind::Set, matches, summary: Some(summary), groups: None, operations: None }
    }

    pub fn query(matches: Vec<ReportMatch>, summary: Summary) -> Self {
        Report { kind: ReportKind::Query, matches, summary: Some(summary), groups: None, operations: None }
    }

    pub fn check(matches: Vec<ReportMatch>, summary: Summary) -> Self {
        Report { kind: ReportKind::Check, matches, summary: Some(summary), groups: None, operations: None }
    }

    pub fn test(matches: Vec<ReportMatch>, summary: Summary) -> Self {
        Report { kind: ReportKind::Test, matches, summary: Some(summary), groups: None, operations: None }
    }

    /// Build a unified run report from multiple sub-reports.
    /// Computes an aggregate summary across all operations.
    /// Only check reports contribute to error/warning counts (set reports
    /// reuse these fields for "updated"/"unchanged").
    pub fn run(reports: Vec<Report>) -> Self {
        let mut total = 0usize;
        let mut errors = 0usize;
        let mut warnings = 0usize;
        let mut files_affected = 0usize;
        let mut passed = true;

        for r in &reports {
            if let Some(ref s) = r.summary {
                total += s.total;
                files_affected += s.files_affected;
                if !s.passed {
                    passed = false;
                }
                // Only aggregate errors/warnings from check reports.
                // Set reports reuse errors=updated, warnings=unchanged.
                if matches!(r.kind, ReportKind::Check) {
                    errors += s.errors;
                    warnings += s.warnings;
                }
            }
        }

        Report {
            kind: ReportKind::Run,
            matches: vec![],
            summary: Some(Summary {
                passed,
                total,
                files_affected,
                errors,
                warnings,
                expected: None,
                query: None,
            }),
            groups: None,
            operations: Some(reports),
        }
    }

    /// Serialize this report to pretty-printed JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// Consume flat `matches`, group them by source file, and clear the flat list.
    /// After this call `matches` is empty and `groups` holds all the data.
    /// Renderers can then unconditionally render whichever of the two is non-empty.
    pub fn with_groups(mut self) -> Self {
        let mut groups: Vec<FileGroup> = Vec::new();
        let mut file_index: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

        for mut rm in self.matches.drain(..) {
            let file = normalize_path(&rm.file);
            let idx = file_index.entry(file.clone()).or_insert_with(|| {
                groups.push(FileGroup { file: file.clone(), matches: Vec::new(), output: None });
                groups.len() - 1
            });
            rm.file = String::new();
            groups[*idx].matches.push(rm);
        }
        self.groups = Some(groups);
        self
    }

    /// Attach pre-computed file outputs to groups (set stdout mode).
    /// Must be called after `with_groups()`. Groups whose file is not in `outputs`
    /// are left with `output = None`.
    pub fn with_file_outputs(mut self, outputs: &std::collections::HashMap<String, String>) -> Self {
        if let Some(ref mut groups) = self.groups {
            for group in groups.iter_mut() {
                if let Some(content) = outputs.get(&group.file) {
                    group.output = Some(content.clone());
                }
            }
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_report_match(file: &str, line: u32, col: u32, value: &str) -> ReportMatch {
        ReportMatch {
            file: file.to_string(),
            line,
            column: col,
            end_line: line,
            end_column: col + value.len() as u32,
            tree: None,
            value: Some(value.to_string()),
            source: None,
            lines: None,
            reason: None,
            severity: None,
            message: None,
            rule_id: None,
            status: None,
            output: None,
        }
    }

    #[test]
    fn test_check_report_json() {
        let m1 = ReportMatch {
            file: "src\\main.rs".to_string(),
            line: 10,
            column: 5,
            end_line: 10,
            end_column: 8,
            tree: None,
            value: Some("foo".to_string()),
            source: None,
            lines: None,
            reason: Some("no foo allowed".to_string()),
            severity: Some(Severity::Error),
            message: None,
            rule_id: None,
            status: None,
            output: None,
        };
        let m2 = ReportMatch {
            file: "src/lib.rs".to_string(),
            line: 3,
            column: 1,
            end_line: 3,
            end_column: 4,
            tree: None,
            value: Some("bar".to_string()),
            source: None,
            lines: None,
            reason: Some("no bar allowed".to_string()),
            severity: Some(Severity::Warning),
            message: None,
            rule_id: None,
            status: None,
            output: None,
        };
        let summary = Summary {
            passed: false,
            total: 2,
            files_affected: 2,
            errors: 1,
            warnings: 1,
            expected: None,
            query: None,
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
        let m = make_report_match("test.cs", 1, 1, "x");
        let summary = Summary {
            passed: true,
            total: 1,
            files_affected: 1,
            errors: 0,
            warnings: 0,
            expected: Some("some".to_string()),
            query: None,
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
