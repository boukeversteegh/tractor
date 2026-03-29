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

    /// Operation type that produced this match ("check", "query", "test", "set", "update").
    pub command: String,

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
        let has_command = !self.command.is_empty();
        let core_count = if has_file { 5 } else { 4 };
        let command_count = if has_command { 1 } else { 0 };
        let mut map = serializer.serialize_map(Some(core_count + command_count + optional_count))?;

        if has_file {
            map.serialize_entry("file", &normalize_path(&self.file))?;
        }
        map.serialize_entry("line", &self.line)?;
        map.serialize_entry("column", &self.column)?;
        map.serialize_entry("end_line", &self.end_line)?;
        map.serialize_entry("end_column", &self.end_column)?;
        if has_command {
            map.serialize_entry("command", &self.command)?;
        }

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
// Totals
// ---------------------------------------------------------------------------

fn is_zero(v: &usize) -> bool { *v == 0 }

/// Numeric aggregates for a report or group. Contains only counts —
/// the verdict (`passed`) lives on the Report itself.
#[derive(Debug, Clone, Serialize)]
pub struct Totals {
    /// Number of results (matches).
    pub results: usize,

    /// Number of distinct files with at least one result.
    pub files: usize,

    /// Error-severity count (check).
    #[serde(skip_serializing_if = "is_zero")]
    pub errors: usize,

    /// Warning-severity count (check).
    #[serde(skip_serializing_if = "is_zero")]
    pub warnings: usize,

    /// Files/mappings that were changed (set).
    #[serde(skip_serializing_if = "is_zero")]
    pub updated: usize,

    /// Files/mappings already in sync (set).
    #[serde(skip_serializing_if = "is_zero")]
    pub unchanged: usize,
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

// ---------------------------------------------------------------------------
// ResultItem — recursive result type
// ---------------------------------------------------------------------------

/// An item in a report's `results` list: either a leaf match or a sub-group.
#[derive(Debug, Clone)]
pub enum ResultItem {
    Match(ReportMatch),
    Group(Box<Report>),
}

impl ResultItem {
    /// Get a reference to the match if this is a Match variant.
    pub fn as_match(&self) -> Option<&ReportMatch> {
        match self {
            ResultItem::Match(m) => Some(m),
            ResultItem::Group(_) => None,
        }
    }

    /// Get a mutable reference to the match if this is a Match variant.
    pub fn as_match_mut(&mut self) -> Option<&mut ReportMatch> {
        match self {
            ResultItem::Match(m) => Some(m),
            ResultItem::Group(_) => None,
        }
    }

    /// Get a reference to the sub-group report if this is a Group variant.
    pub fn as_group(&self) -> Option<&Report> {
        match self {
            ResultItem::Match(_) => None,
            ResultItem::Group(r) => Some(r),
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
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub kind: ReportKind,
    pub matches: Vec<ReportMatch>,

    /// Did the command succeed? False if check errors, test failures, or set drift.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub success: Option<bool>,

    /// Numeric aggregates (result count, file count, command-specific counts).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub totals: Option<Totals>,

    /// Test-specific: the expected value string (`none`, `some`, or a number).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,

    /// The XPath query as received by tractor (set when `-v query` is used).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,

    /// Optional pre-grouped structure. Populated by `with_groups()`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub groups: Option<Vec<FileGroup>>,

    /// Sub-reports for `ReportKind::Run`. Each entry is a complete report
    /// from one operation in the config file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operations: Option<Vec<Report>>,

    // ---- New unified fields (Step 3) ----

    /// Unified results list. Contains either leaf matches or sub-groups.
    /// Populated by `build_results()` or `with_results_grouped()`.
    /// During migration, may be empty while old fields are still in use.
    #[serde(skip)]
    pub results: Vec<ResultItem>,

    /// What the children in `results` are grouped by ("file", "command").
    /// None when `results` contains ungrouped leaf matches.
    #[serde(skip)]
    pub group: Option<String>,

    /// Hoisted file path when this Report represents a file group.
    #[serde(skip)]
    pub file: Option<String>,

    /// Full modified file content for set stdout mode (group-level).
    /// Moved from FileGroup to Report so groups can be represented as Reports.
    #[serde(skip)]
    pub output_content: Option<String>,
}

impl Report {
    pub fn set(matches: Vec<ReportMatch>, success: bool, totals: Totals) -> Self {
        Report { kind: ReportKind::Set, matches, success: Some(success), totals: Some(totals), expected: None, query: None, groups: None, operations: None, results: vec![], group: None, file: None, output_content: None }
    }

    pub fn query(matches: Vec<ReportMatch>, totals: Totals) -> Self {
        Report { kind: ReportKind::Query, matches, success: None, totals: Some(totals), expected: None, query: None, groups: None, operations: None, results: vec![], group: None, file: None, output_content: None }
    }

    pub fn check(matches: Vec<ReportMatch>, success: bool, totals: Totals) -> Self {
        Report { kind: ReportKind::Check, matches, success: Some(success), totals: Some(totals), expected: None, query: None, groups: None, operations: None, results: vec![], group: None, file: None, output_content: None }
    }

    pub fn test(matches: Vec<ReportMatch>, success: bool, totals: Totals) -> Self {
        Report { kind: ReportKind::Test, matches, success: Some(success), totals: Some(totals), expected: None, query: None, groups: None, operations: None, results: vec![], group: None, file: None, output_content: None }
    }

    /// Build a unified run report from multiple sub-reports.
    /// Computes aggregate totals across all operations.
    pub fn run(reports: Vec<Report>) -> Self {
        let mut total = 0usize;
        let mut errors = 0usize;
        let mut warnings = 0usize;
        let mut updated = 0usize;
        let mut unchanged = 0usize;
        let mut files = 0usize;
        let mut success = true;

        for r in &reports {
            if let Some(ref t) = r.totals {
                total += t.results;
                files += t.files;
                errors += t.errors;
                warnings += t.warnings;
                updated += t.updated;
                unchanged += t.unchanged;
            }
            if let Some(s) = r.success {
                if !s { success = false; }
            }
        }

        Report {
            kind: ReportKind::Run,
            matches: vec![],
            success: Some(success),
            totals: Some(Totals {
                results: total,
                files,
                errors,
                warnings,
                updated,
                unchanged,
            }),
            expected: None,
            query: None,
            groups: None,
            operations: Some(reports),
            results: vec![],
            group: None,
            file: None,
            output_content: None,
        }
    }

    /// Serialize this report to pretty-printed JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }

    // ---- ResultItem helpers ----

    /// Collect references to all leaf matches, recursing into groups.
    /// Prefers `results` if populated, falls back to old fields.
    pub fn all_matches(&self) -> Vec<&ReportMatch> {
        if !self.results.is_empty() {
            let mut out = Vec::new();
            Self::collect_matches_recursive(&self.results, &mut out);
            return out;
        }
        // Fallback to old fields
        if let Some(ref groups) = self.groups {
            return groups.iter().flat_map(|g| g.matches.iter()).collect();
        }
        self.matches.iter().collect()
    }

    /// Collect mutable references to all leaf matches, recursing into groups.
    /// Prefers `results` if populated, falls back to old fields.
    pub fn all_matches_mut(&mut self) -> Vec<&mut ReportMatch> {
        if !self.results.is_empty() {
            let mut out = Vec::new();
            Self::collect_matches_mut_recursive(&mut self.results, &mut out);
            return out;
        }
        // Fallback to old fields
        if let Some(ref mut groups) = self.groups {
            return groups.iter_mut().flat_map(|g| g.matches.iter_mut()).collect();
        }
        self.matches.iter_mut().collect()
    }

    fn collect_matches_recursive<'a>(items: &'a [ResultItem], out: &mut Vec<&'a ReportMatch>) {
        for item in items {
            match item {
                ResultItem::Match(m) => out.push(m),
                ResultItem::Group(g) => Self::collect_matches_recursive(&g.results, out),
            }
        }
    }

    fn collect_matches_mut_recursive<'a>(items: &'a mut [ResultItem], out: &mut Vec<&'a mut ReportMatch>) {
        for item in items {
            match item {
                ResultItem::Match(m) => out.push(m),
                ResultItem::Group(g) => Self::collect_matches_mut_recursive(&mut g.results, out),
            }
        }
    }

    /// Consume flat `matches`, group them by source file, and clear the flat list.
    /// Populates both old `groups` field and new `results` field.
    pub fn with_groups(mut self) -> Self {
        let mut old_groups: Vec<FileGroup> = Vec::new();
        let mut new_groups: Vec<ResultItem> = Vec::new();
        let mut file_index: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

        for mut rm in self.matches.drain(..) {
            let file = normalize_path(&rm.file);
            let idx = *file_index.entry(file.clone()).or_insert_with(|| {
                old_groups.push(FileGroup { file: file.clone(), matches: Vec::new(), output: None });
                new_groups.push(ResultItem::Group(Box::new(Report {
                    kind: self.kind,
                    matches: vec![],
                    success: None,
                    totals: None,
                    expected: None,
                    query: None,
                    groups: None,
                    operations: None,
                    results: vec![],
                    group: None,
                    file: Some(file.clone()),
                    output_content: None,
                })));
                old_groups.len() - 1
            });
            rm.file = String::new();
            old_groups[idx].matches.push(rm.clone());
            // Add to new results structure
            if let ResultItem::Group(ref mut g) = new_groups[idx] {
                g.results.push(ResultItem::Match(rm));
            }
        }

        self.groups = Some(old_groups);
        self.results = new_groups;
        self.group = Some("file".to_string());
        self
    }

    /// Attach pre-computed file outputs to groups (set stdout mode).
    /// Must be called after `with_groups()`. Updates both old and new structures.
    pub fn with_file_outputs(mut self, outputs: &std::collections::HashMap<String, String>) -> Self {
        // Update old groups
        if let Some(ref mut groups) = self.groups {
            for group in groups.iter_mut() {
                if let Some(content) = outputs.get(&group.file) {
                    group.output = Some(content.clone());
                }
            }
        }
        // Update new results
        for item in &mut self.results {
            if let ResultItem::Group(ref mut g) = item {
                if let Some(ref file) = g.file {
                    if let Some(content) = outputs.get(file.as_str()) {
                        g.output_content = Some(content.clone());
                    }
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
            command: String::new(),
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
            command: "check".to_string(),
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
            command: "check".to_string(),
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
        let totals = Totals {
            results: 2,
            files: 2,
            errors: 1,
            warnings: 1,
            updated: 0,
            unchanged: 0,
        };
        let report = Report::check(vec![m1, m2], false, totals);
        let json = report.to_json();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();

        // Totals + passed
        assert_eq!(v["success"], false);
        assert_eq!(v["totals"]["results"], 2);
        assert_eq!(v["totals"]["files"], 2);
        assert_eq!(v["totals"]["errors"], 1);
        assert_eq!(v["totals"]["warnings"], 1);

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
        let totals = Totals {
            results: 1,
            files: 1,
            errors: 0,
            warnings: 0,
            updated: 0,
            unchanged: 0,
        };
        let mut report = Report::test(vec![m], true, totals);
        report.expected = Some("some".to_string());
        let json = report.to_json();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(v["kind"], "test");
        assert_eq!(v["success"], true);
        assert_eq!(v["expected"], "some");
        // No reason/severity on plain match
        assert!(v["matches"][0].get("reason").is_none());
        assert!(v["matches"][0].get("severity").is_none());
    }
}
