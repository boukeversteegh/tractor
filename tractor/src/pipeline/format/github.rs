use tractor_core::{report::Report, normalize_path};

/// Render report matches as GitHub Actions annotations: `::error file=...,line=...::reason`
pub fn render_github(report: &Report) -> String {
    let mut out = String::new();
    if let Some(ref groups) = report.groups {
        for g in groups {
            for rm in &g.matches {
                render_github_match(&mut out, rm, Some(&g.file));
            }
        }
    } else {
        for rm in &report.matches {
            render_github_match(&mut out, rm, None);
        }
    }
    out
}

fn render_github_match(out: &mut String, rm: &tractor_core::report::ReportMatch, group_file: Option<&str>) {
    let reason = rm.reason.as_deref().unwrap_or("violation");
    let level  = rm.severity.map_or("error", |s| s.as_str());
    let file   = normalize_path(group_file.unwrap_or(&rm.file));
    out.push_str(&format!(
        "::{level} file={file},line={line},endLine={end_line},col={col},endColumn={end_col}::{reason}\n",
        level    = level,
        file     = file,
        line     = rm.line,
        end_line = rm.end_line,
        col      = rm.column,
        end_col  = rm.end_column,
        reason   = reason,
    ));
}
