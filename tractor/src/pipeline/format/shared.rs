//! Shared helpers used across multiple format renderers.

use std::path::Path;
use tractor_core::normalize_path;
use tractor_core::report::ReportMatch;
use super::options::{ViewField, ViewSet};

pub fn to_absolute_path(path: &str) -> String {
    let p = Path::new(path);
    let absolute = if p.is_absolute() {
        p.to_path_buf()
    } else if let Ok(cwd) = std::env::current_dir() {
        cwd.join(p)
    } else {
        p.to_path_buf()
    };
    normalize_path(&absolute.to_string_lossy())
}

/// Whether totals/metadata should be shown for this report.
/// Always shown when the report has a verdict (success is Some).
/// For query reports (no verdict), only shown if explicitly requested via view.
pub fn should_show_totals(report: &tractor_core::report::Report, view: &ViewSet) -> bool {
    if report.success.is_some() {
        true
    } else {
        view.has(ViewField::Totals) || view.has(ViewField::Query)
    }
}

/// Determine whether a field should be rendered for this match.
/// A field is shown when:
///   - it is selected in the view (for view-gated fields)
///   - it is not hoisted to a parent group (not in skip_dims)
///   - the match actually has a value for it
pub fn should_emit_file(rm: &ReportMatch, skip_dims: &[&str]) -> bool {
    !skip_dims.contains(&"file") && !rm.file.is_empty()
}

pub fn should_emit_command(rm: &ReportMatch, view: &ViewSet, skip_dims: &[&str]) -> bool {
    view.has(ViewField::Command) && !skip_dims.contains(&"command") && !rm.command.is_empty()
}

pub fn should_emit_rule_id(rm: &ReportMatch, skip_dims: &[&str]) -> bool {
    !skip_dims.contains(&"rule_id") && rm.rule_id.is_some()
}

/// Check if a match has non-None data for a given view field.
pub fn match_has_field(rm: &ReportMatch, field: ViewField) -> bool {
    match field {
        ViewField::Tree => rm.tree.is_some(),
        ViewField::Value => rm.value.is_some(),
        ViewField::Source => rm.source.is_some(),
        ViewField::Lines => rm.lines.is_some(),
        ViewField::Reason => rm.reason.is_some(),
        ViewField::Severity => rm.severity.is_some(),
        ViewField::Status => rm.status.is_some(),
        ViewField::Origin => rm.origin.is_some(),
        ViewField::Output => rm.output.is_some(),
        _ => false,
    }
}

/// Compute the list of fields to render for a match.
///
/// Returns view-requested fields first (in user order), then any extra
/// diagnostic fields that are present on the match but not in the view.
/// This ensures diagnostics are always visible regardless of -v settings,
/// while respecting the user's field ordering for requested fields.
///
/// The extra diagnostic fields are: Reason, Lines. These are the essential
/// fields for understanding what went wrong. Reason renders severity+origin
/// inline when present, so those don't need separate entries.
pub fn render_fields_for_match(view: &ViewSet, rm: &ReportMatch) -> (Vec<ViewField>, Vec<ViewField>) {
    let view_fields = view.fields.clone();

    let diagnostic_extras: &[ViewField] = &[
        ViewField::Severity, ViewField::Reason, ViewField::Origin,
        ViewField::Lines,
    ];
    let extra: Vec<ViewField> = diagnostic_extras.iter()
        .filter(|&&f| !view.has(f) && match_has_field(rm, f))
        .copied()
        .collect();

    (view_fields, extra)
}
