//! Plain-text report renderer.
//!
//! Renders a `Report` as plain text by iterating matches and emitting selected
//! fields in the order declared by the ViewSet. No field labels — just values.
//!
//! Summary is always included for check/test reports; opt-in via `-v summary`
//! for query reports.

use tractor_core::{
    render_xml_node, normalize_path,
    render_source_precomputed, render_lines_precomputed,
    report::{Report, ReportKind, ReportMatch, Summary},
    RenderOptions,
};
use super::options::{ViewField, ViewSet};

pub fn render_text_report(report: &Report, view: &ViewSet, render_opts: &RenderOptions) -> String {
    let mut out = String::new();

    // Collect matches with optional group file — groups take priority
    let matches: Vec<(Option<&str>, &ReportMatch)> = if let Some(ref groups) = report.groups {
        groups.iter().flat_map(|g| g.matches.iter().map(move |rm| (Some(g.file.as_str()), rm))).collect()
    } else {
        report.matches.iter().map(|rm| (None, rm)).collect()
    };

    // Blank line between matches when a single match produces more than one output line.
    // File/line/column are combined onto one location line — they don't count individually.
    // In message-template mode all matches render as single lines — no separator.
    let message_mode = matches.first().map_or(false, |(_, rm)| rm.message.is_some());
    let has_location = view.has(ViewField::File) || view.has(ViewField::Line) || view.has(ViewField::Column);
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

    // Summary: always for check/test; gated on -v summary or -v query for query
    let show_summary = match report.kind {
        ReportKind::Query => view.has(ViewField::Summary) || view.has(ViewField::Query),
        ReportKind::Check | ReportKind::Test => true,
    };
    if show_summary {
        if let Some(ref summary) = report.summary {
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(&format_summary(summary, report.kind));
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
                    out.push_str(&render_lines_precomputed(
                        ls,
                        rm.tree.as_ref(),
                        rm.line, rm.end_line,
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
            // File/Line/Column handled above as combined location line
            // Summary/Count/Schema handled outside match loop
            _ => {}
        }
    }
}

fn format_summary(summary: &Summary, kind: ReportKind) -> String {
    let mut out = String::new();

    if let Some(ref query) = summary.query {
        out.push_str(&format!("Query: {}\n", query));
    }

    let count_line = match kind {
        ReportKind::Query => {
            let f = summary.files_affected;
            if f <= 1 {
                format!("{} matches\n", summary.total)
            } else {
                format!("{} matches in {} files\n", summary.total, f)
            }
        }
        ReportKind::Check => {
            if summary.passed {
                "All checks passed\n".to_string()
            } else if summary.errors > 0 {
                let f = summary.files_affected;
                format!("{} error{} in {} file{}\n",
                    summary.errors, if summary.errors == 1 { "" } else { "s" },
                    f, if f == 1 { "" } else { "s" })
            } else {
                let f = summary.files_affected;
                format!("{} warning{} in {} file{}\n",
                    summary.warnings, if summary.warnings == 1 { "" } else { "s" },
                    f, if f == 1 { "" } else { "s" })
            }
        }
        ReportKind::Test => {
            if summary.passed { "passed\n".to_string() } else { "failed\n".to_string() }
        }
    };
    out.push_str(&count_line);
    out
}
