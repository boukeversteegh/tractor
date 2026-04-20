//! Batch executor for tractor operations.
//!
//! The executor is the core engine of tractor. It takes a list of operation
//! plans and pushes results into a `ReportBuilder`. Plans can come from:
//!
//! - A config file (`tractor run config.yaml`)
//! - CLI commands (`tractor check`, `tractor query`, etc.)
//! - Programmatic construction
//!
//! ## Input-resolution boundary
//!
//! An `OperationPlan` arriving here is already input-resolved: it carries a
//! `Vec<Source>` and the [`Filters`] envelope it needs. Glob expansion,
//! CLI intersection, diff-files intersection, language detection and
//! inline stdin/`-s` handling all happen *before* construction (see
//! `input::resolve_operation_inputs`). The executor therefore treats
//! disk files and virtual inline sources identically — a single
//! `Vec<Source>` per operation, no branching.

mod query;
mod check;
mod test;
mod set;
mod update;

use rayon::prelude::*;
use tractor::report::{ReportBuilder, ReportMatch};
use tractor::tree_mode::TreeMode;
use tractor::Match;

use crate::cli::context::ExecCtx;
use crate::input::filter::Filters;
use crate::input::Source;

pub use query::{QueryOperation, QueryOperationPlan, QueryExpr};
pub use check::CheckOperationPlan;
pub use test::{TestOperation, TestOperationPlan, TestAssertion};
pub use set::{SetOperation, SetOperationPlan, SetMapping, SetWriteMode, SetReportMode};
pub use update::{UpdateOperation, UpdateOperationPlan};

// ---------------------------------------------------------------------------
// Operation types (stable API)
// ---------------------------------------------------------------------------

/// A single operation plan to execute. This is the stable intermediate
/// representation the executor consumes — config files and CLI commands
/// build an `Operation`, which the planner resolves into an `OperationPlan`.
#[derive(Debug, Clone)]
pub enum OperationPlan {
    Query(QueryOperationPlan),
    Check(CheckOperationPlan),
    Test(TestOperationPlan),
    Set(SetOperationPlan),
    Update(UpdateOperationPlan),
}

// ---------------------------------------------------------------------------
// Execution defaults
// ---------------------------------------------------------------------------

/// Default maximum number of files tractor will process.
#[allow(dead_code)]
pub const DEFAULT_MAX_FILES: usize = 10_000;

// ---------------------------------------------------------------------------
// Executor
// ---------------------------------------------------------------------------

/// Execute a list of operation plans, pushing results into the given `ReportBuilder`.
///
/// Plans must already carry resolved `sources` and `filters`; this function
/// is a thin dispatcher. The `ExecCtx` carries the environmental state
/// (verbose, base_dir) that originates in `RunContext` — the single source
/// of truth per CLI invocation.
pub fn execute(
    operations: &[OperationPlan],
    ctx: &ExecCtx<'_>,
    report: &mut ReportBuilder,
) -> Result<(), Box<dyn std::error::Error>> {
    for op in operations {
        match op {
            OperationPlan::Query(q) => query::execute_query(q, ctx, report)?,
            OperationPlan::Check(c) => check::execute_check(c, ctx, report)?,
            OperationPlan::Test(t) => test::execute_test(t, ctx, report)?,
            OperationPlan::Set(s) => set::execute_set(s, ctx, report)?,
            OperationPlan::Update(u) => update::execute_update(u, ctx, report)?,
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
    filters: &Filters,
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
                file_matches.retain(|m| filters.include(m));
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

    fn run(ops: &[OperationPlan]) -> tractor::report::Report {
        let mut builder = ReportBuilder::new();
        execute(ops, &ExecCtx::default(), &mut builder).unwrap();
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

    /// Helper: compile a vec of rules the way CLI paths do, with no ruleset
    /// boundary and no base_dir (suitable for tests with already-absolute
    /// paths and no per-rule globs).
    fn compile(rules: Vec<Rule>, default_language: Option<&str>) -> Vec<tractor::CompiledRule> {
        tractor::compile_ruleset(&[], &[], default_language, None, rules, None)
            .expect("no globs → compile cannot fail")
    }

    #[test]
    fn check_finds_violations() {
        let (_dir, path) = temp_json_file(r#"{"debug": true, "verbose": true}"#);
        let ops = vec![OperationPlan::Check(CheckOperationPlan {
            sources: disk_sources(&[&path]),
            filters: Filters::default(),
            compiled_rules: compile(
                vec![
                    Rule::new("no-debug", "//debug[.='true']")
                        .with_reason("debug should not be enabled")
                        .with_severity(Severity::Error),
                ],
                None,
            ),
            tree_mode: None,
            ignore_whitespace: false,
            parse_depth: None,
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
        let ops = vec![OperationPlan::Check(CheckOperationPlan {
            sources: disk_sources(&[&path]),
            filters: Filters::default(),
            compiled_rules: compile(
                vec![
                    Rule::new("no-debug", "//debug[.='true']")
                        .with_reason("debug should not be enabled"),
                ],
                None,
            ),
            tree_mode: None,
            ignore_whitespace: false,
            parse_depth: None,
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
        let ops = vec![OperationPlan::Check(CheckOperationPlan {
            sources: vec![inline],
            filters: Filters::default(),
            compiled_rules: compile(
                vec![
                    Rule::new("no-debug", "//debug[.='true']")
                        .with_reason("debug should not be enabled")
                        .with_severity(Severity::Error),
                ],
                Some("json"),
            ),
            tree_mode: None,
            ignore_whitespace: false,
            parse_depth: None,
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
        let ops = vec![OperationPlan::Check(CheckOperationPlan {
            sources: vec![inline],
            filters: Filters::default(),
            compiled_rules: compile(
                vec![
                    Rule::new("no-debug", "//debug[.='true']")
                        .with_reason("debug should not be enabled"),
                ],
                Some("json"),
            ),
            tree_mode: None,
            ignore_whitespace: false,
            parse_depth: None,
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
            OperationPlan::Check(CheckOperationPlan {
                sources: disk_sources(&[data_path.to_str().unwrap()]),
                filters: Filters::default(),
                compiled_rules: compile(
                    vec![
                        Rule::new("has-name", "//name[.='missing']")
                            .with_reason("name should not be 'missing'"),
                    ],
                    None,
                ),
                tree_mode: None,
                ignore_whitespace: false,
                parse_depth: None,
            }),
            OperationPlan::Set(set::SetOperationPlan {
                sources: disk_sources(&[config_path.to_str().unwrap()]),
                filters: Filters::default(),
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
