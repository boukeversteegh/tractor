//! Report model types.
//!
//! A `Report` is the normalized output of any tractor command. It is the
//! boundary between stage 2 (report construction) and stage 3 (serialization).
//!
//! Pipeline:
//!   source files → [stage 1: parse + query] → Vec<Match>
//!                → [stage 2: report build]  → Report
//!                → [stage 3: output]        → stdout
//!
//! Currently only the types are defined. Modes still produce output directly
//! (stage 2→3 are merged). The Report type will be wired in when the report
//! rework is implemented.

use crate::Match;

// ---------------------------------------------------------------------------
// Severity
// ---------------------------------------------------------------------------

/// Severity level for check violations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

// ---------------------------------------------------------------------------
// Summary
// ---------------------------------------------------------------------------

/// Aggregated result summary. Present in check and test reports.
#[derive(Debug)]
pub struct Summary {
    /// Did the command succeed (no error-severity violations / expectation met)?
    pub passed: bool,

    /// Total number of matches.
    pub total: usize,

    /// Number of distinct files that had at least one match.
    pub files_affected: usize,

    /// Error-severity match count (check only).
    pub errors: usize,

    /// Warning-severity match count (check only).
    pub warnings: usize,

    /// The expected value string for test assertions (`none`, `some`, or a number).
    pub expected: Option<String>,
}

// ---------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------

/// Which command produced this report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportKind {
    Query,
    Check,
    Test,
}

/// The normalized output of a tractor command.
///
/// This is the intended stage 2→3 boundary. Currently unused at runtime —
/// modes produce output directly. Will be wired in during the report rework.
#[derive(Debug)]
pub struct Report {
    pub kind: ReportKind,
    pub matches: Vec<ReportMatch>,

    /// Present for check and test reports; absent for query.
    pub summary: Option<Summary>,
}

impl Report {
    pub fn query(matches: Vec<ReportMatch>) -> Self {
        Report { kind: ReportKind::Query, matches, summary: None }
    }

    pub fn check(matches: Vec<ReportMatch>, summary: Summary) -> Self {
        Report { kind: ReportKind::Check, matches, summary: Some(summary) }
    }

    pub fn test(matches: Vec<ReportMatch>, summary: Summary) -> Self {
        Report { kind: ReportKind::Test, matches, summary: Some(summary) }
    }
}
