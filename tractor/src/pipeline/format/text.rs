//! Plain-text report renderer.
//!
//! Renders a `Report` as plain text by iterating matches and emitting selected
//! fields in a fixed canonical order. No field labels — just values.
//!
//! Summary is always included for check/test reports; opt-in via `-v summary`
//! for query reports.

use tractor_core::{
    TextViewMode, format_matches, normalize_path,
    output::OutputOptions,
    report::{Report, ReportKind, ReportMatch, Summary},
    RenderOptions,
};
use super::options::{ViewField, ViewSet};

pub fn render_text_report(report: &Report, view: &ViewSet, render_opts: &RenderOptions) -> String {
    let mut out = String::new();

    // Collect matches — groups take priority (with_groups() drains flat list)
    let matches: Vec<&ReportMatch> = if let Some(ref groups) = report.groups {
        groups.iter().flat_map(|g| g.matches.iter()).collect()
    } else {
        report.matches.iter().collect()
    };

    // Blank line between matches when a single match produces more than one output line.
    // File/line/column are combined onto one location line — they don't count individually.
    // In message-template mode all matches render as single lines — no separator.
    let message_mode = matches.first().map_or(false, |rm| rm.message.is_some());
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

    for (i, rm) in matches.iter().enumerate() {
        if needs_separator && i > 0 {
            out.push('\n');
        }
        append_match(&mut out, rm, view, render_opts);
    }

    // Summary: always for check/test; gated on -v summary for query
    let show_summary = match report.kind {
        ReportKind::Query => view.has(ViewField::Summary),
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

fn append_match(out: &mut String, rm: &ReportMatch, view: &ViewSet, render_opts: &RenderOptions) {
    let opts = to_output_opts(render_opts);
    let single = &[rm.inner.clone()];

    // When a message template was used, it is the intended primary output —
    // it replaces tree/value/etc in text format.  Use -f json/xml if you want
    // both the message and structured fields.
    if let Some(ref msg) = rm.message {
        out.push_str(msg);
        out.push('\n');
        return;
    }

    // Location prefix: file, line, and/or column — combined on one line as file:line:col
    if view.has(ViewField::File) || view.has(ViewField::Line) || view.has(ViewField::Column) {
        let mut loc = String::new();
        if view.has(ViewField::File) {
            loc.push_str(&normalize_path(&rm.inner.file));
        }
        if view.has(ViewField::Line) {
            if !loc.is_empty() { loc.push(':'); }
            loc.push_str(&rm.inner.line.to_string());
        }
        if view.has(ViewField::Column) {
            loc.push(':');
            loc.push_str(&rm.inner.column.to_string());
        }
        out.push_str(&loc);
        out.push('\n');
    }

    // Canonical field order
    if view.has(ViewField::Tree) {
        out.push_str(&format_matches(single, TextViewMode::Xml, &opts));
    }
    if view.has(ViewField::Value) {
        out.push_str(&rm.inner.value);
        out.push('\n');
    }
    if view.has(ViewField::Source) {
        out.push_str(&format_matches(single, TextViewMode::Source, &opts));
    }
    if view.has(ViewField::Lines) {
        out.push_str(&format_matches(single, TextViewMode::Lines, &opts));
    }
    if view.has(ViewField::Reason) {
        if let Some(ref reason) = rm.reason {
            out.push_str(reason);
            out.push('\n');
        }
    }
    if view.has(ViewField::Severity) {
        if let Some(severity) = rm.severity {
            out.push_str(severity.as_str());
            out.push('\n');
        }
    }
}

fn to_output_opts(render_opts: &RenderOptions) -> OutputOptions {
    OutputOptions {
        message: None,
        use_color: render_opts.use_color,
        strip_locations: !render_opts.include_locations,
        max_depth: render_opts.max_depth,
        pretty_print: render_opts.pretty_print,
        language: render_opts.language.clone(),
        warning: false,
    }
}

fn format_summary(summary: &Summary, kind: ReportKind) -> String {
    match kind {
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
    }
}
