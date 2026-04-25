//! Plain-text report renderer.
//!
//! Renders a `Report` as plain text by iterating matches and emitting selected
//! fields in the order declared by the ViewSet. No field labels — just values.
//!
//! Summary is always included for check/test reports; opt-in via `-v summary`
//! for query reports.

use std::collections::HashMap;

use tractor::{
    render_query_tree_node, render_query_tree_with_source, normalize_path,
    render_source_precomputed, render_lines, format_schema_tree,
    report::{Report, ReportMatch, ResultItem, Totals},
    RenderOptions,
};
use super::options::{ViewField, ViewSet};
use super::shared::{should_show_totals, render_fields_for_match};
use super::{Projection, ProjectionRenderError};

/// Text is human-readable — grouping affects display structure but matches
/// are rendered with inherited file context from groups, not field omission.
pub fn render_text_report(report: &Report, view: &ViewSet, render_opts: &RenderOptions, _dimensions: &[&str]) -> String {
    render_text_results(report, view, render_opts, true)
}

pub fn render_text_output(
    report: &Report,
    view: &ViewSet,
    render_opts: &RenderOptions,
    _dimensions: &[&str],
    projection: Projection,
    single: bool,
) -> Result<String, ProjectionRenderError> {
    match projection {
        Projection::Report => Ok(render_text_report(report, view, render_opts, &[])),
        Projection::Results => {
            if single {
                render_single_text_result(report, view, render_opts)
            } else {
                Ok(render_text_results(report, view, render_opts, false))
            }
        }
        Projection::Summary => Ok(render_text_summary(report)),
        Projection::Totals => Ok(render_text_totals(report)),
        Projection::Count => Ok(format!(
            "{}\n",
            report.totals.as_ref().map(|totals| totals.results).unwrap_or(0)
        )),
        Projection::Schema => Ok(render_text_schema(report, render_opts)),
        Projection::Tree | Projection::Value | Projection::Source | Projection::Lines => {
            render_text_field_projection(report, projection, render_opts, single)
        }
        Projection::Shape => {
            let shape_opts = render_opts.clone().with_shape_only(true);
            render_text_field_projection(report, Projection::Tree, &shape_opts, single)
        }
    }
}

fn render_text_results(
    report: &Report,
    view: &ViewSet,
    render_opts: &RenderOptions,
    include_summary: bool,
) -> String {
    let mut out = String::new();
    let mut source_cache: HashMap<String, Option<String>> = HashMap::new();

    // Set stdout mode: groups with captured outputs — render group by group.
    let has_group_output = view.has(ViewField::Output) && report.results.iter().any(|item| {
        matches!(item, ResultItem::Group(g) if !g.outputs.is_empty())
    });
    if has_group_output {
        out.push_str(&render_set_stdout_results(&report.results, view, render_opts));
        return out;
    }

    // Collect matches with optional group file context
    let matches: Vec<(Option<&str>, &ReportMatch)> = collect_matches_with_file(&report.results, None);

    // Blank line between matches when a single match produces more than one output line.
    // File/line/column are combined onto one location line — they don't count individually.
    // In message-template mode all matches render as single lines — no separator.
    let message_mode = matches.first().map_or(false, |(_, rm)| rm.message.is_some());
    let has_location = view.has(ViewField::File) || view.has(ViewField::Line) || view.has(ViewField::Column);
    // Status is now inline on the location line, so it doesn't produce an extra line
    let single_line_fields = [ViewField::Value, ViewField::Reason, ViewField::Severity]
        .iter().filter(|&&f| view.has(f)).count();
    let needs_separator = !message_mode && (
        view.has(ViewField::Tree)
        || view.has(ViewField::Lines)
        || view.has(ViewField::Source)
        || single_line_fields >= 2
        || (single_line_fields >= 1 && has_location)
    );

    for (i, (group_file, rm)) in matches.iter().enumerate() {
        if needs_separator && i > 0 {
            out.push('\n');
        }
        append_match(&mut out, rm, view, render_opts, *group_file, &mut source_cache);
    }

    if report.schema.is_some() {
        let schema = render_text_schema(report, render_opts);
        if !schema.is_empty() {
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(&schema);
        }
    }

    if include_summary && should_show_totals(report, view) {
        if let Some(ref totals) = report.totals {
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            if let Some(ref query) = report.query {
                out.push_str(&format!("Query: {}\n", query));
            }
            out.push_str(&format_summary(totals, report.success, report.expected.as_deref()));
        }
    }

    out
}

fn render_single_text_result(
    report: &Report,
    view: &ViewSet,
    render_opts: &RenderOptions,
) -> Result<String, ProjectionRenderError> {
    let mut out = String::new();
    let mut source_cache: HashMap<String, Option<String>> = HashMap::new();
    let matches = collect_matches_with_file(&report.results, None);
    let Some((group_file, rm)) = matches.into_iter().next() else {
        return Err(ProjectionRenderError::EmptySingle);
    };
    append_match(&mut out, rm, view, render_opts, group_file, &mut source_cache);
    Ok(out)
}

fn render_text_summary(report: &Report) -> String {
    let mut out = String::new();
    if let Some(ref query) = report.query {
        out.push_str(&format!("Query: {}\n", query));
    }
    if let Some(ref totals) = report.totals {
        out.push_str(&format_summary(totals, report.success, report.expected.as_deref()));
    }
    out
}

fn render_text_totals(report: &Report) -> String {
    let Some(ref totals) = report.totals else {
        return String::new();
    };

    let mut lines = vec![
        format!("results: {}", totals.results),
        format!("files: {}", totals.files),
    ];
    if totals.fatals > 0 {
        lines.push(format!("fatals: {}", totals.fatals));
    }
    if totals.errors > 0 {
        lines.push(format!("errors: {}", totals.errors));
    }
    if totals.warnings > 0 {
        lines.push(format!("warnings: {}", totals.warnings));
    }
    if totals.infos > 0 {
        lines.push(format!("infos: {}", totals.infos));
    }
    if totals.updated > 0 {
        lines.push(format!("updated: {}", totals.updated));
    }
    if totals.unchanged > 0 {
        lines.push(format!("unchanged: {}", totals.unchanged));
    }
    format!("{}\n", lines.join("\n"))
}

fn render_text_schema(report: &Report, render_opts: &RenderOptions) -> String {
    report
        .schema
        .as_ref()
        .map(|schema| format_schema_tree(schema, render_opts.max_depth.or(Some(4)), render_opts.use_color))
        .unwrap_or_default()
}

fn render_text_field_projection(
    report: &Report,
    projection: Projection,
    render_opts: &RenderOptions,
    single: bool,
) -> Result<String, ProjectionRenderError> {
    let projected: Vec<String> = report
        .all_matches()
        .into_iter()
        .filter_map(|rm| render_projected_field(rm, projection, render_opts))
        .collect();

    if single {
        projected
            .into_iter()
            .next()
            .ok_or(ProjectionRenderError::EmptySingle)
    } else {
        Ok(projected.concat())
    }
}

fn render_projected_field(
    rm: &ReportMatch,
    projection: Projection,
    render_opts: &RenderOptions,
) -> Option<String> {
    match projection {
        Projection::Tree => rm.tree.as_ref().map(|node| render_query_tree_node(node, render_opts)),
        Projection::Value => rm.value.as_ref().map(|value| format!("{value}\n")),
        Projection::Source => rm.source.as_ref().map(|source| {
            render_source_precomputed(
                source,
                rm.tree.as_ref(),
                rm.line,
                rm.column,
                rm.end_line,
                rm.end_column,
                render_opts,
            )
        }),
        Projection::Lines => rm.lines.as_ref().map(|lines| {
            render_lines(
                lines,
                rm.tree.as_ref(),
                rm.line,
                rm.column,
                rm.end_line,
                rm.end_column,
                render_opts,
            )
        }),
        _ => None,
    }
}

fn append_match(
    out: &mut String,
    rm: &ReportMatch,
    view: &ViewSet,
    render_opts: &RenderOptions,
    group_file: Option<&str>,
    source_cache: &mut HashMap<String, Option<String>>,
) {

    // When a message template was used, it is the intended primary output —
    // it replaces tree/value/etc in text format.
    if let Some(ref msg) = rm.message {
        out.push_str(msg);
        out.push('\n');
        return;
    }

    // Determine which fields to render: view-requested + diagnostic extras.
    let (view_fields, extra_fields) = render_fields_for_match(view, rm);

    // Location prefix: file, line, and/or column — combined on one line as file:line:col
    // Skip for file-less diagnostics (no meaningful location) and stdin input.
    let file = group_file.unwrap_or(&rm.file);
    let has_location = view.has(ViewField::File) || view.has(ViewField::Line) || view.has(ViewField::Column);
    let inline_status_reason = should_inline_status_reason(view, rm);
    if has_location && !file.is_empty() && !tractor::is_pathless_file(file) {
        let mut loc = String::new();
        if view.has(ViewField::File) {
            loc.push_str(&normalize_path(file));
        }
        if view.has(ViewField::Line) {
            if !loc.is_empty() { loc.push(':'); }
            loc.push_str(&rm.line.to_string());
        }
        if view.has(ViewField::Column) {
            loc.push(':');
            loc.push_str(&rm.column.to_string());
        }
        if view.has(ViewField::Status) {
            if let Some(ref status) = rm.status {
                loc.push_str(": ");
                loc.push_str(status);
                if inline_status_reason {
                    if let Some(ref reason) = rm.reason {
                        loc.push(' ');
                        loc.push_str(reason);
                    }
                }
            }
        }
        out.push_str(&loc);
        out.push('\n');
    }

    let combined_tree_source = if view.has(ViewField::Tree) && view.has(ViewField::Source) {
        render_combined_tree_source(rm, file, render_opts, source_cache)
    } else {
        None
    };
    if let Some(ref rendered) = combined_tree_source {
        out.push_str(rendered);
    }

    // All fields to stdout: view-requested first, then extras.
    // For text, Severity/Origin are rendered inline with Reason, and
    // Source is redundant when Lines is present — skip these as extras.
    // Severity/Origin are rendered inline with Reason — skip as standalone extras.
    let text_skip = |f: &ViewField| -> bool {
        (combined_tree_source.is_some() && matches!(f, ViewField::Tree | ViewField::Source))
            ||
        matches!(f, ViewField::Severity | ViewField::Origin)
            && !view.has(*f)
    };
    for field in view_fields.iter().chain(extra_fields.iter()) {
        if !text_skip(field) {
            render_field(out, field, rm, view, group_file, render_opts);
        }
    }

}

fn render_combined_tree_source(
    rm: &ReportMatch,
    file: &str,
    render_opts: &RenderOptions,
    source_cache: &mut HashMap<String, Option<String>>,
) -> Option<String> {
    let tree = rm.tree.as_ref()?;
    let source = load_source_for_match(rm, file, source_cache)?;
    render_query_tree_with_source(tree, &source, render_opts)
}

fn load_source_for_match(
    rm: &ReportMatch,
    file: &str,
    source_cache: &mut HashMap<String, Option<String>>,
) -> Option<String> {
    if !file.is_empty() && !tractor::is_pathless_file(file) {
        if let Some(cached) = source_cache.get(file) {
            return cached.clone();
        }

        let loaded = std::fs::read_to_string(file).ok();
        source_cache.insert(file.to_string(), loaded.clone());
        return loaded;
    }

    rm.source.clone().filter(|s| !s.is_empty())
}

/// Render a single field from a match into the output buffer.
fn render_field(
    out: &mut String,
    field: &ViewField,
    rm: &ReportMatch,
    view: &ViewSet,
    _group_file: Option<&str>,
    render_opts: &RenderOptions,
) {
    match field {
        ViewField::Tree => {
            if let Some(ref node) = rm.tree {
                out.push_str(&render_query_tree_node(node, render_opts));
            }
        }
        ViewField::Value => {
            if let Some(ref v) = rm.value {
                out.push_str(v);
                out.push('\n');
            }
        }
        ViewField::Source => {
            if let Some(ref s) = rm.source {
                out.push_str(&render_source_precomputed(
                    s,
                    rm.tree.as_ref(),
                    rm.line, rm.column, rm.end_line, rm.end_column,
                    render_opts,
                ));
            }
        }
        ViewField::Lines => {
            if let Some(ref ls) = rm.lines {
                out.push_str(&render_lines(
                    ls,
                    rm.tree.as_ref(),
                    rm.line, rm.column, rm.end_line, rm.end_column,
                    render_opts,
                ));
            }
        }
        ViewField::Reason => {
            if should_inline_status_reason(view, rm) {
                return;
            }
            if let Some(ref reason) = rm.reason {
                // Render as "severity(origin): reason" when available
                if let Some(severity) = rm.severity {
                    out.push_str(severity.as_str());
                    if let Some(origin) = rm.origin {
                        out.push('(');
                        out.push_str(origin.as_str());
                        out.push(')');
                    }
                    out.push_str(": ");
                }
                out.push_str(reason);
                out.push('\n');
            }
        }
        ViewField::Severity => {
            // Severity is rendered inline with reason; only standalone if reason absent.
            if rm.reason.is_none() {
                if let Some(severity) = rm.severity {
                    out.push_str(severity.as_str());
                    out.push('\n');
                }
            }
        }
        ViewField::Status => {
            let has_location = view.has(ViewField::File) || view.has(ViewField::Line) || view.has(ViewField::Column);
            if !has_location {
                if let Some(ref status) = rm.status {
                    out.push_str(status);
                    if should_inline_status_reason(view, rm) {
                        if let Some(ref reason) = rm.reason {
                            out.push(' ');
                            out.push_str(reason);
                        }
                    }
                    out.push('\n');
                }
            }
        }
        ViewField::Output => {
            if let Some(ref content) = rm.output {
                out.push_str(content);
            }
        }
        ViewField::Origin => {
            if rm.file.is_empty() {
                if let Some(origin) = rm.origin {
                    out.push_str(origin.as_str());
                    out.push('\n');
                }
            }
        }
        _ => {}
    }
}

/// Format summary text, deriving wording from totals fields and success.
/// No longer depends on ReportKind — the data itself determines the output.
fn format_summary(totals: &Totals, success: Option<bool>, expected: Option<&str>) -> String {
    let success_val = success.unwrap_or(true);
    let has_check = totals.fatals > 0 || totals.errors > 0 || totals.warnings > 0;
    let has_set = totals.updated > 0 || totals.unchanged > 0;
    let has_test = expected.is_some();
    let f = totals.files;

    // Test assertions
    if has_test && !has_check && !has_set {
        return if success_val { "passed\n".to_string() } else { "failed\n".to_string() };
    }

    // Check violations
    if has_check && !has_set {
        if success_val {
            return "All checks passed\n".to_string();
        }
        let mut parts = Vec::new();
        if totals.fatals > 0 {
            parts.push(format!("{} fatal{}", totals.fatals, if totals.fatals == 1 { "" } else { "s" }));
        }
        if totals.errors > 0 {
            parts.push(format!("{} error{}", totals.errors, if totals.errors == 1 { "" } else { "s" }));
        }
        if totals.warnings > 0 {
            parts.push(format!("{} warning{}", totals.warnings, if totals.warnings == 1 { "" } else { "s" }));
        }
        return format!("{} in {} file{}\n", parts.join(", "), f, if f == 1 { "" } else { "s" });
    }

    // Set operations
    if has_set && !has_check {
        let updated = totals.updated;
        let unchanged = totals.unchanged;
        if updated == 0 && unchanged == 0 {
            return "No matches\n".to_string();
        } else if unchanged == 0 {
            return format!("Set {} match{} in {} file{}\n",
                updated, if updated == 1 { "" } else { "es" },
                f, if f == 1 { "" } else { "s" });
        } else {
            return format!("Set {} match{} in {} file{} ({} unchanged)\n",
                updated, if updated == 1 { "" } else { "es" },
                f, if f == 1 { "" } else { "s" },
                unchanged);
        }
    }

    // Mixed or pure query — generic summary
    if success.is_none() {
        // Query (no verdict)
        if f <= 1 {
            return format!("{} matches\n", totals.results);
        }
        return format!("{} matches in {} files\n", totals.results, f);
    }

    // Mixed operations or generic verdict
    if success_val {
        return format!("{} matches across {} files\n", totals.results, f);
    }
    let mut parts = Vec::new();
    if totals.fatals > 0 {
        parts.push(format!("{} fatal{}", totals.fatals, if totals.fatals == 1 { "" } else { "s" }));
    }
    if totals.errors > 0 {
        parts.push(format!("{} error{}", totals.errors, if totals.errors == 1 { "" } else { "s" }));
    }
    if totals.warnings > 0 {
        parts.push(format!("{} warning{}", totals.warnings, if totals.warnings == 1 { "" } else { "s" }));
    }
    if totals.updated > 0 {
        parts.push(format!("{} updated", totals.updated));
    }
    if parts.is_empty() {
        "failed\n".to_string()
    } else {
        format!("{}\n", parts.join(", "))
    }
}

// ---------------------------------------------------------------------------
// ResultItem helpers for text rendering
// ---------------------------------------------------------------------------

/// Collect leaf matches with their inherited file context from the results tree.
fn collect_matches_with_file<'a>(
    items: &'a [ResultItem],
    parent_file: Option<&'a str>,
) -> Vec<(Option<&'a str>, &'a ReportMatch)> {
    let mut out = Vec::new();
    for item in items {
        match item {
            ResultItem::Match(rm) => out.push((parent_file, rm)),
            ResultItem::Group(g) => {
                let file = g.file.as_deref().or(parent_file);
                out.extend(collect_matches_with_file(&g.results, file));
            }
        }
    }
    out
}

fn should_inline_status_reason(view: &ViewSet, rm: &ReportMatch) -> bool {
    view.has(ViewField::Status)
        && view.has(ViewField::Reason)
        && rm.status.is_some()
        && rm.reason.is_some()
        && rm.severity.is_none()
}

/// Render set stdout mode from results tree (groups with captured outputs).
fn render_set_stdout_results(items: &[ResultItem], view: &ViewSet, render_opts: &RenderOptions) -> String {
    let mut out = String::new();
    let mut source_cache: HashMap<String, Option<String>> = HashMap::new();
    let has_location = view.has(ViewField::File) || view.has(ViewField::Line) || view.has(ViewField::Column);
    let has_per_match = has_location || view.has(ViewField::Status);

    for item in items {
        if let ResultItem::Group(g) = item {
            let file = g.file.as_deref();
            if has_per_match {
                // Render leaf matches within this group
                for child in &g.results {
                    if let ResultItem::Match(rm) = child {
                        append_match(&mut out, rm, view, render_opts, file, &mut source_cache);
                    }
                }
            }
            // Group-level captured outputs (full modified file content)
            for captured in &g.outputs {
                out.push_str(&captured.content);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::render_text_report;
    use crate::format::{ViewField, ViewSet};
    use tractor::report::{Report, ReportMatch, ResultItem, Totals};
    use tractor::RenderOptions;

    #[test]
    fn render_text_inlines_status_and_reason_when_both_are_selected() {
        let report = Report {
            success: Some(true),
            totals: Some(Totals {
                results: 1,
                files: 1,
                fatals: 0,
                errors: 0,
                warnings: 0,
                infos: 0,
                updated: 1,
                unchanged: 0,
            }),
            expected: None,
            query: None,
            schema: None,
            outputs: vec![],
            results: vec![ResultItem::Match(ReportMatch {
                file: "app-config.json".to_string(),
                line: 3,
                column: 13,
                end_line: 3,
                end_column: 25,
                command: "set".to_string(),
                tree: None,
                value: Some("db.prod.internal".to_string()),
                source: None,
                lines: None,
                reason: Some("//database/host".to_string()),
                severity: None,
                message: None,
                origin: None,
                rule_id: None,
                status: Some("updated".to_string()),
                output: None,
            })],
            group: None,
            file: None,
            command: None,
            rule_id: None,
        };

        let view = ViewSet::new(vec![ViewField::File, ViewField::Line, ViewField::Status, ViewField::Reason]);
        let rendered = render_text_report(&report, &view, &RenderOptions::default(), &[]);

        assert_eq!(
            rendered,
            "app-config.json:3: updated //database/host\nSet 1 match in 1 file\n"
        );
    }
}
