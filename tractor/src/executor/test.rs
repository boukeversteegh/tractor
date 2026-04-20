//! Test operation: run XPath queries and check match counts against expectations.

use tractor::normalized_xpath::NormalizedXpath;
use tractor::report::ReportBuilder;
use tractor::tree_mode::TreeMode;

use crate::input::filter::Filters;
use crate::input::Source;

use crate::cli::context::ExecCtx;

use super::{match_to_report_match, query_files_multi, check_expectation};

// ---------------------------------------------------------------------------
// Operation type
// ---------------------------------------------------------------------------

/// A test operation plan: run XPath queries and check match counts against expectations.
///
/// Multiple assertions can target the same set of sources — each source is
/// parsed once per assertion.
#[derive(Debug, Clone)]
pub struct TestOperationPlan {
    /// Pre-resolved unified input list.
    pub sources: Vec<Source>,
    /// Pre-built result filters.
    pub filters: Filters,
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

/// Pre-resolution shape for a test operation. Mirrors [`TestOperationPlan`]
/// but omits the input-resolution-derived fields (`sources`, `filters`).
/// Produced by the config parser and CLI layer, then turned into a
/// fully-resolved `TestOperationPlan` by the planner via
/// [`TestOperation::into_plan`].
#[derive(Debug, Clone)]
pub struct TestOperation {
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

impl TestOperation {
    /// Attach resolved inputs and produce the final executor-ready plan.
    pub fn into_plan(self, sources: Vec<Source>, filters: Filters) -> TestOperationPlan {
        TestOperationPlan {
            sources,
            filters,
            assertions: self.assertions,
            tree_mode: self.tree_mode,
            language: self.language,
            limit: self.limit,
            ignore_whitespace: self.ignore_whitespace,
            parse_depth: self.parse_depth,
        }
    }
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
    op: &TestOperationPlan,
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
    for assertion in &op.assertions {
        let matches = query_files_multi(
            &op.sources, &[assertion.xpath.as_str()], op.language.as_deref(),
            op.tree_mode, op.ignore_whitespace, op.parse_depth,
            op.limit, ctx.verbose, &op.filters,
        )?;
        if !check_expectation(&assertion.expect, matches.len())? {
            report.fail();
        }
        report.add_all(matches.into_iter().map(|m| match_to_report_match(m, "test")));
    }

    Ok(())
}
