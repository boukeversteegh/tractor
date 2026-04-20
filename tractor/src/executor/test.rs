//! Test operation: run XPath queries and check match counts against expectations.

use tractor::normalized_xpath::NormalizedXpath;
use tractor::report::ReportBuilder;
use tractor::tree_mode::TreeMode;

use crate::input::filter::ResultFilter;
use crate::input::Source;

use crate::cli::context::ExecCtx;

use super::{filter_refs, match_to_report_match, query_files_multi, check_expectation};

// ---------------------------------------------------------------------------
// Operation type
// ---------------------------------------------------------------------------

/// A test operation: run XPath queries and check match counts against expectations.
///
/// Multiple assertions can target the same set of sources — each source is
/// parsed once per assertion.
pub struct TestOperation {
    /// Pre-resolved unified input list.
    pub sources: Vec<Source>,
    /// Pre-built result filters.
    pub filters: Vec<Box<dyn ResultFilter>>,
    /// Assertions to evaluate.
    pub assertions: Vec<TestAssertion>,
    /// Tree mode override for parsing.
    pub tree_mode: Option<TreeMode>,
    /// Language override applied during parsing.
    pub language: Option<String>,
    /// Maximum number of matches to return per assertion.
    pub limit: Option<usize>,
    /// Ignore whitespace-only text nodes during parsing.
    pub ignore_whitespace: bool,
    /// Maximum parse depth.
    pub parse_depth: Option<usize>,
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
    ctx: &ExecCtx<'_>,
    report: &mut ReportBuilder,
) -> Result<(), Box<dyn std::error::Error>> {
    if op.sources.is_empty() {
        for assertion in &op.assertions {
            if !check_expectation(&assertion.expect, 0)? {
                report.fail();
            }
        }
        return Ok(());
    }

    // Query each assertion's xpath individually to get per-assertion counts.
    let refs = filter_refs(&op.filters);
    for assertion in &op.assertions {
        let matches = query_files_multi(
            &op.sources, &[assertion.xpath.as_str()], op.language.as_deref(),
            op.tree_mode, op.ignore_whitespace, op.parse_depth,
            op.limit, ctx.verbose, &refs,
        )?;
        if !check_expectation(&assertion.expect, matches.len())? {
            report.fail();
        }
        report.add_all(matches.into_iter().map(|m| match_to_report_match(m, "test")));
    }

    Ok(())
}
