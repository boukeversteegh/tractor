use tractor_core::{report::{Report, ReportMatch}, Match};
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

fn render_gcc_match(out: &mut String, rm: &ReportMatch) {
    let reason   = rm.reason.as_deref().unwrap_or("violation");
    let severity = rm.severity.map_or("error", |s| s.as_str());
    out.push_str(&format!(
        "{}:{}:{}: {}: {}\n",
        to_absolute_path(&rm.file), rm.line, rm.column, severity, reason
    ));
    // Reconstruct a minimal Match for source context rendering
    let m = make_context_match(rm);
    append_source_context(out, &m);
}


/// Render ReportMatches in gcc format using a message template (for `test --error`).
pub fn render_gcc_report_with_template(matches: &[ReportMatch], template: &str, is_warning: bool) -> String {
    let severity = if is_warning { "warning" } else { "error" };
    let mut out = String::new();
    for rm in matches {
        // Build the message from the template using available fields
        let msg = template
            .replace("{file}", &tractor_core::normalize_path(&rm.file))
            .replace("{line}", &rm.line.to_string())
            .replace("{col}", &rm.column.to_string())
            .replace("{value}", rm.value.as_deref().unwrap_or(""));
        out.push_str(&format!(
            "{}:{}:{}: {}: {}\n",
            to_absolute_path(&rm.file), rm.line, rm.column, severity, msg
        ));
        let m = make_context_match(rm);
        append_source_context(&mut out, &m);
    }
    out
}

/// Construct a minimal Match for source-context rendering.
/// source_lines is empty (already consumed at report-build time), so context
/// is suppressed — this is acceptable since gcc format is CI-oriented.
fn make_context_match(rm: &ReportMatch) -> Match {
    use std::sync::Arc;
    Match::with_location(
        rm.file.clone(),
        rm.line, rm.column,
        rm.end_line, rm.end_column,
        rm.value.clone().unwrap_or_default(),
        Arc::new(vec![]),
    )
}
