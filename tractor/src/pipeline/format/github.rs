use tractor_core::{report::{Report, ResultItem}, normalize_path};

/// Render report matches as GitHub Actions annotations: `::error file=...,line=...::reason`
pub fn render_github(report: &Report) -> String {
    let mut out = String::new();
    render_github_results(&mut out, &report.results, None);
    // Fallback to old fields if results is empty
    if out.is_empty() {
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
    }
    out
}

fn render_github_results(out: &mut String, items: &[ResultItem], parent_file: Option<&str>) {
    for item in items {
        match item {
            ResultItem::Match(rm) => {
                render_github_match(out, rm, parent_file);
            }
            ResultItem::Group(g) => {
                let file = g.file.as_deref().or(parent_file);
                render_github_results(out, &g.results, file);
            }
        }
    }
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
