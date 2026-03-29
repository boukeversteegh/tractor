use tractor_core::{render_lines, report::{Report, ReportMatch, ResultItem}, RenderOptions};
use super::shared::to_absolute_path;

/// Render report matches in gcc format: `file:line:col: severity: reason`
pub fn render_gcc(report: &Report, opts: &RenderOptions) -> String {
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
                // If this group has a file, use it as context for child matches
                let file = g.file.as_deref().or(parent_file);
                render_gcc_results(out, &g.results, file, opts);
            }
        }
    }
}

fn render_gcc_match(out: &mut String, rm: &ReportMatch, group_file: Option<&str>, opts: &RenderOptions) {
    let reason   = rm.reason.as_deref().unwrap_or("violation");
    let severity = rm.severity.map_or("error", |s| s.as_str());
    let file = group_file.unwrap_or(&rm.file);
    out.push_str(&format!(
        "{}:{}:{}: {}: {}\n",
        to_absolute_path(file), rm.line, rm.column, severity, reason
    ));
    if let Some(ref ls) = rm.lines {
        out.push_str(&render_lines(
            ls, rm.tree.as_ref(),
            rm.line, rm.column, rm.end_line, rm.end_column,
            opts,
        ));
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
