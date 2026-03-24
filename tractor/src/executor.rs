//! Batch executor for tractor operations.
//!
//! The executor is the core engine of tractor. It takes a list of operations
//! and returns a `Report` for each one. Operations can come from:
//!
//! - A config file (`tractor run config.yaml`)
//! - CLI commands (`tractor check`, `tractor query`, etc.)
//! - Programmatic construction
//!
//! Each operation is self-contained: it declares the files, xpath/rules, and
//! options it needs. The executor handles file resolution, parsing, querying,
//! and produces a unified `Report` that can be rendered in any format.

use std::path::PathBuf;
use std::collections::HashSet;
use rayon::prelude::*;
use tractor_core::rule::{Rule, RuleSet};
use tractor_core::report::{Report, ReportMatch, Severity, Summary};
use tractor_core::tree_mode::TreeMode;
use tractor_core::{expand_globs, filter_supported_files, detect_language, parse_to_documents, Match, apply_replacements};
use tractor_core::xpath_upsert::{upsert, update_only};

use crate::pipeline::run_rules;

// ---------------------------------------------------------------------------
// Operation types (stable API)
// ---------------------------------------------------------------------------

/// A single operation to execute. This is the stable intermediate
/// representation — config files parse into this, CLI commands construct
/// it, and the executor consumes it.
#[derive(Debug, Clone)]
pub enum Operation {
    Query(QueryOperation),
    Check(CheckOperation),
    Test(TestOperation),
    Set(SetOperation),
    Update(UpdateOperation),
}

/// A query operation: run an XPath expression against files, return matches.
#[derive(Debug, Clone)]
pub struct QueryOperation {
    /// File glob patterns to include.
    pub files: Vec<String>,
    /// File glob patterns to exclude.
    pub exclude: Vec<String>,
    /// XPath expression to evaluate.
    pub xpath: String,
    /// Tree mode override for parsing.
    pub tree_mode: Option<TreeMode>,
    /// Language override for parsing.
    pub language: Option<String>,
    /// Maximum number of matches to return.
    pub limit: Option<usize>,
    /// Ignore whitespace-only text nodes during parsing.
    pub ignore_whitespace: bool,
    /// Maximum parse depth.
    pub parse_depth: Option<usize>,
}

/// A check operation: run XPath rules against files, report violations.
#[derive(Debug, Clone)]
pub struct CheckOperation {
    /// File glob patterns to include.
    pub files: Vec<String>,
    /// File glob patterns to exclude.
    pub exclude: Vec<String>,
    /// Rules to check.
    pub rules: Vec<Rule>,
    /// Default tree mode for all rules (rules can override).
    pub tree_mode: Option<TreeMode>,
    /// Default language for all rules (rules can override).
    pub language: Option<String>,
    /// Ignore whitespace-only text nodes during parsing.
    pub ignore_whitespace: bool,
    /// Maximum parse depth.
    pub parse_depth: Option<usize>,
    /// Ruleset-level include patterns for per-rule glob matching.
    /// Used by rules-file configs; empty for single-xpath checks.
    #[doc(hidden)]
    pub ruleset_include: Vec<String>,
    /// Ruleset-level exclude patterns for per-rule glob matching.
    #[doc(hidden)]
    pub ruleset_exclude: Vec<String>,
}

/// A test operation: run an XPath query and check match count against an expectation.
#[derive(Debug, Clone)]
pub struct TestOperation {
    /// File glob patterns to include.
    pub files: Vec<String>,
    /// File glob patterns to exclude.
    pub exclude: Vec<String>,
    /// XPath expression to evaluate.
    pub xpath: String,
    /// Expected match count: "none", "some", or a number.
    pub expect: String,
    /// Tree mode override for parsing.
    pub tree_mode: Option<TreeMode>,
    /// Language override for parsing.
    pub language: Option<String>,
    /// Maximum number of matches to return.
    pub limit: Option<usize>,
    /// Ignore whitespace-only text nodes during parsing.
    pub ignore_whitespace: bool,
    /// Maximum parse depth.
    pub parse_depth: Option<usize>,
}

/// An update operation: modify existing matched nodes without creating new structure.
/// Unlike set, update fails if the XPath does not match any existing nodes.
#[derive(Debug, Clone)]
pub struct UpdateOperation {
    /// File glob patterns to include.
    pub files: Vec<String>,
    /// File glob patterns to exclude.
    pub exclude: Vec<String>,
    /// XPath expression to match nodes to update.
    pub xpath: String,
    /// New value for matched nodes.
    pub value: String,
    /// Tree mode override for parsing.
    pub tree_mode: Option<TreeMode>,
    /// Language override for parsing.
    pub language: Option<String>,
    /// Maximum number of matches to update per file.
    pub limit: Option<usize>,
    /// Ignore whitespace-only text nodes during parsing.
    pub ignore_whitespace: bool,
    /// Maximum parse depth.
    pub parse_depth: Option<usize>,
}

/// A set operation: ensure values exist at specified XPaths.
#[derive(Debug, Clone)]
pub struct SetOperation {
    /// File glob patterns to include.
    pub files: Vec<String>,
    /// File glob patterns to exclude.
    pub exclude: Vec<String>,
    /// Mappings to apply.
    pub mappings: Vec<SetMapping>,
    /// Tree mode override for parsing.
    pub tree_mode: Option<TreeMode>,
    /// Language override for parsing.
    pub language: Option<String>,
    /// If true, check for drift without writing files.
    /// The report's summary.passed will be false if any files would change.
    pub verify: bool,
}

/// A single xpath → value mapping for set operations.
#[derive(Debug, Clone)]
pub struct SetMapping {
    pub xpath: String,
    pub value: String,
}

// ---------------------------------------------------------------------------
// Execution options
// ---------------------------------------------------------------------------

/// Options controlling how operations are executed.
#[derive(Debug, Clone)]
pub struct ExecuteOptions {
    /// Print verbose diagnostics to stderr.
    pub verbose: bool,
    /// Base directory for resolving relative file paths.
    /// If None, uses the current working directory.
    pub base_dir: Option<PathBuf>,
}

impl Default for ExecuteOptions {
    fn default() -> Self {
        ExecuteOptions {
            verbose: false,
            base_dir: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Executor
// ---------------------------------------------------------------------------

/// Execute a list of operations and return a `Report` for each one.
pub fn execute(
    operations: &[Operation],
    options: &ExecuteOptions,
) -> Result<Vec<Report>, Box<dyn std::error::Error>> {
    let mut reports = Vec::with_capacity(operations.len());

    for op in operations {
        reports.push(match op {
            Operation::Query(q) => execute_query(q, options)?,
            Operation::Check(c) => execute_check(c, options)?,
            Operation::Test(t) => execute_test(t, options)?,
            Operation::Set(s) => execute_set(s, options)?,
            Operation::Update(u) => execute_update(u, options)?,
        });
    }

    Ok(reports)
}

// ---------------------------------------------------------------------------
// Query execution
// ---------------------------------------------------------------------------

fn execute_query(
    op: &QueryOperation,
    options: &ExecuteOptions,
) -> Result<Report, Box<dyn std::error::Error>> {
    let files = resolve_files(&op.files, &op.exclude, options);

    if files.is_empty() {
        return Ok(Report::query(vec![], empty_summary()));
    }

    let matches = query_files(
        &files, &op.xpath, op.language.as_deref(),
        op.tree_mode, op.ignore_whitespace, op.parse_depth,
        op.limit, options.verbose,
    )?;

    let total = matches.len();
    let files_affected = count_unique_files(&matches);
    let report_matches = matches.into_iter()
        .map(match_to_full_report_match)
        .collect();

    Ok(Report::query(report_matches, Summary {
        passed: true,
        total,
        files_affected,
        errors: 0,
        warnings: 0,
        expected: None,
        query: None,
    }))
}

// ---------------------------------------------------------------------------
// Check execution
// ---------------------------------------------------------------------------

fn execute_check(
    op: &CheckOperation,
    options: &ExecuteOptions,
) -> Result<Report, Box<dyn std::error::Error>> {
    if op.rules.is_empty() {
        return Ok(Report::check(vec![], empty_summary()));
    }

    let files = resolve_files(&op.files, &op.exclude, options);

    if files.is_empty() {
        return Ok(Report::check(vec![], empty_summary()));
    }

    // Build a RuleSet from the operation. Ruleset-level include/exclude
    // come from rules files; per-rule patterns still participate in glob matching.
    let ruleset = RuleSet {
        rules: op.rules.clone(),
        include: op.ruleset_include.clone(),
        exclude: op.ruleset_exclude.clone(),
        default_tree_mode: op.tree_mode,
        default_language: op.language.clone(),
    };

    let rule_matches = run_rules(
        &ruleset,
        &files,
        op.tree_mode,
        op.ignore_whitespace,
        op.parse_depth,
        options.verbose,
    )?;

    let mut files_affected = HashSet::new();
    let mut errors = 0usize;
    let mut warnings = 0usize;

    let report_matches: Vec<ReportMatch> = rule_matches
        .into_iter()
        .map(|rm| {
            let rule = &ruleset.rules[rm.rule_index];
            let reason = rule
                .reason
                .clone()
                .unwrap_or_else(|| format!("[{}] check failed", rule.id));
            let severity = rule.severity;

            match severity {
                Severity::Error => errors += 1,
                Severity::Warning => warnings += 1,
            }
            files_affected.insert(rm.m.file.clone());

            // Apply rule-level message template (if the rule defines one)
            let message = rule
                .message
                .as_deref()
                .map(|t| tractor_core::format_message(t, &rm.m));

            let mut report_match = match_to_full_report_match(rm.m);
            report_match.reason = Some(reason);
            report_match.severity = Some(severity);
            report_match.rule_id = Some(rule.id.clone());
            report_match.message = message;
            report_match
        })
        .collect();

    let total = report_matches.len();
    Ok(Report::check(report_matches, Summary {
        passed: errors == 0,
        total,
        files_affected: files_affected.len(),
        errors,
        warnings,
        expected: None,
        query: None,
    }))
}

// ---------------------------------------------------------------------------
// Set execution
// ---------------------------------------------------------------------------

fn execute_set(
    op: &SetOperation,
    options: &ExecuteOptions,
) -> Result<Report, Box<dyn std::error::Error>> {
    let files = resolve_files(&op.files, &op.exclude, options);
    let mut report_matches = Vec::new();
    let mut files_affected = HashSet::new();
    let mut updated_count = 0usize;
    let mut unchanged_count = 0usize;

    for file_path in &files {
        let lang_override = op.language.as_deref();
        let lang = lang_override
            .unwrap_or_else(|| detect_language(file_path));

        let source = std::fs::read_to_string(file_path)?;
        let mut current = source.clone();
        let mut mappings_applied = 0;

        for mapping in &op.mappings {
            let result = upsert(&current, lang, &mapping.xpath, &mapping.value, None)?;
            if result.source != current {
                mappings_applied += 1;
                current = result.source;
            }
        }

        let was_modified = current != source;

        // Write the file unless we're in verify mode
        if was_modified && !op.verify {
            std::fs::write(file_path, &current)?;
        }

        let status_str = if was_modified { "updated" } else { "unchanged" };
        if was_modified {
            updated_count += 1;
            files_affected.insert(file_path.clone());
        } else {
            unchanged_count += 1;
        }

        report_matches.push(ReportMatch {
            file: file_path.clone(),
            line: 1,
            column: 1,
            end_line: 1,
            end_column: 1,
            tree: None,
            value: None,
            source: None,
            lines: None,
            reason: None,
            severity: None,
            message: None,
            rule_id: None,
            status: Some(status_str.to_string()),
            output: if was_modified && op.verify {
                Some(format!("{} mapping{} would change", mappings_applied, if mappings_applied == 1 { "" } else { "s" }))
            } else {
                None
            },
        });
    }

    let total = report_matches.len();
    let passed = if op.verify { updated_count == 0 } else { true };

    Ok(Report::set(report_matches, Summary {
        passed,
        total,
        files_affected: files_affected.len(),
        errors: updated_count,
        warnings: unchanged_count,
        expected: None,
        query: None,
    }))
}

// ---------------------------------------------------------------------------
// Test execution
// ---------------------------------------------------------------------------

fn execute_test(
    op: &TestOperation,
    options: &ExecuteOptions,
) -> Result<Report, Box<dyn std::error::Error>> {
    let files = resolve_files(&op.files, &op.exclude, options);

    if files.is_empty() {
        let passed = check_expectation(&op.expect, 0)?;
        return Ok(Report::test(vec![], Summary {
            passed,
            total: 0,
            files_affected: 0,
            errors: 0,
            warnings: 0,
            expected: Some(op.expect.clone()),
            query: None,
        }));
    }

    let matches = query_files(
        &files, &op.xpath, op.language.as_deref(),
        op.tree_mode, op.ignore_whitespace, op.parse_depth,
        op.limit, options.verbose,
    )?;

    let total = matches.len();
    let files_affected = count_unique_files(&matches);
    let passed = check_expectation(&op.expect, total)?;

    let report_matches = matches.into_iter()
        .map(match_to_full_report_match)
        .collect();

    Ok(Report::test(report_matches, Summary {
        passed,
        total,
        files_affected,
        errors: 0,
        warnings: 0,
        expected: Some(op.expect.clone()),
        query: None,
    }))
}

// ---------------------------------------------------------------------------
// Update execution
// ---------------------------------------------------------------------------

fn execute_update(
    op: &UpdateOperation,
    options: &ExecuteOptions,
) -> Result<Report, Box<dyn std::error::Error>> {
    let files = resolve_files(&op.files, &op.exclude, options);
    let mut total_updated = 0usize;
    let mut files_modified = HashSet::new();
    let mut fallback_files = Vec::new();

    for file_path in &files {
        let lang = op.language.as_deref()
            .unwrap_or_else(|| detect_language(file_path));
        let source = std::fs::read_to_string(file_path)?;

        match update_only(&source, lang, &op.xpath, &op.value, op.limit) {
            Ok(result) => {
                if result.source != source {
                    std::fs::write(file_path, &result.source)?;
                    total_updated += result.matches_updated;
                    files_modified.insert(file_path.clone());
                }
            }
            Err(tractor_core::xpath_upsert::UpsertError::UnsupportedLanguage(_)) => {
                fallback_files.push(file_path.clone());
            }
            Err(e) => return Err(e.into()),
        }
    }

    // Legacy fallback for languages without renderers
    if !fallback_files.is_empty() {
        let matches = query_files(
            &fallback_files, &op.xpath, op.language.as_deref(),
            op.tree_mode, op.ignore_whitespace, op.parse_depth,
            None, options.verbose,
        )?;
        if !matches.is_empty() {
            let summary = apply_replacements(&matches, &op.value)?;
            total_updated += summary.replacements_made;
            for m in &matches {
                files_modified.insert(m.file.clone());
            }
        }
    }

    Ok(Report::set(vec![], Summary {
        passed: total_updated > 0,
        total: total_updated,
        files_affected: files_modified.len(),
        errors: 0,
        warnings: 0,
        expected: None,
        query: None,
    }))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Check whether an expectation is met.
fn check_expectation(expect: &str, count: usize) -> Result<bool, Box<dyn std::error::Error>> {
    let passed = match expect {
        "none" => count == 0,
        "some" => count > 0,
        _ => {
            let expected: usize = expect.parse()
                .map_err(|_| format!("invalid expectation '{}': use 'none', 'some', or a number", expect))?;
            count == expected
        }
    };
    Ok(passed)
}

fn empty_summary() -> Summary {
    Summary {
        passed: true,
        total: 0,
        files_affected: 0,
        errors: 0,
        warnings: 0,
        expected: None,
        query: None,
    }
}

fn count_unique_files(matches: &[Match]) -> usize {
    let mut seen = HashSet::new();
    for m in matches {
        seen.insert(&m.file);
    }
    seen.len()
}

/// Convert a raw `Match` into a `ReportMatch` with all content fields populated.
/// Operation-specific fields (reason, severity, rule_id, status, message) are
/// left as None and must be set by the caller.
fn match_to_full_report_match(m: Match) -> ReportMatch {
    ReportMatch {
        file: m.file.clone(),
        line: m.line,
        column: m.column,
        end_line: m.end_line,
        end_column: m.end_column,
        tree: m.xml_node.clone(),
        value: Some(m.value.clone()),
        source: Some(m.extract_source_snippet()),
        lines: Some(
            m.get_source_lines_range()
                .into_iter()
                .map(|l| l.trim_end_matches('\r').to_owned())
                .collect(),
        ),
        reason: None,
        severity: None,
        message: None,
        rule_id: None,
        status: None,
        output: None,
    }
}

/// Parse and query files in parallel, returning sorted matches.
fn query_files(
    files: &[String],
    xpath_expr: &str,
    lang: Option<&str>,
    tree_mode: Option<TreeMode>,
    ignore_whitespace: bool,
    parse_depth: Option<usize>,
    limit: Option<usize>,
    verbose: bool,
) -> Result<Vec<Match>, Box<dyn std::error::Error>> {
    let mut all_matches: Vec<Match> = files
        .par_iter()
        .filter_map(|file_path| {
            let mut result = match parse_to_documents(
                std::path::Path::new(file_path),
                lang,
                tree_mode,
                ignore_whitespace,
                parse_depth,
            ) {
                Ok(r) => r,
                Err(e) => {
                    if verbose {
                        eprintln!("warning: {}: {}", file_path, e);
                    }
                    return None;
                }
            };

            match result.query(xpath_expr) {
                Ok(matches) => Some(matches),
                Err(e) => {
                    if verbose {
                        eprintln!("warning: {}: query error: {}", file_path, e);
                    }
                    None
                }
            }
        })
        .flatten()
        .collect();

    all_matches.sort_by(|a, b| (&a.file, a.line, a.column).cmp(&(&b.file, b.line, b.column)));

    if let Some(limit) = limit {
        all_matches.truncate(limit);
    }

    Ok(all_matches)
}

fn resolve_files(
    file_globs: &[String],
    exclude_globs: &[String],
    options: &ExecuteOptions,
) -> Vec<String> {
    let mut files = if let Some(base) = &options.base_dir {
        // Resolve globs relative to base_dir
        let globs: Vec<String> = file_globs.iter().map(|g| {
            if std::path::Path::new(g).is_absolute() {
                g.clone()
            } else {
                base.join(g).to_string_lossy().to_string()
            }
        }).collect();
        expand_globs(&globs)
    } else {
        expand_globs(file_globs)
    };

    // Filter excludes
    if !exclude_globs.is_empty() {
        let exclude_patterns: Vec<glob::Pattern> = exclude_globs.iter()
            .filter_map(|p| glob::Pattern::new(p).ok())
            .collect();

        let opts = glob::MatchOptions {
            case_sensitive: true,
            require_literal_separator: false,
            require_literal_leading_dot: false,
        };

        files.retain(|f| {
            !exclude_patterns.iter().any(|p| p.matches_with(f, opts))
        });
    }

    filter_supported_files(files)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;


    fn temp_json_file(content: &str) -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.json");
        std::fs::write(&path, content).unwrap();
        (dir, path.to_str().unwrap().to_string())
    }

    fn temp_yaml_file(content: &str) -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.yaml");
        std::fs::write(&path, content).unwrap();
        (dir, path.to_str().unwrap().to_string())
    }

    // -----------------------------------------------------------------------
    // Query operation tests
    // -----------------------------------------------------------------------

    #[test]
    fn query_returns_matches() {
        let (_dir, path) = temp_json_file(r#"{"name": "alice", "age": 30}"#);

        let ops = vec![Operation::Query(QueryOperation {
            files: vec![path.clone()],
            exclude: vec![],
            xpath: "//name".into(),
            tree_mode: None,
            language: None,
            limit: None,
            ignore_whitespace: false,
            parse_depth: None,
        })];

        let reports = execute(&ops, &ExecuteOptions::default()).unwrap();
        assert_eq!(reports.len(), 1);
        let report = &reports[0];
        assert!(report.summary.as_ref().unwrap().passed);
        assert_eq!(report.matches.len(), 1);
        assert_eq!(report.matches[0].value.as_deref(), Some("alice"));
    }

    #[test]
    fn query_with_limit() {
        let (_dir, path) = temp_json_file(r#"{"a": 1, "b": 2, "c": 3}"#);

        let ops = vec![Operation::Query(QueryOperation {
            files: vec![path.clone()],
            exclude: vec![],
            xpath: "//*[number(.) > 0]".into(),
            tree_mode: None,
            language: None,
            limit: Some(2),
            ignore_whitespace: false,
            parse_depth: None,
        })];

        let reports = execute(&ops, &ExecuteOptions::default()).unwrap();
        assert!(reports[0].matches.len() <= 2);
    }

    #[test]
    fn query_empty_files() {
        let ops = vec![Operation::Query(QueryOperation {
            files: vec![],
            exclude: vec![],
            xpath: "//x".into(),
            tree_mode: None,
            language: None,
            limit: None,
            ignore_whitespace: false,
            parse_depth: None,
        })];

        let reports = execute(&ops, &ExecuteOptions::default()).unwrap();
        assert_eq!(reports[0].matches.len(), 0);
        assert!(reports[0].summary.as_ref().unwrap().passed);
    }

    // -----------------------------------------------------------------------
    // Set operation tests
    // -----------------------------------------------------------------------

    #[test]
    fn set_updates_json_value() {
        let (_dir, path) = temp_json_file(r#"{"database": {"host": "old"}}"#);

        let ops = vec![Operation::Set(SetOperation {
            files: vec![path.clone()],
            exclude: vec![],
            mappings: vec![SetMapping {
                xpath: "//database/host".into(),
                value: "new-host".into(),
            }],
            tree_mode: None,
            language: None,
            verify: false,
        })];

        let reports = execute(&ops, &ExecuteOptions::default()).unwrap();
        assert!(reports[0].summary.as_ref().unwrap().passed);

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("new-host"), "file should contain new value: {}", content);
        assert!(!content.contains("old"), "file should not contain old value: {}", content);
    }

    #[test]
    fn set_creates_missing_node() {
        let (_dir, path) = temp_json_file(r#"{"database": {}}"#);

        let ops = vec![Operation::Set(SetOperation {
            files: vec![path.clone()],
            exclude: vec![],
            mappings: vec![SetMapping {
                xpath: "//database/host".into(),
                value: "localhost".into(),
            }],
            tree_mode: None,
            language: None,
            verify: false,
        })];

        let reports = execute(&ops, &ExecuteOptions::default()).unwrap();
        assert!(reports[0].summary.as_ref().unwrap().passed);

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("localhost"), "missing node should be created: {}", content);
    }

    #[test]
    fn set_multiple_mappings() {
        let (_dir, path) = temp_json_file(r#"{"database": {"host": "old", "port": 1234}}"#);

        let ops = vec![Operation::Set(SetOperation {
            files: vec![path.clone()],
            exclude: vec![],
            mappings: vec![
                SetMapping { xpath: "//database/host".into(), value: "new-host".into() },
                SetMapping { xpath: "//database/port".into(), value: "5432".into() },
            ],
            tree_mode: None,
            language: None,
            verify: false,
        })];

        let reports = execute(&ops, &ExecuteOptions::default()).unwrap();
        assert!(reports[0].summary.as_ref().unwrap().passed);

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("new-host"), "host should be updated: {}", content);
        assert!(content.contains("5432"), "port should be updated: {}", content);
    }

    #[test]
    fn set_no_change_when_value_matches() {
        let original = r#"{
  "database": {
    "host": "localhost"
  }
}"#;
        let (_dir, path) = temp_json_file(original);

        let ops = vec![Operation::Set(SetOperation {
            files: vec![path.clone()],
            exclude: vec![],
            mappings: vec![SetMapping {
                xpath: "//database/host".into(),
                value: "localhost".into(),
            }],
            tree_mode: None,
            language: None,
            verify: false,
        })];

        let reports = execute(&ops, &ExecuteOptions::default()).unwrap();
        assert!(reports[0].summary.as_ref().unwrap().passed);

        // Check status is "unchanged"
        assert_eq!(reports[0].matches[0].status.as_deref(), Some("unchanged"));
    }

    // -----------------------------------------------------------------------
    // Verify mode tests
    // -----------------------------------------------------------------------

    #[test]
    fn verify_detects_drift() {
        let (_dir, path) = temp_json_file(r#"{"database": {"host": "wrong"}}"#);

        let ops = vec![Operation::Set(SetOperation {
            files: vec![path.clone()],
            exclude: vec![],
            mappings: vec![SetMapping {
                xpath: "//database/host".into(),
                value: "correct".into(),
            }],
            tree_mode: None,
            language: None,
            verify: true,
        })];

        let reports = execute(&ops, &ExecuteOptions::default()).unwrap();

        // Should fail: drift detected
        assert!(!reports[0].summary.as_ref().unwrap().passed, "verify should detect drift");

        // File should NOT be modified
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("wrong"), "file should not be modified in verify mode");
    }

    #[test]
    fn verify_passes_when_in_sync() {
        let (_dir, path) = temp_json_file(r#"{
  "database": {
    "host": "correct"
  }
}"#);

        let ops = vec![Operation::Set(SetOperation {
            files: vec![path.clone()],
            exclude: vec![],
            mappings: vec![SetMapping {
                xpath: "//database/host".into(),
                value: "correct".into(),
            }],
            tree_mode: None,
            language: None,
            verify: true,
        })];

        let reports = execute(&ops, &ExecuteOptions::default()).unwrap();
        assert!(reports[0].summary.as_ref().unwrap().passed, "verify should pass when values are in sync");
    }

    // -----------------------------------------------------------------------
    // Check operation tests
    // -----------------------------------------------------------------------

    #[test]
    fn check_finds_violations() {
        let (_dir, path) = temp_json_file(r#"{"debug": true, "verbose": true}"#);

        let ops = vec![Operation::Check(CheckOperation {
            files: vec![path.clone()],
            exclude: vec![],
            rules: vec![
                Rule::new("no-debug", "//debug[.='true']")
                    .with_reason("debug should not be enabled")
                    .with_severity(Severity::Error),
            ],
            tree_mode: None,
            language: None,
            ignore_whitespace: false,
            parse_depth: None,
            ruleset_include: vec![],
            ruleset_exclude: vec![],
        })];

        let reports = execute(&ops, &ExecuteOptions::default()).unwrap();
        let report = &reports[0];
        let summary = report.summary.as_ref().unwrap();
        assert!(!summary.passed, "check should fail when violations found");
        assert_eq!(report.matches.len(), 1);
        assert_eq!(report.matches[0].rule_id.as_deref(), Some("no-debug"));
        assert_eq!(report.matches[0].reason.as_deref(), Some("debug should not be enabled"));
    }

    #[test]
    fn check_passes_when_no_violations() {
        let (_dir, path) = temp_json_file(r#"{"debug": false}"#);

        let ops = vec![Operation::Check(CheckOperation {
            files: vec![path.clone()],
            exclude: vec![],
            rules: vec![
                Rule::new("no-debug", "//debug[.='true']")
                    .with_reason("debug should not be enabled"),
            ],
            tree_mode: None,
            language: None,
            ignore_whitespace: false,
            parse_depth: None,
            ruleset_include: vec![],
            ruleset_exclude: vec![],
        })];

        let reports = execute(&ops, &ExecuteOptions::default()).unwrap();
        assert!(reports[0].summary.as_ref().unwrap().passed);
    }

    // -----------------------------------------------------------------------
    // Mixed operations tests
    // -----------------------------------------------------------------------

    #[test]
    fn mixed_check_and_set() {
        let dir = tempfile::tempdir().unwrap();

        // A config file to set values on
        let config_path = dir.path().join("config.json");
        std::fs::write(&config_path, r#"{"host": "old"}"#).unwrap();

        // A data file to check
        let data_path = dir.path().join("data.json");
        std::fs::write(&data_path, r#"{"name": "test"}"#).unwrap();

        let ops = vec![
            Operation::Check(CheckOperation {
                files: vec![data_path.to_str().unwrap().into()],
                exclude: vec![],
                rules: vec![
                    Rule::new("has-name", "//name[.='missing']")
                        .with_reason("name should not be 'missing'"),
                ],
                tree_mode: None,
                language: None,
                ignore_whitespace: false,
                parse_depth: None,
                ruleset_include: vec![],
                ruleset_exclude: vec![],
            }),
            Operation::Set(SetOperation {
                files: vec![config_path.to_str().unwrap().into()],
                exclude: vec![],
                mappings: vec![SetMapping {
                    xpath: "//host".into(),
                    value: "new-host".into(),
                }],
                tree_mode: None,
                language: None,
                verify: false,
            }),
        ];

        let reports = execute(&ops, &ExecuteOptions::default()).unwrap();
        assert_eq!(reports.len(), 2);
        assert!(reports[0].summary.as_ref().unwrap().passed);
        assert!(reports[1].summary.as_ref().unwrap().passed);

        // Config should be updated
        let content = std::fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("new-host"));
    }

    #[test]
    fn set_yaml_updates_value() {
        let (_dir, path) = temp_yaml_file("database:\n  host: old\n  port: 5432\n");

        let ops = vec![Operation::Set(SetOperation {
            files: vec![path.clone()],
            exclude: vec![],
            mappings: vec![SetMapping {
                xpath: "//database/host".into(),
                value: "new-host".into(),
            }],
            tree_mode: None,
            language: None,
            verify: false,
        })];

        let reports = execute(&ops, &ExecuteOptions::default()).unwrap();
        assert!(reports[0].summary.as_ref().unwrap().passed);

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("new-host"), "yaml host should be updated: {}", content);
        assert!(content.contains("5432"), "yaml port should be preserved: {}", content);
    }
}
