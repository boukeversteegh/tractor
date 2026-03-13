use tractor_core::{format_message, report::Report, Match};
use super::shared::{to_absolute_path, append_source_context};

/// Render report matches in gcc format: `file:line:col: severity: reason`
pub fn render_gcc(report: &Report) -> String {
    let mut out = String::new();
    // Render from flat matches or grouped matches, whichever is populated.
    if let Some(ref groups) = report.groups {
        for g in groups {
            for rm in &g.matches {
                render_gcc_match(&mut out, rm);
            }
        }
    } else {
        for rm in &report.matches {
            render_gcc_match(&mut out, rm);
        }
    }
    out
}

fn render_gcc_match(out: &mut String, rm: &tractor_core::report::ReportMatch) {
    let reason   = rm.reason.as_deref().unwrap_or("violation");
    let severity = rm.severity.map_or("error", |s| s.as_str());
    let m = &rm.inner;
    out.push_str(&format!(
        "{}:{}:{}: {}: {}\n",
        to_absolute_path(&m.file), m.line, m.column, severity, reason
    ));
    append_source_context(out, m);
}

/// Render matches in gcc format using a message template (for `test --error`).
pub fn render_gcc_with_template(matches: &[Match], template: &str, is_warning: bool) -> String {
    let severity = if is_warning { "warning" } else { "error" };
    let mut out = String::new();
    for m in matches {
        let msg = format_message(template, m);
        out.push_str(&format!(
            "{}:{}:{}: {}: {}\n",
            to_absolute_path(&m.file), m.line, m.column, severity, msg
        ));
        append_source_context(&mut out, m);
    }
    out
}
