use tractor_core::{render_lines, report::{Report, ReportMatch}, RenderOptions};
use super::shared::to_absolute_path;

/// Render report matches in gcc format: `file:line:col: severity: reason`
pub fn render_gcc(report: &Report, opts: &RenderOptions) -> String {
    let mut out = String::new();
    if let Some(ref groups) = report.groups {
        for g in groups {
            for rm in &g.matches {
                render_gcc_match(&mut out, rm, Some(&g.file), opts);
            }
        }
    } else {
        for rm in &report.matches {
            render_gcc_match(&mut out, rm, None, opts);
        }
    }
    out
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
