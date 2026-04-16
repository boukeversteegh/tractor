//! Update operation: modify existing matched nodes without creating new structure.

use tractor::report::{ReportBuilder, ReportMatch};
use tractor::tree_mode::TreeMode;
use tractor::{detect_language, apply_replacements, NormalizedPath};
use tractor::xpath_upsert::update_only;

use crate::input::file_resolver::{FileResolver, FileRequest};

use super::{ExecuteOptions, filter_refs, match_to_report_match, query_files_multi};

// ---------------------------------------------------------------------------
// Operation type
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Execution
// ---------------------------------------------------------------------------

pub(crate) fn execute_update(
    op: &UpdateOperation,
    options: &ExecuteOptions,
    resolver: &FileResolver,
    report: &mut ReportBuilder,
) -> Result<(), Box<dyn std::error::Error>> {
    let request = FileRequest {
        files: &op.files,
        exclude: &op.exclude,
        diff_files: op.diff_files.as_deref(),
        diff_lines: op.diff_lines.as_deref(),
        command: "update",
    };
    let (files, filters) = resolver.resolve(&request, report);
    let mut fallback_files: Vec<NormalizedPath> = Vec::new();

    for file_path in &files {
        let lang = op.language.as_deref()
            .unwrap_or_else(|| detect_language(file_path.as_str()));
        let source = std::fs::read_to_string(file_path)?;

        match update_only(&source, lang, &op.xpath, &op.value, op.limit) {
            Ok(result) => {
                if result.source != source {
                    std::fs::write(file_path, &result.source)?;
                    for m in &result.matches {
                        let mut rm = match_to_report_match(m.clone(), "update");
                        rm.status = Some("updated".to_string());
                        report.add(rm);
                    }
                }
            }
            Err(tractor::xpath_upsert::UpsertError::UnsupportedLanguage(_)) => {
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
