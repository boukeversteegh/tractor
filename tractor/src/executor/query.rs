//! Query operation: run XPath expressions against sources, return matches.

use tractor::normalized_xpath::NormalizedXpath;
use tractor::report::ReportBuilder;
use tractor::tree_mode::TreeMode;

use crate::matcher::validate_xpath_diagnostic;
use crate::input::filter::ResultFilter;
use crate::input::Source;

use super::{ExecuteOptions, filter_refs, match_to_report_match, query_files_multi};

// ---------------------------------------------------------------------------
// Operation type
// ---------------------------------------------------------------------------

/// A query operation: run XPath expressions against sources, return matches.
///
/// Disk and inline inputs are already unified into `sources` at construction
/// time — the executor does not branch on input kind.
///
/// Multiple queries can target the same set of sources — each source is parsed
/// once and all XPath expressions are evaluated against it.
pub struct QueryOperation {
    /// Pre-resolved unified input list.
    pub sources: Vec<Source>,
    /// Pre-built result filters (diff-lines, etc.).
    pub filters: Vec<Box<dyn ResultFilter>>,
    /// XPath queries to evaluate.
    pub queries: Vec<QueryExpr>,
    /// Tree mode override for parsing.
    pub tree_mode: Option<TreeMode>,
    /// Language override applied during parsing (per-op, overrides source's own).
    pub language: Option<String>,
    /// Maximum number of matches to return (across all queries).
    pub limit: Option<usize>,
    /// Ignore whitespace-only text nodes during parsing.
    pub ignore_whitespace: bool,
    /// Maximum parse depth.
    pub parse_depth: Option<usize>,
}

/// A single XPath query expression.
#[derive(Debug, Clone)]
pub struct QueryExpr {
    /// XPath expression to evaluate.
    pub xpath: NormalizedXpath,
}

// ---------------------------------------------------------------------------
// Execution
// ---------------------------------------------------------------------------

pub(crate) fn execute_query(
    op: &QueryOperation,
    options: &ExecuteOptions,
    report: &mut ReportBuilder,
) -> Result<(), Box<dyn std::error::Error>> {
    // Validate all XPath expressions upfront — add fatal diagnostics on failure
    let diagnostics: Vec<_> = op.queries.iter()
        .filter_map(|q| validate_xpath_diagnostic(&q.xpath, "query"))
        .collect();
    if !diagnostics.is_empty() {
        report.add_all(diagnostics);
        return Ok(());
    }

    if op.sources.is_empty() {
        return Ok(());
    }

    let xpaths: Vec<&str> = op.queries.iter().map(|q| q.xpath.as_str()).collect();

    let matches = query_files_multi(
        &op.sources, &xpaths, op.language.as_deref(),
        op.tree_mode, op.ignore_whitespace, op.parse_depth,
        op.limit, options.verbose, &filter_refs(&op.filters),
    )?;

    report.add_all(matches.into_iter().map(|m| match_to_report_match(m, "query")));

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tractor::report::ReportBuilder;
    use tractor::NormalizedPath;
    use crate::executor::{Operation, ExecuteOptions, execute};

    fn temp_json_file(content: &str) -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.json");
        std::fs::write(&path, content).unwrap();
        (dir, path.to_str().unwrap().to_string())
    }

    fn disk_source(path: &str) -> Source {
        let np = NormalizedPath::absolute(path);
        let lang = tractor::detect_language(np.as_str()).to_string();
        Source::disk(np, lang)
    }

    fn run_query_ops(ops: &[Operation]) -> tractor::report::Report {
        let mut builder = ReportBuilder::new();
        builder.set_no_verdict();
        execute(ops, &ExecuteOptions::default(), &mut builder).unwrap();
        builder.build()
    }

    #[test]
    fn query_returns_matches() {
        let (_dir, path) = temp_json_file(r#"{"name": "alice", "age": 30}"#);
        let ops = vec![Operation::Query(QueryOperation {
            sources: vec![disk_source(&path)],
            filters: vec![],
            queries: vec![QueryExpr { xpath: "//name".into() }],
            tree_mode: None,
            language: None,
            limit: None,
            ignore_whitespace: false,
            parse_depth: None,
        })];
        let report = run_query_ops(&ops);
        assert!(report.success.is_none());
        assert_eq!(report.all_matches().len(), 1);
        assert_eq!(report.all_matches()[0].value.as_deref(), Some("alice"));
    }

    #[test]
    fn query_with_limit() {
        let (_dir, path) = temp_json_file(r#"{"a": 1, "b": 2, "c": 3}"#);
        let ops = vec![Operation::Query(QueryOperation {
            sources: vec![disk_source(&path)],
            filters: vec![],
            queries: vec![QueryExpr { xpath: "//*[number(.) > 0]".into() }],
            tree_mode: None,
            language: None,
            limit: Some(2),
            ignore_whitespace: false,
            parse_depth: None,
        })];
        let report = run_query_ops(&ops);
        assert!(report.all_matches().len() <= 2);
    }

    #[test]
    fn query_empty_sources() {
        let ops = vec![Operation::Query(QueryOperation {
            sources: vec![],
            filters: vec![],
            queries: vec![QueryExpr { xpath: "//x".into() }],
            tree_mode: None,
            language: None,
            limit: None,
            ignore_whitespace: false,
            parse_depth: None,
        })];
        let report = run_query_ops(&ops);
        assert_eq!(report.all_matches().len(), 0);
        assert!(report.success.is_none());
    }
}
