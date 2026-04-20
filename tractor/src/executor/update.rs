//! Update operation: modify existing matched nodes without creating new structure.

use tractor::report::{ReportBuilder, ReportMatch};
use tractor::tree_mode::TreeMode;
use tractor::{apply_replacements, NormalizedPath};
use tractor::xpath_upsert::update_only;

use crate::input::filter::Filters;
use crate::input::Source;

use crate::cli::context::ExecCtx;

use super::{match_to_report_match, query_files_multi};

// ---------------------------------------------------------------------------
// Operation type
// ---------------------------------------------------------------------------

/// An update operation: modify existing matched nodes without creating new
/// structure. Unlike set, update fails if the XPath does not match any
/// existing nodes. Inline (virtual) sources are rejected at construction
/// time — update always mutates real files.
#[derive(Debug, Clone)]
pub struct UpdateOperation {
    /// Pre-resolved unified input list (disk-only for update).
    pub sources: Vec<Source>,
    /// Pre-built result filters (used by the fallback path).
    pub filters: Filters,
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
    ctx: &ExecCtx<'_>,
    report: &mut ReportBuilder,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut fallback_sources: Vec<Source> = Vec::new();

    for source in &op.sources {
        // update writes to disk, so a virtual source here is a construction
        // bug. Skip defensively rather than panic.
        if source.is_virtual() {
            continue;
        }
        let lang = op.language.as_deref().unwrap_or(&source.language);
        let file_path: &NormalizedPath = &source.path;
        let disk_bytes = std::fs::read_to_string(file_path)?;

        match update_only(&disk_bytes, lang, &op.xpath, &op.value, op.limit) {
            Ok(result) => {
                if result.source != disk_bytes {
                    std::fs::write(file_path, &result.source)?;
                    for m in &result.matches {
                        let mut rm = match_to_report_match(m.clone(), "update");
                        rm.status = Some("updated".to_string());
                        report.add(rm);
                    }
                }
            }
            Err(tractor::xpath_upsert::UpsertError::UnsupportedLanguage(_)) => {
                fallback_sources.push(source.clone());
            }
            Err(e) => return Err(e.into()),
        }
    }

    // Legacy fallback for languages without renderers
    if !fallback_sources.is_empty() {
        let matches = query_files_multi(
            &fallback_sources, &[op.xpath.as_str()], op.language.as_deref(),
            op.tree_mode, op.ignore_whitespace, op.parse_depth,
            None, ctx.verbose, &op.filters,
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
