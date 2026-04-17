//! Query operation: run XPath expressions against files, return matches.

use tractor::normalized_xpath::NormalizedXpath;
use tractor::report::ReportBuilder;
use tractor::tree_mode::TreeMode;

use crate::matcher::validate_xpath_diagnostic;
use crate::input::file_resolver::{FileResolver, FileRequest};
use crate::input::Source;

use super::{build_sources, ExecuteOptions, filter_refs, match_to_report_match, query_files_multi};

// ---------------------------------------------------------------------------
// Operation type
// ---------------------------------------------------------------------------

/// A query operation: run XPath expressions against files, return matches.
///
/// Supports two input modes:
/// - **Files**: set `files` (and optionally `exclude`). This is the default.
/// - **Inline source**: set `inline_source` and `language`. Files are ignored.
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
    /// Optional inline source (from stdin or `-s/--string`). Unified with
    /// resolved disk files inside the executor; the same `query_files_multi`
    /// handles both.
    pub inline_source: Option<Source>,
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
    resolver: &FileResolver,
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

    let inline_content_holder = op.inline_source.as_ref().and_then(|s| {
        if s.is_pathless() { None } else { s.inline_content().map(|c| (&s.path, c)) }
    });
    let request = FileRequest {
        files: &op.files,
        exclude: &op.exclude,
        diff_files: op.diff_files.as_deref(),
        diff_lines: op.diff_lines.as_deref(),
        command: "query",
        inline: inline_content_holder,
        has_inline: op.inline_source.is_some(),
    };
    let (files, filters) = resolver.resolve(&request, report);
    let sources = build_sources(files, op.inline_source.as_ref(), op.language.as_deref());

    if sources.is_empty() {
        return Ok(());
    }

    let xpaths: Vec<&str> = op.queries.iter().map(|q| q.xpath.as_str()).collect();

    let matches = query_files_multi(
        &sources, &xpaths, op.language.as_deref(),
        op.tree_mode, op.ignore_whitespace, op.parse_depth,
        op.limit, options.verbose, &filter_refs(&filters),
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
    use crate::executor::{Operation, ExecuteOptions, execute};

    fn temp_json_file(content: &str) -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.json");
        std::fs::write(&path, content).unwrap();
        (dir, path.to_str().unwrap().to_string())
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
        })];
        let report = run_query_ops(&ops);
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
        })];
        let report = run_query_ops(&ops);
        assert_eq!(report.all_matches().len(), 0);
        assert!(report.success.is_none());
    }
}
