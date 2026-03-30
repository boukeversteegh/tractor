//! Plain-text report renderer.
//!
//! Renders a `Report` as plain text by iterating matches and emitting selected
//! fields in the order declared by the ViewSet. No field labels — just values.
//!
//! Summary is always included for check/test reports; opt-in via `-v summary`
//! for query reports.

use tractor_core::{
    render_xml_node, normalize_path,
    render_source_precomputed, render_lines,
    report::{Report, ReportMatch, ResultItem, Totals},
    RenderOptions,
};
use super::options::{ViewField, ViewSet};

pub fn render_text_report(report: &Report, view: &ViewSet, render_opts: &RenderOptions) -> String {
    let mut out = String::new();

    // Set stdout mode: groups with output_content — render group by group.
    let has_group_output = view.has(ViewField::Output) && report.results.iter().any(|item| {
        matches!(item, ResultItem::Group(g) if g.output_content.is_some())
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
        append_match(&mut out, rm, view, render_opts, *group_file);
    }

    // Summary: always for check/test/set (unless output view active, handled above);
    // gated on -v summary or -v query for query
    // Show summary: always when report has a verdict, opt-in for queries
    let show_summary = if report.success.is_some() {
        true
    } else {
        view.has(ViewField::Totals) || view.has(ViewField::Query)
    };
    if show_summary {
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

fn append_match(out: &mut String, rm: &ReportMatch, view: &ViewSet, render_opts: &RenderOptions, group_file: Option<&str>) {

    // When a message template was used, it is the intended primary output —
    // it replaces tree/value/etc in text format.
    if let Some(ref msg) = rm.message {
        out.push_str(msg);
        out.push('\n');
        return;
    }

    // Location prefix: file, line, and/or column — combined on one line as file:line:col
    // Status is appended inline when both a location and a status are present (GCC-style).
    if view.has(ViewField::File) || view.has(ViewField::Line) || view.has(ViewField::Column) {
        let mut loc = String::new();
        if view.has(ViewField::File) {
            let file = group_file.unwrap_or(&rm.file);
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
        // Append status inline on the location line (GCC-style: file:line: status)
        if view.has(ViewField::Status) {
            if let Some(ref status) = rm.status {
                loc.push_str(": ");
                loc.push_str(status);
            }
        }
        out.push_str(&loc);
        out.push('\n');
    }

    // Content fields — iterate ViewSet for declaration order
    for field in &view.fields {
        match field {
            ViewField::Tree => {
                if let Some(ref node) = rm.tree {
                    let rendered = render_xml_node(node, render_opts);
                    if render_opts.pretty_print && !rendered.ends_with('\n') {
                        out.push_str(&rendered);
                        out.push('\n');
                    } else {
                        out.push_str(&rendered);
                    }
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
                if let Some(ref reason) = rm.reason {
                    out.push_str(reason);
                    out.push('\n');
                }
            }
            ViewField::Severity => {
                if let Some(severity) = rm.severity {
                    out.push_str(severity.as_str());
                    out.push('\n');
                }
            }
            ViewField::Status => {
                // Status is printed inline on the location line when a location is present.
                // Only print it as a standalone line if no location fields are in view.
                let has_location = view.has(ViewField::File) || view.has(ViewField::Line) || view.has(ViewField::Column);
                if !has_location {
                    if let Some(ref status) = rm.status {
                        out.push_str(status);
                        out.push('\n');
                    }
                }
            }
            ViewField::Output => {
                // Output is at group level for set reports; nothing to print here.
                // (If a match has output directly, print it as a fallback.)
                if let Some(ref content) = rm.output {
                    out.push_str(content);
                }
            }
            // File/Line/Column handled above as combined location line
            // Summary/Count/Schema handled outside match loop
            _ => {}
        }
    }
}

/// Format summary text, deriving wording from totals fields and success.
/// No longer depends on ReportKind — the data itself determines the output.
fn format_summary(totals: &Totals, success: Option<bool>, expected: Option<&str>) -> String {
    let success_val = success.unwrap_or(true);
    let has_check = totals.errors > 0 || totals.warnings > 0;
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
fn collect_matches_with_file<'a>(items: &'a [ResultItem], parent_file: Option<&'a str>) -> Vec<(Option<&'a str>, &'a ReportMatch)> {
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

/// Render set stdout mode from results tree (groups with output_content).
fn render_set_stdout_results(items: &[ResultItem], view: &ViewSet, render_opts: &RenderOptions) -> String {
    let mut out = String::new();
    let has_location = view.has(ViewField::File) || view.has(ViewField::Line) || view.has(ViewField::Column);
    let has_per_match = has_location || view.has(ViewField::Status);

    for item in items {
        if let ResultItem::Group(g) = item {
            let file = g.file.as_deref();
            if has_per_match {
                // Render leaf matches within this group
                for child in &g.results {
                    if let ResultItem::Match(rm) = child {
                        append_match(&mut out, rm, view, render_opts, file);
                    }
                }
            }
            // Group-level output (the full modified file content)
            if let Some(ref content) = g.output_content {
                out.push_str(content);
            }
        }
    }
    out
}
