use tractor_core::{report::Report, normalize_path};

/// Render report matches as GitHub Actions annotations: `::error file=...,line=...::reason`
pub fn render_github(report: &Report) -> String {
    let mut out = String::new();
    for rm in &report.matches {
        let reason = rm.reason.as_deref().unwrap_or("violation");
        let level  = rm.severity.map_or("error", |s| s.as_str());
        let m      = &rm.inner;
        let file   = normalize_path(&m.file);
        out.push_str(&format!(
            "::{level} file={file},line={line},endLine={end_line},col={col},endColumn={end_col}::{reason}\n",
            level    = level,
            file     = file,
            line     = m.line,
            end_line = m.end_line,
            col      = m.column,
            end_col  = m.end_column,
            reason   = reason,
        ));
    }
    out
}
