use tractor_core::{report::Report, normalize_path};

/// Render report matches as GitHub Actions annotations: `::error file=...,line=...::reason`
pub fn render_github(report: &Report) -> String {
    let mut out = String::new();
    let iter: Box<dyn Iterator<Item = &tractor_core::report::ReportMatch>> =
        if let Some(ref groups) = report.groups {
            Box::new(groups.iter().flat_map(|g| g.matches.iter()))
        } else {
            Box::new(report.matches.iter())
        };
    for rm in iter {
        let reason = rm.reason.as_deref().unwrap_or("violation");
        let level  = rm.severity.map_or("error", |s| s.as_str());
        let file   = normalize_path(&rm.file);
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
    out
}
