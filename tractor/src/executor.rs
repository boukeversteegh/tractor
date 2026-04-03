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

use std::path::PathBuf;
use rayon::prelude::*;
use tractor_core::rule::{Rule, RuleSet};
use tractor_core::report::{ReportBuilder, ReportMatch, Severity};
use tractor_core::tree_mode::TreeMode;
use tractor_core::{expand_globs, filter_supported_files, detect_language, parse_to_documents, parse_string_to_documents, Match, apply_replacements};
use tractor_core::xpath_upsert::{upsert, update_only};

use crate::filter::ResultFilter;
use crate::pipeline::matcher::validate_xpath_diagnostic;
use crate::pipeline::run_rules;
use crate::pipeline::git;

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

/// A query operation: run XPath expressions against files, return matches.
///
/// Supports two input modes:
/// - **Files**: set `files` (and optionally `exclude`). This is the default.
/// - **Inline source**: set `inline_source` and `inline_lang`. Files are ignored.
///
/// Multiple queries can target the same set of files — each file is parsed
/// once and all XPath expressions are evaluated against it.
#[derive(Debug, Clone)]
pub struct QueryOperation {
    /// File glob patterns to include.
    pub files: Vec<String>,
    /// File glob patterns to exclude.
    pub exclude: Vec<String>,
    /// Git diff spec: only consider files changed in this diff.
    pub diff_files: Option<String>,
    /// Git diff spec: only include matches in changed hunks.
    pub diff_lines: Option<String>,
    /// XPath queries to evaluate.
    pub queries: Vec<QueryExpr>,
    /// Tree mode override for parsing.
    pub tree_mode: Option<TreeMode>,
    /// Language override for parsing.
    pub language: Option<String>,
    /// Maximum number of matches to return (across all queries).
    pub limit: Option<usize>,
    /// Ignore whitespace-only text nodes during parsing.
    pub ignore_whitespace: bool,
    /// Maximum parse depth.
    pub parse_depth: Option<usize>,
    /// Inline source string to parse instead of files.
    pub inline_source: Option<String>,
    /// Language for inline source (required when inline_source is set).
    pub inline_lang: Option<String>,
}

/// A single XPath query expression.
#[derive(Debug, Clone)]
pub struct QueryExpr {
    /// XPath expression to evaluate.
    pub xpath: String,
}

/// A check operation: run XPath rules against files, report violations.
#[derive(Debug, Clone)]
pub struct CheckOperation {
    /// File glob patterns to include.
    pub files: Vec<String>,
    /// File glob patterns to exclude.
    pub exclude: Vec<String>,
    /// Git diff spec: only consider files changed in this diff.
    pub diff_files: Option<String>,
    /// Git diff spec: only include matches in changed hunks.
    pub diff_lines: Option<String>,
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

/// A test operation: run XPath queries and check match counts against expectations.
///
/// Multiple assertions can target the same set of files — each file is parsed
/// once and all XPath expressions are evaluated against it.
#[derive(Debug, Clone)]
pub struct TestOperation {
    /// File glob patterns to include.
    pub files: Vec<String>,
    /// File glob patterns to exclude.
    pub exclude: Vec<String>,
    /// Git diff spec: only consider files changed in this diff.
    pub diff_files: Option<String>,
    /// Git diff spec: only include matches in changed hunks.
    pub diff_lines: Option<String>,
    /// Assertions to evaluate.
    pub assertions: Vec<TestAssertion>,
    /// Tree mode override for parsing.
    pub tree_mode: Option<TreeMode>,
    /// Language override for parsing.
    pub language: Option<String>,
    /// Maximum number of matches to return per assertion.
    pub limit: Option<usize>,
    /// Ignore whitespace-only text nodes during parsing.
    pub ignore_whitespace: bool,
    /// Maximum parse depth.
    pub parse_depth: Option<usize>,
    /// Inline source string to parse instead of files.
    pub inline_source: Option<String>,
    /// Language for inline source (required when inline_source is set).
    pub inline_lang: Option<String>,
}

/// A single test assertion: an XPath query with an expected match count.
#[derive(Debug, Clone)]
pub struct TestAssertion {
    /// XPath expression to evaluate.
    pub xpath: String,
    /// Expected match count: "none", "some", or a number.
    pub expect: String,
}

/// An update operation: modify existing matched nodes without creating new structure.
/// Unlike set, update fails if the XPath does not match any existing nodes.
#[derive(Debug, Clone)]
pub struct UpdateOperation {
    /// File glob patterns to include.
    pub files: Vec<String>,
    /// File glob patterns to exclude.
    pub exclude: Vec<String>,
    /// Git diff spec: only consider files changed in this diff.
    pub diff_files: Option<String>,
    /// Git diff spec: only include matches in changed hunks.
    pub diff_lines: Option<String>,
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
    /// Git diff spec: only consider files changed in this diff.
    pub diff_files: Option<String>,
    /// Git diff spec: only include matches in changed hunks.
    pub diff_lines: Option<String>,
    /// Mappings to apply.
    pub mappings: Vec<SetMapping>,
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
    /// Git diff spec for filtering to changed files (e.g. "HEAD~3", "main..HEAD").
    /// When set, resolved files are intersected with the set of changed files.
    pub diff_files: Option<String>,
    /// Git diff spec for filtering matches to changed hunks.
    /// When set, only matches whose lines overlap with changed hunks are included.
    pub diff_lines: Option<String>,
}

impl Default for ExecuteOptions {
    fn default() -> Self {
        ExecuteOptions {
            verbose: false,
            base_dir: None,
            diff_files: None,
            diff_lines: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Executor
// ---------------------------------------------------------------------------

/// Build result filters from global and per-operation diff specs.
///
/// Both global (ExecuteOptions) and per-operation diff specs are applied.
/// Each produces a separate filter; all must pass for a match to be included.
fn build_filters(
    global_diff: Option<&str>,
    op_diff: Option<&str>,
    cwd: &std::path::Path,
) -> Vec<Box<dyn ResultFilter>> {
    let mut filters: Vec<Box<dyn ResultFilter>> = Vec::new();

    for spec in [global_diff, op_diff].into_iter().flatten() {
        match git::DiffHunkFilter::from_spec(spec, cwd) {
            Ok(f) => filters.push(Box::new(f)),
            Err(e) => eprintln!("warning: --diff-lines filter failed: {}", e),
        }
    }

    filters
}

/// Resolve files and build result filters for an operation.
///
/// Combines diff-files (file-level) and diff-lines (hunk-level) filtering
/// with glob expansion and exclude patterns.
fn resolve_op_files(
    files: &[String],
    exclude: &[String],
    diff_files: Option<&str>,
    diff_lines: Option<&str>,
    options: &ExecuteOptions,
) -> (Vec<String>, Vec<Box<dyn ResultFilter>>) {
    let cwd = options.base_dir.as_deref()
        .unwrap_or_else(|| std::path::Path::new("."));
    let filters = build_filters(options.diff_lines.as_deref(), diff_lines, cwd);
    let files = resolve_files(files, exclude, diff_files, &filters, options);
    (files, filters)
}

/// Convert owned filters to borrowed references for passing to query engine.
fn filter_refs(filters: &[Box<dyn ResultFilter>]) -> Vec<&dyn ResultFilter> {
    filters.iter().map(|f| f.as_ref()).collect()
}

/// Execute a list of operations, pushing results into the given `ReportBuilder`.
pub fn execute(
    operations: &[Operation],
    options: &ExecuteOptions,
    report: &mut ReportBuilder,
) -> Result<(), Box<dyn std::error::Error>> {
    for op in operations {
        match op {
            Operation::Query(q) => execute_query(q, options, report)?,
            Operation::Check(c) => execute_check(c, options, report)?,
            Operation::Test(t) => execute_test(t, options, report)?,
            Operation::Set(s) => execute_set(s, options, report)?,
            Operation::Update(u) => execute_update(u, options, report)?,
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Query execution
// ---------------------------------------------------------------------------

fn execute_query(
    op: &QueryOperation,
    options: &ExecuteOptions,
    report: &mut ReportBuilder,
) -> Result<(), Box<dyn std::error::Error>> {
    // Inline source mode: parse a string instead of files.
    if let Some(ref source) = op.inline_source {
        let lang = op.inline_lang.as_deref()
            .or(op.language.as_deref())
            .ok_or("inline source requires a language (--lang)")?;
        return execute_query_inline(source, lang, op, report);
    }

    let (files, filters) = resolve_op_files(
        &op.files, &op.exclude, op.diff_files.as_deref(), op.diff_lines.as_deref(), options,
    );

    if files.is_empty() {
        return Ok(());
    }

    let xpaths: Vec<&str> = op.queries.iter().map(|q| q.xpath.as_str()).collect();

    // Validate all XPath expressions upfront — add fatal diagnostics on failure
    let diagnostics: Vec<_> = xpaths.iter()
        .filter_map(|xpath| validate_xpath_diagnostic(xpath, "query"))
        .collect();
    if !diagnostics.is_empty() {
        report.add_all(diagnostics);
        return Ok(());
    }

    let matches = query_files_multi(
        &files, &xpaths, op.language.as_deref(),
        op.tree_mode, op.ignore_whitespace, op.parse_depth,
        op.limit, options.verbose, &filter_refs(&filters),
    )?;

    report.add_all(matches.into_iter().map(|m| match_to_report_match(m, "query")));

    Ok(())
}

/// Inline source query: parse a string and run all XPath expressions.
fn execute_query_inline(
    source: &str,
    lang: &str,
    op: &QueryOperation,
    report: &mut ReportBuilder,
) -> Result<(), Box<dyn std::error::Error>> {
    // Validate all XPath expressions upfront
    let diagnostics: Vec<_> = op.queries.iter()
        .filter_map(|q| validate_xpath_diagnostic(&q.xpath, "query"))
        .collect();
    if !diagnostics.is_empty() {
        report.add_all(diagnostics);
        return Ok(());
    }

    let mut result = parse_string_to_documents(
        source, lang, "<stdin>".to_string(), op.tree_mode, op.ignore_whitespace,
    )?;

    let mut all_matches = Vec::new();
    for query in &op.queries {
        let matches = result.query(&query.xpath)?;
        all_matches.extend(matches);
    }

    if let Some(limit) = op.limit {
        all_matches.truncate(limit);
    }

    report.add_all(all_matches.into_iter().map(|m| match_to_report_match(m, "query")));

    Ok(())
}

// ---------------------------------------------------------------------------
// Check execution
// ---------------------------------------------------------------------------

fn execute_check(
    op: &CheckOperation,
    options: &ExecuteOptions,
    report: &mut ReportBuilder,
) -> Result<(), Box<dyn std::error::Error>> {
    if op.rules.is_empty() {
        return Ok(());
    }

    // --- Phase 0: Validate XPath expressions upfront ---
    let diagnostics: Vec<_> = op.rules.iter()
        .filter_map(|rule| validate_xpath_diagnostic(&rule.xpath, "check"))
        .collect();
    if !diagnostics.is_empty() {
        report.add_all(diagnostics);
        return Ok(());
    }

    // --- Phase 1: Validate rule examples inline ---
    validate_rule_examples(&op.rules, op.language.as_deref(), op.tree_mode, report)?;

    // --- Phase 2: Run the actual file check ---
    let (files, filters) = resolve_op_files(
        &op.files, &op.exclude, op.diff_files.as_deref(), op.diff_lines.as_deref(), options,
    );

    // Build a RuleSet from the operation. Ruleset-level include/exclude
    // come from rules files; per-rule patterns still participate in glob matching.
    let ruleset = RuleSet {
        rules: op.rules.clone(),
        include: op.ruleset_include.clone(),
        exclude: op.ruleset_exclude.clone(),
        default_tree_mode: op.tree_mode,
        default_language: op.language.clone(),
    };

    if !files.is_empty() {
        let rule_matches = run_rules(
            &ruleset,
            &files,
            op.tree_mode,
            op.ignore_whitespace,
            op.parse_depth,
            options.verbose,
            &filter_refs(&filters),
        )?;

        for rm in rule_matches {
            let rule = &ruleset.rules[rm.rule_index];
            let reason = rule
                .reason
                .clone()
                .unwrap_or_else(|| format!("[{}] check failed", rule.id));
            let severity = rule.severity;

            // Apply rule-level message template (if the rule defines one)
            let message = rule
                .message
                .as_deref()
                .map(|t| tractor_core::format_message(t, &rm.m));

            let mut report_match = match_to_report_match(rm.m, "check");
            report_match.reason = Some(reason);
            report_match.severity = Some(severity);
            report_match.rule_id = Some(rule.id.clone());
            report_match.message = message;
            report.add(report_match);
        }
    }

    Ok(())
}

/// Validate rule examples by parsing and querying inline.
///
/// For each rule with examples, parses the example source and runs the rule's
/// XPath query. Adds failure matches to the builder for any unmet expectations.
fn validate_rule_examples(
    rules: &[Rule],
    default_language: Option<&str>,
    default_tree_mode: Option<TreeMode>,
    report: &mut ReportBuilder,
) -> Result<(), Box<dyn std::error::Error>> {
    for rule in rules {
        if !rule.has_examples() {
            continue;
        }

        let lang = rule.language.as_deref()
            .or(default_language)
            .ok_or_else(|| format!(
                "rule '{}' has examples but no language specified (set language on the rule or check operation)",
                rule.id
            ))?;

        let tree_mode = rule.tree_mode.or(default_tree_mode);

        // Validate valid examples: expect "none" (query should NOT match valid code)
        for (i, example) in rule.valid_examples.iter().enumerate() {
            let mut result = parse_string_to_documents(
                example, lang, "<stdin>".to_string(), tree_mode, false,
            )?;
            let matches = result.query(&rule.xpath)?;
            if !check_expectation("none", matches.len())? {
                report.add(example_failure_match(
                    &rule.id,
                    &format!(
                        "[{}] valid example {} unexpectedly matched query",
                        rule.id, i + 1
                    ),
                ));
            }
        }

        // Validate invalid examples: expect "some" (query SHOULD match invalid code)
        for (i, example) in rule.invalid_examples.iter().enumerate() {
            let mut result = parse_string_to_documents(
                example, lang, "<stdin>".to_string(), tree_mode, false,
            )?;
            let matches = result.query(&rule.xpath)?;
            if !check_expectation("some", matches.len())? {
                report.add(example_failure_match(
                    &rule.id,
                    &format!(
                        "[{}] invalid example {} did not match query",
                        rule.id, i + 1
                    ),
                ));
            }
        }
    }

    Ok(())
}

/// Build a synthetic ReportMatch for a failed example validation.
fn example_failure_match(rule_id: &str, reason: &str) -> ReportMatch {
    ReportMatch {
        file: String::new(),
        line: 0,
        column: 0,
        end_line: 0,
        end_column: 0,
        command: "test".to_string(),
        tree: None,
        value: None,
        source: None,
        lines: None,
        reason: Some(reason.to_string()),
        severity: Some(Severity::Error),
        message: None,
       
        origin: None,
        rule_id: Some(rule_id.to_string()),
        status: None,
        output: None,
    }
}

// ---------------------------------------------------------------------------
// Set execution
// ---------------------------------------------------------------------------

fn execute_set(
    op: &SetOperation,
    options: &ExecuteOptions,
    report: &mut ReportBuilder,
) -> Result<(), Box<dyn std::error::Error>> {
    let (files, _filters) = resolve_op_files(
        &op.files, &op.exclude, op.diff_files.as_deref(), op.diff_lines.as_deref(), options,
    );

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

        // In verify mode, drift means failure
        if op.verify && was_modified {
            report.fail();
        }

        report.add(ReportMatch {
            file: file_path.clone(),
            line: 1,
            column: 1,
            end_line: 1,
            end_column: 1,
            command: "set".to_string(),
            tree: None,
            value: None,
            source: None,
            lines: None,
            reason: None,
            severity: None,
            message: None,
           
            origin: None,
            rule_id: None,
            status: Some(status_str.to_string()),
            output: if was_modified && op.verify {
                Some(format!("{} mapping{} would change", mappings_applied, if mappings_applied == 1 { "" } else { "s" }))
            } else {
                None
            },
        });
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Test execution
// ---------------------------------------------------------------------------

fn execute_test(
    op: &TestOperation,
    options: &ExecuteOptions,
    report: &mut ReportBuilder,
) -> Result<(), Box<dyn std::error::Error>> {
    // Inline source mode: parse a string and check each assertion individually.
    if let Some(ref source) = op.inline_source {
        let lang = op.inline_lang.as_deref()
            .or(op.language.as_deref())
            .ok_or("inline source requires a language (--lang)")?;
        let mut result = parse_string_to_documents(
            source, lang, "<stdin>".to_string(), op.tree_mode, op.ignore_whitespace,
        )?;
        return run_test_assertions_on_result(&mut result, &op.assertions, op.limit, report);
    }

    let (files, filters) = resolve_op_files(
        &op.files, &op.exclude, op.diff_files.as_deref(), op.diff_lines.as_deref(), options,
    );

    if files.is_empty() {
        for assertion in &op.assertions {
            if !check_expectation(&assertion.expect, 0)? {
                report.fail();
            }
        }
        return Ok(());
    }

    // Query each assertion's xpath individually to get per-assertion counts.
    let refs = filter_refs(&filters);
    for assertion in &op.assertions {
        let matches = query_files_multi(
            &files, &[assertion.xpath.as_str()], op.language.as_deref(),
            op.tree_mode, op.ignore_whitespace, op.parse_depth,
            op.limit, options.verbose, &refs,
        )?;
        if !check_expectation(&assertion.expect, matches.len())? {
            report.fail();
        }
        report.add_all(matches.into_iter().map(|m| match_to_report_match(m, "test")));
    }

    Ok(())
}

/// Run test assertions against a single parsed document (inline source).
fn run_test_assertions_on_result(
    result: &mut tractor_core::XeeParseResult,
    assertions: &[TestAssertion],
    limit: Option<usize>,
    report: &mut ReportBuilder,
) -> Result<(), Box<dyn std::error::Error>> {
    for assertion in assertions {
        let mut matches = result.query(&assertion.xpath)?;
        if let Some(limit) = limit {
            matches.truncate(limit);
        }
        if !check_expectation(&assertion.expect, matches.len())? {
            report.fail();
        }
        report.add_all(matches.into_iter().map(|m| match_to_report_match(m, "test")));
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Update execution
// ---------------------------------------------------------------------------

fn execute_update(
    op: &UpdateOperation,
    options: &ExecuteOptions,
    report: &mut ReportBuilder,
) -> Result<(), Box<dyn std::error::Error>> {
    let (files, filters) = resolve_op_files(
        &op.files, &op.exclude, op.diff_files.as_deref(), op.diff_lines.as_deref(), options,
    );
    let mut fallback_files = Vec::new();
    let mut files_modified = std::collections::HashSet::new();

    for file_path in &files {
        let lang = op.language.as_deref()
            .unwrap_or_else(|| detect_language(file_path));
        let source = std::fs::read_to_string(file_path)?;

        match update_only(&source, lang, &op.xpath, &op.value, op.limit) {
            Ok(result) => {
                if result.source != source {
                    std::fs::write(file_path, &result.source)?;
                    files_modified.insert(file_path.clone());
                    for m in &result.matches {
                        let mut rm = match_to_report_match(m.clone(), "update");
                        rm.status = Some("updated".to_string());
                        report.add(rm);
                    }
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
        let matches = query_files_multi(
            &fallback_files, &[op.xpath.as_str()], op.language.as_deref(),
            op.tree_mode, op.ignore_whitespace, op.parse_depth,
            None, options.verbose, &filter_refs(&filters),
        )?;
        if !matches.is_empty() {
            let summary = apply_replacements(&matches, &op.value)?;
            for m in &matches[..summary.replacements_made.min(matches.len())] {
                report.add(ReportMatch {
                    file: m.file.clone(),
                    line: m.line, column: m.column, end_line: m.end_line, end_column: m.end_column,
                    command: "update".to_string(),
                    tree: None, value: None, source: None, lines: None,
                    reason: None, severity: None, message: None,
                    origin: None, rule_id: None,
                    status: Some("updated".to_string()),
                    output: None,
                });
            }
        }
    }

    // No matches with "updated" status means nothing was changed
    if !report.has_updates() {
        report.fail();
    }

    Ok(())
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

/// Convert a raw `Match` into a `ReportMatch` with all content fields populated.
/// Operation-specific fields (reason, severity, rule_id, status, message) are
/// left as None and must be set by the caller.
fn match_to_report_match(m: Match, command: &str) -> ReportMatch {
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
fn query_files_multi(
    files: &[String],
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

fn resolve_files(
    file_globs: &[String],
    exclude_globs: &[String],
    op_diff_files: Option<&str>,
    filters: &[Box<dyn ResultFilter>],
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

    let files = filter_supported_files(files);

    // Intersect with git diff-files. Both global (ExecuteOptions) and
    // per-operation specs apply — each narrows the file set further.
    let cwd = options.base_dir.as_deref()
        .unwrap_or_else(|| std::path::Path::new("."));

    let files = apply_diff_files_filter(files, options.diff_files.as_deref(), cwd);
    let mut files = apply_diff_files_filter(files, op_diff_files, cwd);

    // Apply file-level filtering from result filters (e.g. DiffHunkFilter
    // skips files that have no changed hunks).
    if !filters.is_empty() {
        files.retain(|f| filters.iter().all(|filter| filter.include_file(f)));
    }

    files
}

fn apply_diff_files_filter(files: Vec<String>, spec: Option<&str>, cwd: &std::path::Path) -> Vec<String> {
    match spec {
        Some(spec) => {
            match git::git_changed_files(spec, cwd) {
                Ok(changed) => git::intersect_changed(files, &changed),
                Err(e) => {
                    eprintln!("warning: --diff-files filter failed: {}", e);
                    files
                }
            }
        }
        None => files,
    }
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

    /// Helper: execute operations and build a report.
    fn run(ops: &[Operation]) -> tractor_core::report::Report {
        let mut builder = ReportBuilder::new();
        execute(ops, &ExecuteOptions::default(), &mut builder).unwrap();
        builder.build()
    }

    /// Helper: execute operations with query (no-verdict) mode and build a report.
    fn run_query(ops: &[Operation]) -> tractor_core::report::Report {
        let mut builder = ReportBuilder::new();
        builder.set_no_verdict();
        execute(ops, &ExecuteOptions::default(), &mut builder).unwrap();
        builder.build()
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
            diff_files: None,
            diff_lines: None,
            queries: vec![QueryExpr { xpath: "//name".into() }],
            tree_mode: None,
            language: None,
            limit: None,
            ignore_whitespace: false,
            parse_depth: None,
            inline_source: None,
            inline_lang: None,
        })];

        let report = run_query(&ops);
        assert!(report.success.is_none()); // query reports have no pass/fail
        assert_eq!(report.all_matches().len(), 1);
        assert_eq!(report.all_matches()[0].value.as_deref(), Some("alice"));
    }

    #[test]
    fn query_with_limit() {
        let (_dir, path) = temp_json_file(r#"{"a": 1, "b": 2, "c": 3}"#);

        let ops = vec![Operation::Query(QueryOperation {
            files: vec![path.clone()],
            exclude: vec![],
            diff_files: None,
            diff_lines: None,
            queries: vec![QueryExpr { xpath: "//*[number(.) > 0]".into() }],
            tree_mode: None,
            language: None,
            limit: Some(2),
            ignore_whitespace: false,
            parse_depth: None,
            inline_source: None,
            inline_lang: None,
        })];

        let report = run_query(&ops);
        assert!(report.all_matches().len() <= 2);
    }

    #[test]
    fn query_empty_files() {
        let ops = vec![Operation::Query(QueryOperation {
            files: vec![],
            exclude: vec![],
            diff_files: None,
            diff_lines: None,
            queries: vec![QueryExpr { xpath: "//x".into() }],
            tree_mode: None,
            language: None,
            limit: None,
            ignore_whitespace: false,
            parse_depth: None,
            inline_source: None,
            inline_lang: None,
        })];

        let report = run_query(&ops);
        assert_eq!(report.all_matches().len(), 0);
        assert!(report.success.is_none()); // query reports have no pass/fail
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
            diff_files: None,
            diff_lines: None,
            mappings: vec![SetMapping {
                xpath: "//database/host".into(),
                value: "new-host".into(),
            }],
            language: None,
            verify: false,
        })];

        let report = run(&ops);
        assert!(report.success.unwrap());

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
            diff_files: None,
            diff_lines: None,
            mappings: vec![SetMapping {
                xpath: "//database/host".into(),
                value: "localhost".into(),
            }],
            language: None,
            verify: false,
        })];

        let report = run(&ops);
        assert!(report.success.unwrap());

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("localhost"), "missing node should be created: {}", content);
    }

    #[test]
    fn set_multiple_mappings() {
        let (_dir, path) = temp_json_file(r#"{"database": {"host": "old", "port": 1234}}"#);

        let ops = vec![Operation::Set(SetOperation {
            files: vec![path.clone()],
            exclude: vec![],
            diff_files: None,
            diff_lines: None,
            mappings: vec![
                SetMapping { xpath: "//database/host".into(), value: "new-host".into() },
                SetMapping { xpath: "//database/port".into(), value: "5432".into() },
            ],
            language: None,
            verify: false,
        })];

        let report = run(&ops);
        assert!(report.success.unwrap());

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
            diff_files: None,
            diff_lines: None,
            mappings: vec![SetMapping {
                xpath: "//database/host".into(),
                value: "localhost".into(),
            }],
            language: None,
            verify: false,
        })];

        let report = run(&ops);
        assert!(report.success.unwrap());

        // Check status is "unchanged"
        assert_eq!(report.all_matches()[0].status.as_deref(), Some("unchanged"));
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
            diff_files: None,
            diff_lines: None,
            mappings: vec![SetMapping {
                xpath: "//database/host".into(),
                value: "correct".into(),
            }],
            language: None,
            verify: true,
        })];

        let report = run(&ops);

        // Should fail: drift detected
        assert!(!report.success.unwrap(), "verify should detect drift");

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
            diff_files: None,
            diff_lines: None,
            mappings: vec![SetMapping {
                xpath: "//database/host".into(),
                value: "correct".into(),
            }],
            language: None,
            verify: true,
        })];

        let report = run(&ops);
        assert!(report.success.unwrap(), "verify should pass when values are in sync");
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
        })];

        let report = run(&ops);
        assert!(report.success.unwrap());
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
            }),
            Operation::Set(SetOperation {
                files: vec![config_path.to_str().unwrap().into()],
                exclude: vec![],
                diff_files: None,
                diff_lines: None,
                mappings: vec![SetMapping {
                    xpath: "//host".into(),
                    value: "new-host".into(),
                }],
                language: None,
                verify: false,
            }),
        ];

        let report = run(&ops);
        assert!(report.success.unwrap());

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
            diff_files: None,
            diff_lines: None,
            mappings: vec![SetMapping {
                xpath: "//database/host".into(),
                value: "new-host".into(),
            }],
            language: None,
            verify: false,
        })];

        let report = run(&ops);
        assert!(report.success.unwrap());

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("new-host"), "yaml host should be updated: {}", content);
        assert!(content.contains("5432"), "yaml port should be preserved: {}", content);
    }

    // -----------------------------------------------------------------------
    // Example validation tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_validate_examples_pass_and_fail_correct() {
        // Valid example has no comments → expect "none" passes
        // Invalid example has a comment → expect "some" passes
        let rules = vec![
            Rule::new("no-comments", "//line_comment")
                .with_language("rust")
                .with_valid_examples(vec!["fn main() {}".to_string()])
                .with_invalid_examples(vec!["// hello\nfn main() {}".to_string()]),
        ];
        let mut builder = ReportBuilder::new();
        validate_rule_examples(&rules, None, None, &mut builder).unwrap();
        let report = builder.build();
        assert!(report.all_matches().is_empty(), "expected no failures: {:?}", report.all_matches());
    }

    #[test]
    fn test_validate_examples_valid_unexpectedly_matches() {
        // Valid example has a comment but rule looks for comments → should fail
        let rules = vec![
            Rule::new("no-comments", "//line_comment")
                .with_language("rust")
                .with_valid_examples(vec!["// oops this is a comment".to_string()]),
        ];
        let mut builder = ReportBuilder::new();
        validate_rule_examples(&rules, None, None, &mut builder).unwrap();
        let report = builder.build();
        let matches = report.all_matches();
        assert_eq!(matches.len(), 1);
        assert!(matches[0].reason.as_ref().unwrap().contains("valid example 1 unexpectedly matched"));
    }

    #[test]
    fn test_validate_examples_invalid_does_not_match() {
        // Invalid example has no comment but rule looks for comments → should fail
        let rules = vec![
            Rule::new("no-comments", "//line_comment")
                .with_language("rust")
                .with_invalid_examples(vec!["fn main() {}".to_string()]),
        ];
        let mut builder = ReportBuilder::new();
        validate_rule_examples(&rules, None, None, &mut builder).unwrap();
        let report = builder.build();
        let matches = report.all_matches();
        assert_eq!(matches.len(), 1);
        assert!(matches[0].reason.as_ref().unwrap().contains("invalid example 1 did not match"));
    }

    #[test]
    fn test_validate_examples_language_from_operation() {
        // Rule has no language but operation default is "rust"
        let rules = vec![
            Rule::new("no-comments", "//line_comment")
                .with_valid_examples(vec!["fn main() {}".to_string()]),
        ];
        let mut builder = ReportBuilder::new();
        validate_rule_examples(&rules, Some("rust"), None, &mut builder).unwrap();
        let report = builder.build();
        assert!(report.all_matches().is_empty());
    }

    #[test]
    fn test_validate_examples_no_language_errors() {
        // Rule has examples but no language anywhere → error
        let rules = vec![
            Rule::new("no-comments", "//line_comment")
                .with_valid_examples(vec!["fn main() {}".to_string()]),
        ];
        let mut builder = ReportBuilder::new();
        let err = validate_rule_examples(&rules, None, None, &mut builder).unwrap_err();
        assert!(err.to_string().contains("no language specified"));
    }

    #[test]
    fn test_validate_examples_no_examples_is_noop() {
        let rules = vec![Rule::new("simple", "//function")];
        let mut builder = ReportBuilder::new();
        validate_rule_examples(&rules, None, None, &mut builder).unwrap();
        let report = builder.build();
        assert!(report.all_matches().is_empty());
    }
}
