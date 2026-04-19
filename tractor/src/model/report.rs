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
use serde::ser::{SerializeMap, SerializeStruct};

use crate::normalized_xpath::NormalizedXpath;
use crate::output::{normalize_path, xml_node_to_string};
use crate::xpath::XmlNode;

// ---------------------------------------------------------------------------
// Severity
// ---------------------------------------------------------------------------

/// Severity level for report matches.
///
/// Four levels, ordered by priority:
/// - `Fatal` — tractor itself broke (invalid input, missing tool). Always `success: false`.
/// - `Error` — user-defined rule violation at error level.
/// - `Warning` — user-defined rule violation at warning level.
/// - `Info` — helpful tractor feedback (e.g. 0 matches, suggestions). Does not affect success.
///
/// Users can set `--severity error|warning` on their rules. `Fatal` and `Info` are
/// reserved for tractor-generated diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Fatal,
    Error,
    Warning,
    Info,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Fatal => "fatal",
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        }
    }
}

// ---------------------------------------------------------------------------
// DiagnosticOrigin
// ---------------------------------------------------------------------------

/// Where a diagnostic (fatal/info) originated — what input was being processed.
///
/// Only meaningful when `file` is empty (no real file to point at).
/// Renderers display this in place of the file path for context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticOrigin {
    /// The XPath expression itself was invalid.
    Xpath,
    /// A CLI argument was invalid or missing.
    Cli,
    /// A tractor config file had an error.
    Config,
    /// A source file couldn't be parsed.
    Input,
}

impl DiagnosticOrigin {
    pub fn as_str(&self) -> &'static str {
        match self {
            DiagnosticOrigin::Xpath => "xpath",
            DiagnosticOrigin::Cli => "cli",
            DiagnosticOrigin::Config => "config",
            DiagnosticOrigin::Input => "input",
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
    /// Where a diagnostic originated (shown when file is empty).
    pub origin:   Option<DiagnosticOrigin>,
    /// Rule identifier for multi-rule reports (via `--config` or `run`).
    pub rule_id:  Option<String>,
    /// Set-command status: "updated" or "unchanged".
    pub status:   Option<String>,
    /// Full modified file content, used by the set command's stdout mode.
    pub output:   Option<String>,
}

/// A captured output payload produced by an operation.
///
/// Outputs are distinct from diagnostic matches: they carry generated content
/// (e.g. captured `set --stdout` file contents) and are rendered separately.
/// An output with `file: Some(path)` belongs to that file; `None` means the
/// payload has no file identity (e.g. stdin → stdout).
#[derive(Debug, Clone, Serialize)]
pub struct ReportOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    pub content: String,
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
            + self.origin.as_ref().map_or(0, |_| 1)
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
        if let Some(ref v) = self.origin   { map.serialize_entry("origin", v)?; }
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

    /// Fatal-severity count (tractor errors).
    #[serde(skip_serializing_if = "is_zero")]
    pub fatals: usize,

    /// Error-severity count (check).
    #[serde(skip_serializing_if = "is_zero")]
    pub errors: usize,

    /// Warning-severity count (check).
    #[serde(skip_serializing_if = "is_zero")]
    pub warnings: usize,

    /// Info-severity count (tractor feedback).
    #[serde(skip_serializing_if = "is_zero")]
    pub infos: usize,

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

/// The normalized output of a tractor command.
///
/// A Report is a recursive group structure. The root and sub-groups share
/// the same type. `results` contains either leaf matches or sub-groups.
///
/// Internally the summary fields (`success`, `totals`, `expected`, `query`)
/// are kept flat for ergonomic access. On the serialized boundary they are
/// nested under a `summary` object so that `-p summary` names a real element.
#[derive(Debug, Clone)]
pub struct Report {
    /// Did the command succeed? False if check errors, test failures, or set drift.
    pub success: Option<bool>,

    /// Numeric aggregates (result count, file count, command-specific counts).
    pub totals: Option<Totals>,

    /// Test-specific: the expected value string (`none`, `some`, or a number).
    pub expected: Option<String>,

    /// The XPath query as received by tractor (set when `-v query` is used).
    pub query: Option<NormalizedXpath>,

    /// Structural overview of matched nodes, captured as opaque text so `-p schema`
    /// has a concrete element to project. Populated when `-v schema` or `-p schema`
    /// is requested; otherwise `None` so schema computation stays opt-in.
    pub schema: Option<String>,

    /// Captured output payloads produced by this report's operation (or,
    /// for sub-groups, payloads that were distributed down to this group
    /// by `with_grouping`). Distinct from diagnostic matches.
    pub outputs: Vec<ReportOutput>,

    /// Unified results list. Contains either leaf matches or sub-groups.
    pub results: Vec<ResultItem>,

    /// What the children in `results` are grouped by ("file", "command", "rule_id").
    /// None when `results` contains ungrouped leaf matches.
    pub group: Option<String>,

    /// Hoisted file path (when this Report is a file group).
    pub file: Option<String>,

    /// Hoisted command (when this Report is a command group).
    pub command: Option<String>,

    /// Hoisted rule_id (when this Report is a rule group).
    pub rule_id: Option<String>,
}

/// The set of top-level summary fields, nested under `summary` in serialized
/// output. Used as the source of truth for the `-p summary` projection so the
/// `<summary>` element shape is owned by one place.
#[derive(Debug, Clone, Serialize)]
pub struct Summary<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub success: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub totals: Option<&'a Totals>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<&'a NormalizedXpath>,
}

impl<'a> Summary<'a> {
    pub fn from_report(report: &'a Report) -> Self {
        Summary {
            success: report.success,
            totals: report.totals.as_ref(),
            expected: report.expected.as_deref(),
            query: report.query.as_ref(),
        }
    }

    /// True when none of the summary fields are populated — lets renderers skip
    /// emitting an empty `<summary>` element in query mode without a verdict.
    pub fn is_empty(&self) -> bool {
        self.success.is_none()
            && self.totals.is_none()
            && self.expected.is_none()
            && self.query.is_none()
    }
}

impl Serialize for Report {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let summary = Summary::from_report(self);
        let has_summary = !summary.is_empty();
        let has_schema = self.schema.is_some();
        let has_outputs = !self.outputs.is_empty();

        let field_count = has_summary as usize + has_schema as usize + has_outputs as usize;
        let mut st = serializer.serialize_struct("Report", field_count)?;
        if has_summary {
            st.serialize_field("summary", &summary)?;
        }
        if let Some(ref schema) = self.schema {
            st.serialize_field("schema", schema)?;
        }
        if has_outputs {
            st.serialize_field("outputs", &self.outputs)?;
        }
        st.end()
    }
}

impl Report {
    /// Serialize this report to pretty-printed JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }

    // ---- ResultItem helpers ----

    /// Collect references to all leaf matches, recursing into groups.
    pub fn all_matches(&self) -> Vec<&ReportMatch> {
        let mut out = Vec::new();
        Self::collect_matches_recursive(&self.results, &mut out);
        out
    }

    /// Collect mutable references to all leaf matches, recursing into groups.
    pub fn all_matches_mut(&mut self) -> Vec<&mut ReportMatch> {
        let mut out = Vec::new();
        Self::collect_matches_mut_recursive(&mut self.results, &mut out);
        out
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

    /// Group results by a single dimension.
    ///
    /// Extracts a key from each leaf match, partitions into sub-groups,
    /// and hoists the key value to the group. Matches retain all their fields
    /// — the renderer decides whether to omit redundant fields from output.
    pub fn group_by(mut self, dimension: &str) -> Self {
        let mut groups: Vec<ResultItem> = Vec::new();
        let mut index: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

        let old_results = std::mem::take(&mut self.results);
        for item in old_results {
            if let ResultItem::Match(rm) = item {
                let key = match dimension {
                    "file" => normalize_path(&rm.file),
                    "command" => rm.command.clone(),
                    "rule_id" => rm.rule_id.clone().unwrap_or_default(),
                    _ => String::new(),
                };
                if key.is_empty() && dimension != "file" {
                    // No key value — leave ungrouped
                    groups.push(ResultItem::Match(rm));
                    continue;
                }
                let idx = *index.entry(key.clone()).or_insert_with(|| {
                    let mut sub = Report::empty();
                    // Hoist the key value to the group — same field name as on a match
                    match dimension {
                        "file" => sub.file = Some(key.clone()),
                        "command" => sub.command = Some(key.clone()),
                        "rule_id" => sub.rule_id = Some(key.clone()),
                        _ => {}
                    }
                    groups.push(ResultItem::Group(Box::new(sub)));
                    groups.len() - 1
                });
                if let ResultItem::Group(ref mut g) = groups[idx] {
                    g.results.push(ResultItem::Match(rm));
                }
            } else {
                // Non-match items (sub-groups) pass through
                groups.push(item);
            }
        }

        self.results = groups;
        self.group = Some(dimension.to_string());
        self
    }

    /// Apply multi-level grouping. Each dimension partitions results into
    /// groups, with nested dimensions applied recursively within each group.
    /// Matches retain all their fields — renderers decide what to omit.
    ///
    /// After grouping, file-bound outputs are distributed down into their
    /// matching file groups (see `distribute_outputs`).
    pub fn with_grouping(mut self, dimensions: &[&str]) -> Self {
        if dimensions.is_empty() {
            return self;
        }
        self = self.with_grouping_inner(dimensions);
        self.distribute_outputs();
        self
    }

    fn with_grouping_inner(mut self, dimensions: &[&str]) -> Self {
        if dimensions.is_empty() {
            return self;
        }
        let dim = dimensions[0];
        let rest = &dimensions[1..];

        self = self.group_by(dim);

        if !rest.is_empty() {
            self.results = self.results.into_iter().map(|item| {
                match item {
                    ResultItem::Group(mut g) => {
                        *g = g.with_grouping_inner(rest);
                        ResultItem::Group(g)
                    }
                    other => other,
                }
            }).collect();
        }

        self
    }

    /// Create an empty report (used for sub-groups).
    fn empty() -> Self {
        Report {
            success: None,
            totals: None,
            expected: None,
            query: None,
            schema: None,
            outputs: vec![],
            results: vec![],
            group: None,
            file: None,
            command: None,
            rule_id: None,
        }
    }

    /// Move file-bound outputs into their matching file-group, recursively.
    ///
    /// For every `ReportOutput` on `self.outputs` whose `file` matches a
    /// descendant file-group, the output is detached from the parent's list
    /// and appended to the group's `outputs`, with its `file` field cleared
    /// (the group already hoists it). Outputs with `file: None` or with no
    /// matching group remain where they are.
    ///
    /// Called from `with_grouping` so callers don't need to invoke it
    /// separately.
    pub fn distribute_outputs(&mut self) {
        if self.outputs.is_empty() && !self.has_any_outputs_in_groups() {
            return;
        }

        // First, recurse so descendants redistribute their own outputs.
        for item in &mut self.results {
            if let ResultItem::Group(ref mut g) = item {
                g.distribute_outputs();
            }
        }

        // Then move our file-bound outputs down into matching file-groups.
        let own = std::mem::take(&mut self.outputs);
        let mut file_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for output in &own {
            if let Some(ref file) = output.file {
                *file_counts.entry(normalize_path(file)).or_insert(0) += 1;
            }
        }
        let mut remaining: Vec<ReportOutput> = Vec::new();
        for output in own {
            let Some(ref file) = output.file else {
                remaining.push(output);
                continue;
            };
            let normalized = normalize_path(file);
            if file_counts.get(&normalized).copied().unwrap_or(0) > 1 {
                remaining.push(output);
                continue;
            }
            let mut placed = false;
            for item in &mut self.results {
                if let ResultItem::Group(ref mut g) = item {
                    if g.file.as_deref().map(normalize_path).as_deref() == Some(&normalized) {
                        let mut moved = output.clone();
                        moved.file = None;
                        g.outputs.push(moved);
                        placed = true;
                        break;
                    }
                }
            }
            if !placed {
                remaining.push(output);
            }
        }
        self.outputs = remaining;
    }

    fn has_any_outputs_in_groups(&self) -> bool {
        self.results.iter().any(|item| match item {
            ResultItem::Group(g) => !g.outputs.is_empty() || g.has_any_outputs_in_groups(),
            _ => false,
        })
    }
}

// ---------------------------------------------------------------------------
// ReportBuilder — collector pattern for accumulating matches
// ---------------------------------------------------------------------------

/// How the builder determines the `success` field on `build()`.
enum SuccessMode {
    /// Derive from match severities: false if any Fatal/Error, true otherwise.
    Derive,
    /// No verdict (query mode): success = None.
    NoVerdict,
}

/// Accumulates `ReportMatch` entries and builds a `Report` with derived totals.
///
/// Executors push matches into the builder. The mode function creates the
/// builder, passes it to the executor, and calls `build()` to finalize.
pub struct ReportBuilder {
    matches: Vec<ReportMatch>,
    outputs: Vec<ReportOutput>,
    failed: bool,
    success_mode: SuccessMode,
    expected: Option<String>,
    query: Option<NormalizedXpath>,
}

impl ReportBuilder {
    pub fn new() -> Self {
        ReportBuilder {
            matches: Vec::new(),
            outputs: Vec::new(),
            failed: false,
            success_mode: SuccessMode::Derive,
            expected: None,
            query: None,
        }
    }

    /// Add a single match to the report.
    pub fn add(&mut self, rm: ReportMatch) {
        self.matches.push(rm);
    }

    /// Add multiple matches to the report.
    pub fn add_all(&mut self, rms: impl IntoIterator<Item = ReportMatch>) {
        self.matches.extend(rms);
    }

    /// Add a captured output payload to the report.
    pub fn add_output(&mut self, output: ReportOutput) {
        self.outputs.push(output);
    }

    /// Add multiple captured output payloads to the report.
    pub fn add_outputs(&mut self, outputs: impl IntoIterator<Item = ReportOutput>) {
        self.outputs.extend(outputs);
    }

    /// Signal that the operation failed (e.g. test expectations unmet).
    /// This forces `success` to `Some(false)` regardless of match severities.
    pub fn fail(&mut self) {
        self.failed = true;
    }

    /// Set query mode: no pass/fail verdict (success = None).
    pub fn set_no_verdict(&mut self) {
        self.success_mode = SuccessMode::NoVerdict;
    }

    /// Set the expected value string (test mode).
    pub fn set_expected(&mut self, expected: String) {
        self.expected = Some(expected);
    }

    /// Set the XPath query (shown with `-v query`).
    pub fn set_query(&mut self, query: NormalizedXpath) {
        self.query = Some(query);
    }

    /// Check if any fatal-severity matches have been added.
    pub fn has_fatals(&self) -> bool {
        self.matches.iter().any(|m| m.severity == Some(Severity::Fatal))
    }

    /// Check if any matches with status="updated" have been added.
    pub fn has_updates(&self) -> bool {
        self.matches.iter().any(|m| m.status.as_deref() == Some("updated"))
    }

    /// Consume the builder and produce a finalized Report.
    ///
    /// Totals are derived from the accumulated matches:
    /// - results, files from match count and unique file paths
    /// - fatals/errors/warnings/infos from severity field
    /// - updated/unchanged from status field
    ///
    /// Success is determined by SuccessMode:
    /// - Derive: false if any Fatal/Error matches or `fail()` was called
    /// - NoVerdict: None (query mode)
    pub fn build(self) -> Report {
        let mut file_set = std::collections::HashSet::new();
        let mut fatals = 0usize;
        let mut errors = 0usize;
        let mut warnings = 0usize;
        let mut infos = 0usize;
        let mut updated = 0usize;
        let mut unchanged = 0usize;

        for m in &self.matches {
            if !m.file.is_empty() {
                file_set.insert(m.file.as_str());
            }
            match m.severity {
                Some(Severity::Fatal) => fatals += 1,
                Some(Severity::Error) => errors += 1,
                Some(Severity::Warning) => warnings += 1,
                Some(Severity::Info) => infos += 1,
                None => {}
            }
            match m.status.as_deref() {
                Some("updated") => updated += 1,
                Some("unchanged") => unchanged += 1,
                _ => {}
            }
        }

        let totals = Totals {
            results: self.matches.len(),
            files: file_set.len(),
            fatals,
            errors,
            warnings,
            infos,
            updated,
            unchanged,
        };

        let success = match self.success_mode {
            SuccessMode::NoVerdict => {
                // No verdict on match results, but fatals are infrastructure
                // errors (broken XPath, bad config) — always fail.
                if fatals > 0 { Some(false) } else { None }
            }
            SuccessMode::Derive => {
                let has_failures = fatals > 0 || errors > 0 || self.failed;
                Some(!has_failures)
            }
        };

        let results = self.matches.into_iter().map(ResultItem::Match).collect();
        Report {
            success,
            totals: Some(totals),
            expected: self.expected,
            query: self.query,
            schema: None,
            outputs: self.outputs,
            results,
            group: None,
            file: None,
            command: None,
            rule_id: None,
        }
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
            origin: None,
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
            origin: None,
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
            origin: None,
            rule_id: None,
            status: None,
            output: None,
        };
        let mut builder = ReportBuilder::new();
        builder.add(m1);
        builder.add(m2);
        let report = builder.build();
        let json = report.to_json();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();

        // Summary fields are nested under /summary so `-p summary` can project them.
        assert_eq!(v["summary"]["success"], false);
        assert_eq!(v["summary"]["totals"]["results"], 2);
        assert_eq!(v["summary"]["totals"]["files"], 2);
        assert_eq!(v["summary"]["totals"]["errors"], 1);
        assert_eq!(v["summary"]["totals"]["warnings"], 1);

        // Matches (via all_matches helper since results is the sole storage)
        let matches = report.all_matches();
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].severity.unwrap().as_str(), "error");
        assert_eq!(matches[1].severity.unwrap().as_str(), "warning");
    }

    #[test]
    fn test_test_report_json() {
        let m = make_report_match("test.cs", 1, 1, "x");
        let mut builder = ReportBuilder::new();
        builder.add(m);
        builder.set_expected("some".to_string());
        let report = builder.build();
        let json = report.to_json();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(v["summary"]["success"], true);
        assert_eq!(v["summary"]["expected"], "some");
        // No reason/severity on plain match
        let matches = report.all_matches();
        assert_eq!(matches.len(), 1);
        assert!(matches[0].reason.is_none());
        assert!(matches[0].severity.is_none());
    }

    #[test]
    fn duplicate_file_outputs_stay_at_report_root() {
        let mut builder = ReportBuilder::new();
        builder.set_no_verdict();
        builder.add(make_report_match("config.yaml", 1, 1, "x"));
        builder.add_outputs([
            ReportOutput {
                file: Some("config.yaml".to_string()),
                content: "host: a\n".to_string(),
            },
            ReportOutput {
                file: Some("config.yaml".to_string()),
                content: "host: b\n".to_string(),
            },
        ]);

        let report = builder.build().with_grouping(&["file"]);

        assert_eq!(report.outputs.len(), 2);
        assert_eq!(report.outputs[0].file.as_deref(), Some("config.yaml"));
        assert_eq!(report.outputs[1].file.as_deref(), Some("config.yaml"));

        let ResultItem::Group(group) = &report.results[0] else {
            panic!("expected grouped file report");
        };
        assert!(group.outputs.is_empty());
    }
}
