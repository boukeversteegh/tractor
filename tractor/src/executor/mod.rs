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
///
/// `variables` are the run's user-defined variables, bound into each query's
/// dynamic context alongside the built-in `$file`. Each query carries its
/// own optional entry context (e.g. a query entry's `variables:` bound as
/// `$query.variables`), applied per expression.
pub(crate) fn query_files_multi(
    sources: &[Source],
    queries: &[(&str, Option<tractor::EntryContext>)],
    lang: Option<&str>,
    tree_mode: Option<TreeMode>,
    ignore_whitespace: bool,
    parse_depth: Option<usize>,
    limit: Option<usize>,
    verbose: bool,
    filters: &Filters,
    variables: &std::sync::Arc<tractor::QueryVariables>,
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
            result.variables = std::sync::Arc::clone(variables);

            let mut file_matches = Vec::new();
            for (xpath_expr, entry) in queries {
                result.entry = entry.clone();
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

    /// Variables from the run context are bound as the $variables map in
    /// rule queries, alongside the built-in $file.
    #[test]
    fn check_binds_user_variables() {
        use tractor::variables::{QueryVariables, VariableValue};

        let (_dir, path) = temp_json_file(r#"{"debug": true}"#);
        let ops = vec![OperationPlan::Check(CheckOperationPlan {
            sources: disk_sources(&[&path]),
            filters: Filters::default(),
            compiled_rules: compile(
                vec![
                    Rule::new("no-debug-in-prod", "//debug[.='true'][$variables?env = 'production']")
                        .with_reason("debug must be off in production"),
                ],
                None,
            ),
            tree_mode: None,
            ignore_whitespace: false,
            parse_depth: None,
        })];

        let mut vars = QueryVariables::new();
        vars.insert("env", VariableValue::String("production".into()));
        let vars = std::sync::Arc::new(vars);
        let ctx = ExecCtx { variables: Some(&vars), ..ExecCtx::default() };

        let mut builder = ReportBuilder::new();
        execute(&ops, &ctx, &mut builder).unwrap();
        let report = builder.build();
        assert!(!report.success.unwrap(), "rule should fire when env = 'production'");
        assert_eq!(report.all_matches().len(), 1);

        // Same op with env bound differently → rule predicate is false
        let mut vars = QueryVariables::new();
        vars.insert("env", VariableValue::String("dev".into()));
        let vars = std::sync::Arc::new(vars);
        let ctx = ExecCtx { variables: Some(&vars), ..ExecCtx::default() };

        let mut builder = ReportBuilder::new();
        execute(&ops, &ctx, &mut builder).unwrap();
        let report = builder.build();
        assert!(report.success.unwrap(), "rule should not fire when env = 'dev'");
    }

    /// Rule-level variables are bound as $rule.variables, independent of the
    /// run-level $variables map.
    #[test]
    fn check_binds_rule_variables() {
        use tractor::variables::{QueryVariables, VariableValue};

        let (_dir, path) = temp_json_file(r#"{"debug": true}"#);
        let mut rule_vars = QueryVariables::new();
        rule_vars.insert("flag", VariableValue::String("debug".into()));

        let ops = vec![OperationPlan::Check(CheckOperationPlan {
            sources: disk_sources(&[&path]),
            filters: Filters::default(),
            compiled_rules: compile(
                vec![
                    // Matches the element whose *name* equals the rule-level
                    // "flag" variable; also asserts run-level lookup is empty.
                    Rule::new(
                        "flag-off",
                        "//*[local-name() = $rule.variables?flag][empty($variables?flag)]",
                    ),
                ],
                None,
            ),
            tree_mode: None,
            ignore_whitespace: false,
            parse_depth: None,
        })]
        .into_iter()
        .map(|op| match op {
            OperationPlan::Check(mut plan) => {
                plan.compiled_rules[0].variables = std::sync::Arc::new(rule_vars.clone());
                OperationPlan::Check(plan)
            }
            other => other,
        })
        .collect::<Vec<_>>();

        let report = run(&ops);
        assert!(!report.success.unwrap(), "$rule.variables?flag should select //debug");
        // One error match; the deliberate $variables?flag probe (undefined at
        // run level) additionally produces an advisory warning.
        let errors: Vec<_> = report.all_matches().into_iter()
            .filter(|m| m.severity == Some(Severity::Error))
            .collect();
        assert_eq!(errors.len(), 1);
    }

    /// A literal lookup on a key that isn't defined produces an advisory
    /// warning (never a failure): the lookup legally yields the empty
    /// sequence, but a typo'd key silently disables a rule.
    #[test]
    fn check_warns_on_undefined_variable_keys() {
        use tractor::variables::{QueryVariables, VariableValue};

        let (_dir, path) = temp_json_file(r#"{"debug": true}"#);
        let ops = vec![OperationPlan::Check(CheckOperationPlan {
            sources: disk_sources(&[&path]),
            filters: Filters::default(),
            compiled_rules: compile(
                vec![Rule::new(
                    "r",
                    // `env` is defined (but != 'production', so no match);
                    // `evn` (typo) and `max` (rule-level) are undefined
                    "//debug[$variables?env = 'production' or $variables?evn = 'production' or $rule.variables?max > 1]",
                )],
                None,
            ),
            tree_mode: None,
            ignore_whitespace: false,
            parse_depth: None,
        })];

        let mut vars = QueryVariables::new();
        vars.insert("env", VariableValue::String("prod".into()));
        let vars = std::sync::Arc::new(vars);
        let ctx = ExecCtx { variables: Some(&vars), ..ExecCtx::default() };

        let mut builder = ReportBuilder::new();
        execute(&ops, &ctx, &mut builder).unwrap();
        let report = builder.build();

        let warnings: Vec<_> = report.all_matches().into_iter()
            .filter(|m| m.severity == Some(Severity::Warning))
            .collect();
        assert_eq!(warnings.len(), 2, "one warning per undefined key: {:?}",
            report.all_matches().iter().map(|m| &m.reason).collect::<Vec<_>>());
        assert!(warnings.iter().any(|m| m.reason.as_deref().unwrap_or("").contains("'evn'")));
        assert!(warnings.iter().any(|m| m.reason.as_deref().unwrap_or("").contains("'max'")));
        // Warnings are advisory — the run still succeeds (rule matched nothing)
        assert_eq!(report.success, Some(true), "warnings must not fail the run");
    }

    /// A rule referencing an undeclared XPath variable (anything other than
    /// the built-ins $file / $variables / $rule.variables) fails validation
    /// with a fatal diagnostic instead of executing.
    #[test]
    fn check_unknown_variable_is_fatal() {
        let (_dir, path) = temp_json_file(r#"{"debug": true}"#);
        let ops = vec![OperationPlan::Check(CheckOperationPlan {
            sources: disk_sources(&[&path]),
            filters: Filters::default(),
            compiled_rules: compile(
                vec![Rule::new("r", "//debug[$undeclared = 'x']")],
                None,
            ),
            tree_mode: None,
            ignore_whitespace: false,
            parse_depth: None,
        })];
        let mut builder = ReportBuilder::new();
        execute(&ops, &ExecCtx::default(), &mut builder).unwrap();
        let report = builder.build();
        assert!(report.all_matches().iter().any(|m|
            m.severity == Some(Severity::Fatal)),
            "undeclared variable should produce a fatal diagnostic");
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
                    variables: Default::default(),
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
