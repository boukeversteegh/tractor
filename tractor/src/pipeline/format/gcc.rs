use tractor_core::{render_lines, report::{Report, ReportMatch, ResultItem}, RenderOptions};
use super::shared::to_absolute_path;

/// Render report matches in gcc format: `file:line:col: severity: reason`
/// GCC is a flat per-line format — grouping affects match ordering only,
/// not field omission. Every match includes all fields.
pub fn render_gcc(report: &Report, opts: &RenderOptions, _dimensions: &[&str]) -> String {
    let mut out = String::new();
    render_gcc_results(&mut out, &report.results, None, opts);
    out
}

/// Walk the results tree recursively, rendering matches in gcc format.
fn render_gcc_results(out: &mut String, items: &[ResultItem], parent_file: Option<&str>, opts: &RenderOptions) {
    for item in items {
        match item {
            ResultItem::Match(rm) => {
                render_gcc_match(out, rm, parent_file, opts);
            }
            ResultItem::Group(g) => {
                let file = g.file.as_deref().or(parent_file);
                render_gcc_results(out, &g.results, file, opts);
            }
        }
    }
}

fn render_gcc_match(out: &mut String, rm: &ReportMatch, group_file: Option<&str>, opts: &RenderOptions) {
    let file = group_file.unwrap_or(&rm.file);

    // Diagnostics (fatal/error) always render as severity: reason, regardless of command.
    if rm.severity.map_or(false, |s| matches!(s,
        tractor_core::report::Severity::Fatal | tractor_core::report::Severity::Error
    )) {
        let reason   = rm.reason.as_deref().unwrap_or("error");
        let severity = rm.severity.map_or("error", |s| match s {
            tractor_core::report::Severity::Fatal => "error",
            tractor_core::report::Severity::Error => "error",
            tractor_core::report::Severity::Warning => "warning",
            tractor_core::report::Severity::Info => "note",
        });
        if file.is_empty() {
            let prefix = rm.origin.map_or("tractor", |o| o.as_str());
            out.push_str(&format!("{}: {}: {}\n", prefix, severity, reason));
        } else {
            out.push_str(&format!(
                "{}:{}:{}: {}: {}\n",
                to_absolute_path(file), rm.line, rm.column, severity, reason
            ));
        }
        if let Some(ref hint) = rm.hint {
            out.push_str(&format!("  note: {}\n", hint));
        }
        if let Some(ref ls) = rm.lines {
            out.push_str(&render_lines(
                ls, rm.tree.as_ref(),
                rm.line, rm.column, rm.end_line, rm.end_column,
                opts,
            ));
        }
        return;
    }

    match rm.command.as_str() {
        "set" => {
            // Set matches: file: status
            let status = rm.status.as_deref().unwrap_or("unknown");
            out.push_str(&format!("{}: {}\n", to_absolute_path(file), status));
        }
        "query" => {
            // Query matches: file:line:col: note: value
            let value = rm.value.as_deref().unwrap_or("");
            out.push_str(&format!(
                "{}:{}:{}: note: {}\n",
                to_absolute_path(file), rm.line, rm.column, value
            ));
        }
        _ => {
            // Check and other matches: file:line:col: severity: reason
            let reason   = rm.reason.as_deref().unwrap_or("violation");
            // Map to gcc-compatible severity labels (Fatal→error, Info→note)
            let severity = rm.severity.map_or("error", |s| match s {
                tractor_core::report::Severity::Fatal => "error",
                tractor_core::report::Severity::Error => "error",
                tractor_core::report::Severity::Warning => "warning",
                tractor_core::report::Severity::Info => "note",
            });
            if file.is_empty() {
                // No file — use origin or tool name as prefix (like gcc's "cc1: error: ...")
                let prefix = rm.origin.map_or("tractor", |o| o.as_str());
                out.push_str(&format!("{}: {}: {}\n", prefix, severity, reason));
            } else {
                out.push_str(&format!(
                    "{}:{}:{}: {}: {}\n",
                    to_absolute_path(file), rm.line, rm.column, severity, reason
                ));
            }
            if let Some(ref hint) = rm.hint {
                out.push_str(&format!("  note: {}\n", hint));
            }
            if let Some(ref ls) = rm.lines {
                out.push_str(&render_lines(
                    ls, rm.tree.as_ref(),
                    rm.line, rm.column, rm.end_line, rm.end_column,
                    opts,
                ));
            }
        }
    }
}


/// Render ReportMatches in gcc format using a message template (for `test --error`).
pub fn render_gcc_report_with_template(matches: &[ReportMatch], template: &str, is_warning: bool, opts: &RenderOptions) -> String {
    let severity = if is_warning { "warning" } else { "error" };
    let mut out = String::new();
    for rm in matches {
        let msg = template
            .replace("{file}", &tractor_core::normalize_path(&rm.file))
            .replace("{line}", &rm.line.to_string())
            .replace("{col}", &rm.column.to_string())
            .replace("{value}", rm.value.as_deref().unwrap_or(""));
        out.push_str(&format!(
            "{}:{}:{}: {}: {}\n",
            to_absolute_path(&rm.file), rm.line, rm.column, severity, msg
        ));
        if let Some(ref ls) = rm.lines {
            out.push_str(&render_lines(
                ls, rm.tree.as_ref(),
                rm.line, rm.column, rm.end_line, rm.end_column,
                opts,
            ));
        }
    }
    out
}
