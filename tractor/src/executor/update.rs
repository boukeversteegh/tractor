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

/// An update operation plan: modify existing matched nodes without creating
/// new structure. Unlike set, update fails if the XPath does not match any
/// existing nodes. Inline (virtual) sources are rejected at construction
/// time — update always mutates real files.
#[derive(Debug, Clone)]
pub struct UpdateOperationPlan {
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
    /// What `xpath` reads from the run-level `$variables`, recorded when the
    /// plan is built.
    ///
    /// `update` has no config entries, so nothing else records its reads —
    /// and an operation whose reads are unrecorded reads *nothing*, which
    /// would silently omit any key it looks up. Deriving it here keeps the
    /// operation inside the same mechanism as entries, with neither a
    /// special case at execution nor a hole.
    pub reads: tractor::NamespaceReads,
}

/// Pre-resolution shape for an update operation. Mirrors [`UpdateOperationPlan`]
/// but omits the input-resolution-derived fields (`sources`, `filters`).
/// Produced by the CLI layer (update has no config form), then turned into
/// a fully-resolved `UpdateOperationPlan` by the planner via
/// [`UpdateOperation::into_plan`].
#[derive(Debug, Clone)]
pub struct UpdateOperation {
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

impl UpdateOperation {
    /// Attach resolved inputs and produce the final executor-ready plan.
    pub fn into_plan(self, sources: Vec<Source>, filters: Filters) -> UpdateOperationPlan {
        UpdateOperationPlan {
            sources,
            filters,
            // Derived from the expression that will run, at the one place a
            // plan is built — the same rule the entry constructors follow.
            reads: tractor::NamespaceReads::of(&self.xpath, "variables"),
            xpath: self.xpath,
            value: self.value,
            tree_mode: self.tree_mode,
            language: self.language,
            limit: self.limit,
            ignore_whitespace: self.ignore_whitespace,
            parse_depth: self.parse_depth,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An update's reads are recorded when its plan is built, so the
    /// run-level union covers it like any entry-bearing operation. Without
    /// this, an update reading `$variables?x` would have empty reads and
    /// the key would be silently omitted from its bindings.
    #[test]
    fn into_plan_records_what_the_xpath_reads() {
        let op = UpdateOperation {
            xpath: "//port[. = $variables?from]".to_string(),
            value: "1".to_string(),
            tree_mode: None,
            language: None,
            limit: None,
            ignore_whitespace: false,
            parse_depth: None,
        };
        let plan = op.into_plan(Vec::new(), Filters::default());
        assert!(plan.reads.reads("from"), "the looked-up key must be recorded");
        assert!(!plan.reads.reads("other"));

        let op = UpdateOperation {
            xpath: "//port".to_string(),
            value: "1".to_string(),
            tree_mode: None,
            language: None,
            limit: None,
            ignore_whitespace: false,
            parse_depth: None,
        };
        let plan = op.into_plan(Vec::new(), Filters::default());
        assert!(!plan.reads.reads_any(), "an xpath reading nothing records nothing");
    }
}

// ---------------------------------------------------------------------------
// Execution
// ---------------------------------------------------------------------------

pub(crate) fn execute_update(
    op: &UpdateOperationPlan,
    ctx: &ExecCtx<'_>,
    report: &mut ReportBuilder,
) -> Result<(), Box<dyn std::error::Error>> {
    let variables = ctx.query_variables();

    // Advisory: update has no config entry, so every `$<entry>.variables` namespace is
    // an empty map here — the advisory says so rather than leaving it silent.
    let bindings = tractor::QueryBindings::run(std::sync::Arc::clone(&variables));
    report.add_all(crate::matcher::variable_diagnostics("update", &bindings, 0, &op.xpath));

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

        match update_only(&disk_bytes, lang, &op.xpath, &op.value, op.limit, bindings.clone()) {
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
            &fallback_sources, &[(op.xpath.as_str(), bindings.clone())], op.language.as_deref(),
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
