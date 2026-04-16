//! Batch executor for tractor operations.
//!
//! The executor is the core engine of tractor. It takes a list of operations
//! and pushes results into a `ReportBuilder`. Operations can come from:
//!
//! - A config file (`tractor run config.yaml`)
//! - CLI commands (`tractor check`, `tractor query`, etc.)
//! - Programmatic construction
//!
//! Each operation is self-contained: it declares the files, xpath/rules, and
//! options it needs. The executor handles file resolution, parsing, querying,
//! and pushes matches into a `ReportBuilder` that can be finalized into a
//! `Report` and rendered in any format.

mod query;
mod check;
mod test;
mod set;
mod update;

use std::path::PathBuf;
use rayon::prelude::*;
use tractor::report::{ReportBuilder, ReportMatch};
use tractor::tree_mode::TreeMode;
use tractor::{parse_to_documents, Match, NormalizedPath};

use crate::input::filter::ResultFilter;
use crate::input::file_resolver::{FileResolver, make_fatal_diagnostic};

pub use query::{QueryOperation, QueryExpr};
pub use check::CheckOperation;
pub use test::{TestOperation, TestAssertion};
pub use set::{SetOperation, SetMapping, SetWriteMode, SetReportMode};
pub use update::UpdateOperation;

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

// ---------------------------------------------------------------------------
// Execution options
// ---------------------------------------------------------------------------

/// Default maximum number of files tractor will process.
pub const DEFAULT_MAX_FILES: usize = 10_000;

/// Options controlling how operations are executed.
#[derive(Debug, Clone)]
pub struct ExecuteOptions {
    /// Print verbose diagnostics to stderr.
    pub verbose: bool,
    /// Base directory for resolving relative file paths.
    /// If None, uses the current working directory.
    pub base_dir: Option<PathBuf>,
    /// Git diff spec for filtering to changed files (e.g. "HEAD~3", "main..HEAD").
    /// When set, resolved files are intersected with the set of changed files.
    pub diff_files: Option<String>,
    /// Git diff spec for filtering matches to changed hunks.
    /// When set, only matches whose lines overlap with changed hunks are included.
    pub diff_lines: Option<String>,
    /// Maximum number of files to process. Glob expansion aborts at 10x this limit.
    pub max_files: usize,
    /// CLI-provided file patterns, intersected with operation file globs.
    pub cli_files: Vec<String>,
    /// Root-level file patterns from config.
    /// `None` = key missing (unrestricted); `Some(vec![])` = explicitly empty.
    pub config_root_files: Option<Vec<String>>,
}

impl Default for ExecuteOptions {
    fn default() -> Self {
        ExecuteOptions {
            verbose: false,
            base_dir: None,
            diff_files: None,
            diff_lines: None,
            max_files: DEFAULT_MAX_FILES,
            cli_files: Vec::new(),
            config_root_files: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Executor
// ---------------------------------------------------------------------------

/// Convert owned filters to borrowed references for passing to query engine.
pub(crate) fn filter_refs(filters: &[Box<dyn ResultFilter>]) -> Vec<&dyn ResultFilter> {
    filters.iter().map(|f| f.as_ref()).collect()
}

/// Execute a list of operations, pushing results into the given `ReportBuilder`.
pub fn execute(
    operations: &[Operation],
    options: &ExecuteOptions,
    report: &mut ReportBuilder,
) -> Result<(), Box<dyn std::error::Error>> {
    // Build centralized file resolver once (root globs, CLI globs, global diff).
    let resolver = match FileResolver::new(options) {
        Ok(r) => r,
        Err(msg) => {
            report.add(make_fatal_diagnostic("config", msg));
            return Ok(());
        }
    };

    for op in operations {
        match op {
            Operation::Query(q) => query::execute_query(q, options, &resolver, report)?,
            Operation::Check(c) => check::execute_check(c, options, &resolver, report)?,
            Operation::Test(t) => test::execute_test(t, options, &resolver, report)?,
            Operation::Set(s) => set::execute_set(s, options, &resolver, report)?,
            Operation::Update(u) => update::execute_update(u, options, &resolver, report)?,
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Convert a raw `Match` into a `ReportMatch` with all content fields populated.
/// Operation-specific fields (reason, severity, rule_id, status, message) are
/// left as None and must be set by the caller.
pub(crate) fn match_to_report_match(m: Match, command: &str) -> ReportMatch {
    ReportMatch {
        file: m.file.clone(),
        line: m.line,
        column: m.column,
        end_line: m.end_line,
        end_column: m.end_column,
        command: command.to_string(),
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
        origin: None,
        rule_id: None,
        status: None,
        output: None,
    }
}

/// Parse and query files in parallel with multiple XPath expressions.
/// Each file is parsed once and all expressions are evaluated against it.
pub(crate) fn query_files_multi(
    files: &[NormalizedPath],
    xpaths: &[&str],
    lang: Option<&str>,
    tree_mode: Option<TreeMode>,
    ignore_whitespace: bool,
    parse_depth: Option<usize>,
    limit: Option<usize>,
    verbose: bool,
    filters: &[&dyn ResultFilter],
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

            let mut file_matches = Vec::new();
            for xpath_expr in xpaths {
                match result.query(xpath_expr) {
                    Ok(matches) => file_matches.extend(matches),
                    Err(e) => {
                        if verbose {
                            eprintln!("warning: {}: query error: {}", file_path, e);
                        }
                    }
                }
            }

            // Apply result filters at the query engine level.
            if !filters.is_empty() {
                file_matches.retain(|m| filters.iter().all(|f| f.include(m)));
            }

            if file_matches.is_empty() { None } else { Some(file_matches) }
        })
        .flatten()
        .collect();

    all_matches.sort_by(|a, b| (&a.file, a.line, a.column).cmp(&(&b.file, b.line, b.column)));

    if let Some(limit) = limit {
        all_matches.truncate(limit);
    }

    Ok(all_matches)
}

/// Check whether an expectation is met.
pub(crate) fn check_expectation(expect: &str, count: usize) -> Result<bool, Box<dyn std::error::Error>> {
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tractor::report::{ReportBuilder, Severity};
    use tractor::rule::Rule;

    fn temp_json_file(content: &str) -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.json");
        std::fs::write(&path, content).unwrap();
        (dir, path.to_str().unwrap().to_string())
    }

    fn run(ops: &[Operation]) -> tractor::report::Report {
        let mut builder = ReportBuilder::new();
        execute(ops, &ExecuteOptions::default(), &mut builder).unwrap();
        builder.build()
    }

    #[test]
    fn check_finds_violations() {
        let (_dir, path) = temp_json_file(r#"{"debug": true, "verbose": true}"#);
        let ops = vec![Operation::Check(CheckOperation {
            files: vec![path.clone()],
            exclude: vec![],
            diff_files: None,
            diff_lines: None,
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
            inline_source: None,
        })];
        let report = run(&ops);
        assert!(!report.success.unwrap(), "check should fail when violations found");
        let matches = report.all_matches();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].rule_id.as_deref(), Some("no-debug"));
        assert_eq!(matches[0].reason.as_deref(), Some("debug should not be enabled"));
    }

    #[test]
    fn check_passes_when_no_violations() {
        let (_dir, path) = temp_json_file(r#"{"debug": false}"#);
        let ops = vec![Operation::Check(CheckOperation {
            files: vec![path.clone()],
            exclude: vec![],
            diff_files: None,
            diff_lines: None,
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
            inline_source: None,
        })];
        let report = run(&ops);
        assert!(report.success.unwrap());
    }

    #[test]
    fn check_inline_source_finds_violations() {
        let ops = vec![Operation::Check(CheckOperation {
            files: vec![],
            exclude: vec![],
            diff_files: None,
            diff_lines: None,
            rules: vec![
                Rule::new("no-debug", "//debug[.='true']")
                    .with_reason("debug should not be enabled")
                    .with_severity(Severity::Error),
            ],
            tree_mode: None,
            language: Some("json".into()),
            ignore_whitespace: false,
            parse_depth: None,
            ruleset_include: vec![],
            ruleset_exclude: vec![],
            inline_source: Some(r#"{"debug": true}"#.into()),
        })];
        let report = run(&ops);
        assert!(!report.success.unwrap(), "inline check should fail when violations found");
        let matches = report.all_matches();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].reason.as_deref(), Some("debug should not be enabled"));
    }

    #[test]
    fn check_inline_source_passes_when_no_violations() {
        let ops = vec![Operation::Check(CheckOperation {
            files: vec![],
            exclude: vec![],
            diff_files: None,
            diff_lines: None,
            rules: vec![
                Rule::new("no-debug", "//debug[.='true']")
                    .with_reason("debug should not be enabled"),
            ],
            tree_mode: None,
            language: Some("json".into()),
            ignore_whitespace: false,
            parse_depth: None,
            ruleset_include: vec![],
            ruleset_exclude: vec![],
            inline_source: Some(r#"{"debug": false}"#.into()),
        })];
        let report = run(&ops);
        assert!(report.success.unwrap());
        assert_eq!(report.all_matches().len(), 0);
    }

    #[test]
    fn mixed_check_and_set() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.json");
        std::fs::write(&config_path, r#"{"host": "old"}"#).unwrap();
        let data_path = dir.path().join("data.json");
        std::fs::write(&data_path, r#"{"name": "test"}"#).unwrap();

        let ops = vec![
            Operation::Check(CheckOperation {
                files: vec![data_path.to_str().unwrap().into()],
                exclude: vec![],
                diff_files: None,
                diff_lines: None,
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
                inline_source: None,
            }),
            Operation::Set(set::SetOperation {
                files: vec![config_path.to_str().unwrap().into()],
                exclude: vec![],
                diff_files: None,
                diff_lines: None,
                mappings: vec![set::SetMapping {
                    xpath: "//host".into(),
                    value: "new-host".into(),
                    value_kind: Some("string".into()),
                }],
                tree_mode: None,
                language: None,
                limit: None,
                ignore_whitespace: false,
                inline_source: None,
                write_mode: set::SetWriteMode::InPlace,
                report_mode: set::SetReportMode::PerMatch,
            }),
        ];

        let report = run(&ops);
        assert!(report.success.unwrap());
        let content = std::fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("new-host"));
    }
}
