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

pub use query::{QueryOperation, QueryOperationPlan, QueryExpr, QueryOutput, QueryOutputField};
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
///
/// Operations run strictly in order, and `$`-directive variable sources
/// (`$file`) are loaded at each operation's start — so a file written by an
/// earlier operation (e.g. a query op's `output:`) is read fresh by the
/// next one. Each operation sees one consistent snapshot; the ordered
/// `operations:` list is the dependency mechanism.
///
/// *Which* sources load is decided at config load, per entry, and recorded
/// on [`tractor::EntryVariables`] — this loop reads those flags rather than
/// inspecting XPath. A source no expression reads is never loaded, so an
/// operation cannot fail over a file a later operation will write.
pub fn execute(
    operations: &[OperationPlan],
    ctx: &ExecCtx<'_>,
    report: &mut ReportBuilder,
) -> Result<(), Box<dyn std::error::Error>> {
    for op in operations {
        let base_dir = ctx.base_dir.unwrap_or_else(|| std::path::Path::new("."));

        // Fresh snapshots for this operation. What gets resolved was decided
        // at config load and travels on each entry's `EntryVariables` — the
        // executor only reads those flags, it does not re-derive policy.
        let run_vars = tractor::variables::bind_run_variables(
            &ctx.query_variables(),
            entries_read_run_variables(op),
            base_dir,
        )?;
        let op_ctx = ExecCtx { variables: Some(&run_vars), ..*ctx };

        // Entry namespaces resolve per entry: one rule reading its own
        // variables must not force resolution for a sibling that doesn't.
        // Borrows the plan untouched when no entry declares a source.
        let op = resolve_entry_variables(op, base_dir)?;

        match op.as_ref() {
            OperationPlan::Query(q) => query::execute_query(q, &op_ctx, report)?,
            OperationPlan::Check(c) => check::execute_check(c, &op_ctx, report)?,
            OperationPlan::Test(t) => test::execute_test(t, &op_ctx, report)?,
            OperationPlan::Set(s) => set::execute_set(s, &op_ctx, report)?,
            OperationPlan::Update(u) => update::execute_update(u, &op_ctx, report)?,
        }
    }

    Ok(())
}

/// Every entry-level `EntryVariables` in an operation, in declaration order.
fn entry_variables(op: &OperationPlan) -> Box<dyn Iterator<Item = &tractor::EntryVariables> + '_> {
    match op {
        OperationPlan::Check(c) => Box::new(c.compiled_rules.iter().map(|r| &r.variables)),
        OperationPlan::Set(s) => Box::new(s.mappings.iter().map(|m| &m.variables)),
        OperationPlan::Query(q) => Box::new(q.queries.iter().map(|q| &q.variables)),
        OperationPlan::Test(t) => Box::new(t.assertions.iter().map(|a| &a.variables)),
        OperationPlan::Update(_) => Box::new(std::iter::empty()),
    }
}

/// Whether any of the operation's expressions reads the run-level
/// `$variables`. Reads the flag each entry recorded at config load rather
/// than re-scanning XPath strings.
///
/// `update` has no entries, so its own xpath is consulted directly.
fn entries_read_run_variables(op: &OperationPlan) -> bool {
    match op {
        OperationPlan::Update(u) => tractor::variables::reads_run_variables(&u.xpath),
        _ => entry_variables(op).any(|v| v.reads_run_variables()),
    }
}

/// Resolve `$`-directive sources in an operation's entry-level variables
/// (rule / mapping / query / assertion `variables:`), **per entry**: an
/// entry that never reads its own namespace neither resolves its sources
/// (so a file a later operation will write cannot fail this one) nor binds
/// them raw. Returns the plan unchanged (borrowed) when no entry declares
/// a source — the common case.
fn resolve_entry_variables<'a>(
    op: &'a OperationPlan,
    base_dir: &std::path::Path,
) -> Result<std::borrow::Cow<'a, OperationPlan>, tractor::variables::VariableSourceError> {
    use std::borrow::Cow;

    if !entry_variables(op).any(|v| v.has_sources()) {
        return Ok(Cow::Borrowed(op));
    }

    /// Replace an entry's declared variables with what should be bound.
    fn bind(
        variables: &mut tractor::EntryVariables,
        base_dir: &std::path::Path,
    ) -> Result<(), tractor::variables::VariableSourceError> {
        let bound = variables.bind(base_dir)?;
        *variables = variables.clone().with_bound(bound);
        Ok(())
    }

    let mut op = op.clone();
    match &mut op {
        OperationPlan::Check(c) => {
            for rule in &mut c.compiled_rules {
                bind(&mut rule.variables, base_dir)?;
            }
        }
        OperationPlan::Set(s) => {
            for mapping in &mut s.mappings {
                bind(&mut mapping.variables, base_dir)?;
            }
        }
        OperationPlan::Query(q) => {
            for query in &mut q.queries {
                bind(&mut query.variables, base_dir)?;
            }
        }
        OperationPlan::Test(t) => {
            for assertion in &mut t.assertions {
                bind(&mut assertion.variables, base_dir)?;
            }
        }
        OperationPlan::Update(_) => {}
    }
    Ok(Cow::Owned(op))
}

/// Execute like [`execute`], but when an operation fails, first render the
/// diagnostics accumulated so far — advisory variable warnings, earlier
/// operations' matches — before propagating the error. Without this, the
/// report that often *explains* the failure (e.g. a warning that a set
/// mapping's variable lookup is an empty map) is silently dropped on the
/// error path.
///
/// Rendering is best-effort: a render failure never masks the original error.
pub fn execute_rendering_partial_report(
    operations: &[OperationPlan],
    ctx: &ExecCtx<'_>,
    report: &mut ReportBuilder,
    run_ctx: &crate::cli::context::RunContext,
) -> Result<(), Box<dyn std::error::Error>> {
    let result = execute(operations, ctx, report);
    if result.is_err() {
        let mut partial = std::mem::replace(report, ReportBuilder::new()).build();
        crate::matcher::prepare_report_for_output(&mut partial, run_ctx);
        let _ = crate::format::render_report(&partial, run_ctx, None);
    }
    result
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
            result.bindings.variables = std::sync::Arc::clone(variables);

            let mut file_matches = Vec::new();
            for (xpath_expr, entry) in queries {
                result.bindings.entry = entry.clone();
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

    /// Resolution is per entry, not per operation: a rule that declares a
    /// source it never reads must not resolve it — otherwise a sibling
    /// rule's use of `$rule.variables` would drag it in and fail the run on
    /// a file a later operation has yet to write.
    #[test]
    fn unread_rule_sources_do_not_resolve_when_a_sibling_reads_its_own() {
        use tractor::variables::{QueryVariables, VariableValue};

        let (_dir, path) = temp_json_file(r#"{"debug": true}"#);
        let mut reader_vars = QueryVariables::new();
        reader_vars.insert("flag", VariableValue::String("debug".into()));
        // The sibling declares a `$file` source pointing at a file that does
        // not exist — resolving it would be a hard error.
        let declaring_vars: QueryVariables =
            serde_yaml::from_str("data:\n  $file: written-by-a-later-op.json\n").unwrap();

        let mut plan = CheckOperationPlan {
            sources: disk_sources(&[&path]),
            filters: Filters::default(),
            compiled_rules: compile(
                vec![
                    // Reads its own namespace → its sources resolve.
                    Rule::new("reader", "//*[local-name() = $rule.variables?flag]"),
                    // Declares a source but never reads it → must be skipped.
                    Rule::new("declarer", "//nothing"),
                ],
                None,
            ),
            tree_mode: None,
            ignore_whitespace: false,
            parse_depth: None,
        };
        plan.compiled_rules[0].variables = tractor::EntryVariables::new(
            reader_vars,
            tractor::EntryKind::Rule,
            plan.compiled_rules[0].xpath.as_str(),
        );
        plan.compiled_rules[1].variables = tractor::EntryVariables::new(
            declaring_vars,
            tractor::EntryKind::Rule,
            plan.compiled_rules[1].xpath.as_str(),
        );

        let mut builder = ReportBuilder::new();
        execute(&[OperationPlan::Check(plan)], &ExecCtx::default(), &mut builder)
            .expect("an unread source must not be resolved, so the missing file cannot fail the run");
        let report = builder.build();
        assert!(!report.success.unwrap(), "the reading rule should still fire");
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
                plan.compiled_rules[0].variables = tractor::EntryVariables::new(
                    rule_vars.clone(),
                    tractor::EntryKind::Rule,
                    plan.compiled_rules[0].xpath.as_str(),
                );
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
