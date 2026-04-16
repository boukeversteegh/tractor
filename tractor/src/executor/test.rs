//! Test operation: run XPath queries and check match counts against expectations.

use tractor::normalized_xpath::NormalizedXpath;
use tractor::report::ReportBuilder;
use tractor::tree_mode::TreeMode;
use tractor::parse_string_to_documents;

use crate::input::file_resolver::{FileResolver, FileRequest};

use super::{ExecuteOptions, filter_refs, match_to_report_match, query_files_multi, check_expectation};

// ---------------------------------------------------------------------------
// Operation type
// ---------------------------------------------------------------------------

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
}

/// A single test assertion: an XPath query with an expected match count.
#[derive(Debug, Clone)]
pub struct TestAssertion {
    /// XPath expression to evaluate.
    pub xpath: NormalizedXpath,
    /// Expected match count: "none", "some", or a number.
    pub expect: String,
}

// ---------------------------------------------------------------------------
// Execution
// ---------------------------------------------------------------------------

pub(crate) fn execute_test(
    op: &TestOperation,
    options: &ExecuteOptions,
    resolver: &FileResolver,
    report: &mut ReportBuilder,
) -> Result<(), Box<dyn std::error::Error>> {
    // Inline source mode: parse a string and check each assertion individually.
    if let Some(ref source) = op.inline_source {
        let lang = op.language.as_deref()
            .ok_or("inline source requires a language (--lang)")?;
        let mut result = parse_string_to_documents(
            source, lang, "<stdin>".to_string(), op.tree_mode, op.ignore_whitespace,
        )?;
        return run_test_assertions_on_result(&mut result, &op.assertions, op.limit, report);
    }

    let request = FileRequest {
        files: &op.files,
        exclude: &op.exclude,
        diff_files: op.diff_files.as_deref(),
        diff_lines: op.diff_lines.as_deref(),
        command: "test",
    };
    let (files, filters) = resolver.resolve(&request, report);

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
    result: &mut tractor::XeeParseResult,
    assertions: &[TestAssertion],
    limit: Option<usize>,
    report: &mut ReportBuilder,
) -> Result<(), Box<dyn std::error::Error>> {
    for assertion in assertions {
        let mut matches = result.query(assertion.xpath.as_str())?;
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
