//! Batch executor for tractor operations.
//!
//! The executor is the core engine of tractor. It takes a list of operations
//! and pushes results into a `ReportBuilder`. Operations can come from:
//!
//! - A config file (`tractor run config.yaml`)
//! - CLI commands (`tractor check`, `tractor query`, etc.)
//! - Programmatic construction
//!
//! ## Input-resolution boundary
//!
//! An `Operation` arriving here is already input-resolved: it carries a
//! `Vec<Source>` and the `ResultFilter`s it needs. Glob expansion, CLI
//! intersection, diff-files intersection, language detection and inline
//! stdin/`-s` handling all happen *before* construction (see `input::resolve_operation_inputs`).
//! The executor therefore treats disk files and virtual inline sources
//! identically — a single `Vec<Source>` per operation, no branching.

mod query;
mod check;
mod test;
mod set;
mod update;

use std::path::PathBuf;

use rayon::prelude::*;
use tractor::report::{ReportBuilder, ReportMatch};
use tractor::tree_mode::TreeMode;
use tractor::Match;

use crate::input::filter::ResultFilter;
use crate::input::Source;

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
///
/// Note: `Operation` is not `Clone` or `Debug` because it carries
/// `Vec<Box<dyn ResultFilter>>` which is neither. Construct, execute,
/// and discard.
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
///
/// This is pared down after the input-resolution boundary moved upstream:
/// glob/CLI/diff-files intersection is resolved into `Operation.sources`
/// *before* `execute()`, so only execution-time knobs remain here.
#[derive(Debug, Clone, Default)]
pub struct ExecuteOptions {
    /// Print verbose diagnostics to stderr.
    pub verbose: bool,
    /// Base directory for resolving relative file paths in rule includes.
    /// Used by `run_rules` to anchor per-rule `include:` globs.
    pub base_dir: Option<PathBuf>,
}

// ---------------------------------------------------------------------------
// Executor
// ---------------------------------------------------------------------------

/// Convert owned filters to borrowed references for passing to query engine.
pub(crate) fn filter_refs(filters: &[Box<dyn ResultFilter>]) -> Vec<&dyn ResultFilter> {
    filters.iter().map(|f| f.as_ref()).collect()
}

/// Execute a list of operations, pushing results into the given `ReportBuilder`.
///
/// Operations must already carry resolved `sources` and `filters`; this
/// function is a thin dispatcher.
pub fn execute(
    operations: &[Operation],
    options: &ExecuteOptions,
    report: &mut ReportBuilder,
) -> Result<(), Box<dyn std::error::Error>> {
    for op in operations {
        match op {
            Operation::Query(q) => query::execute_query(q, options, report)?,
            Operation::Check(c) => check::execute_check(c, options, report)?,
            Operation::Test(t) => test::execute_test(t, options, report)?,
            Operation::Set(s) => set::execute_set(s, options, report)?,
            Operation::Update(u) => update::execute_update(u, options, report)?,
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

/// Parse and query sources in parallel with multiple XPath expressions.
/// Each source is parsed once and all expressions are evaluated against it.
///
/// Virtual and disk sources flow through the same loop — `source.parse()`
/// dispatches on content kind so the caller doesn't branch.
pub(crate) fn query_files_multi(
    sources: &[Source],
    xpaths: &[&str],
    lang: Option<&str>,
    tree_mode: Option<TreeMode>,
    ignore_whitespace: bool,
    parse_depth: Option<usize>,
    limit: Option<usize>,
    verbose: bool,
    filters: &[&dyn ResultFilter],
) -> Result<Vec<Match>, Box<dyn std::error::Error>> {
    let mut all_matches: Vec<Match> = sources
        .par_iter()
        .filter_map(|source| {
            let path_str = source.path.as_str();
            let mut result = match source.parse(lang, tree_mode, ignore_whitespace, parse_depth) {
                Ok(r) => r,
                Err(e) => {
                    if verbose {
                        eprintln!("warning: {}: {}", path_str, e);
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
                            eprintln!("warning: {}: query error: {}", path_str, e);
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
    use tractor::NormalizedPath;

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

    fn disk_sources(paths: &[&str]) -> Vec<Source> {
        paths
            .iter()
            .map(|p| {
                let np = NormalizedPath::absolute(p);
                let lang = tractor::detect_language(np.as_str()).to_string();
                Source::disk(np, lang)
            })
            .collect()
    }

    #[test]
    fn check_finds_violations() {
        let (_dir, path) = temp_json_file(r#"{"debug": true, "verbose": true}"#);
        let ops = vec![Operation::Check(CheckOperation {
            sources: disk_sources(&[&path]),
            filters: vec![],
            rules: vec![
                Rule::new("no-debug", "//debug[.='true']")
                    .with_reason("debug should not be enabled")
                    .with_severity(Severity::Error),
            ],
            tree_mode: None,
            ignore_whitespace: false,
            parse_depth: None,
            ruleset_include: vec![],
            ruleset_exclude: vec![],
            ruleset_default_language: None,
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
            sources: disk_sources(&[&path]),
            filters: vec![],
            rules: vec![
                Rule::new("no-debug", "//debug[.='true']")
                    .with_reason("debug should not be enabled"),
            ],
            tree_mode: None,
            ignore_whitespace: false,
            parse_depth: None,
            ruleset_include: vec![],
            ruleset_exclude: vec![],
            ruleset_default_language: None,
        })];
        let report = run(&ops);
        assert!(report.success.unwrap());
    }

    #[test]
    fn check_inline_source_finds_violations() {
        let inline = Source::inline_pathless(
            "json",
            std::sync::Arc::new(r#"{"debug": true}"#.to_string()),
        );
        let ops = vec![Operation::Check(CheckOperation {
            sources: vec![inline],
            filters: vec![],
            rules: vec![
                Rule::new("no-debug", "//debug[.='true']")
                    .with_reason("debug should not be enabled")
                    .with_severity(Severity::Error),
            ],
            tree_mode: None,
            ignore_whitespace: false,
            parse_depth: None,
            ruleset_include: vec![],
            ruleset_exclude: vec![],
            ruleset_default_language: Some("json".into()),
        })];
        let report = run(&ops);
        assert!(!report.success.unwrap(), "inline check should fail when violations found");
        let matches = report.all_matches();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].reason.as_deref(), Some("debug should not be enabled"));
    }

    #[test]
    fn check_inline_source_passes_when_no_violations() {
        let inline = Source::inline_pathless(
            "json",
            std::sync::Arc::new(r#"{"debug": false}"#.to_string()),
        );
        let ops = vec![Operation::Check(CheckOperation {
            sources: vec![inline],
            filters: vec![],
            rules: vec![
                Rule::new("no-debug", "//debug[.='true']")
                    .with_reason("debug should not be enabled"),
            ],
            tree_mode: None,
            ignore_whitespace: false,
            parse_depth: None,
            ruleset_include: vec![],
            ruleset_exclude: vec![],
            ruleset_default_language: Some("json".into()),
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
                sources: disk_sources(&[data_path.to_str().unwrap()]),
                filters: vec![],
                rules: vec![
                    Rule::new("has-name", "//name[.='missing']")
                        .with_reason("name should not be 'missing'"),
                ],
                tree_mode: None,
                ignore_whitespace: false,
                parse_depth: None,
                ruleset_include: vec![],
                ruleset_exclude: vec![],
                ruleset_default_language: None,
            }),
            Operation::Set(set::SetOperation {
                sources: disk_sources(&[config_path.to_str().unwrap()]),
                filters: vec![],
                mappings: vec![set::SetMapping {
                    xpath: "//host".into(),
                    value: "new-host".into(),
                    value_kind: Some("string".into()),
                }],
                tree_mode: None,
                limit: None,
                ignore_whitespace: false,
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
