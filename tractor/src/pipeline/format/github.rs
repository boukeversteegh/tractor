use tractor_core::{report::{Report, ResultItem}, normalize_path};

/// Render report matches as GitHub Actions annotations: `::error file=...,line=...::reason`
/// GitHub annotations are self-contained — grouping affects ordering only,
/// not field omission. Every annotation includes all fields.
pub fn render_github(report: &Report, _dimensions: &[&str]) -> String {
    let mut out = String::new();
    render_github_results(&mut out, &report.results, None);
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
    // GitHub Actions only supports error, warning, notice
    let level = rm.severity.map_or("error", |s| match s {
        tractor_core::report::Severity::Fatal => "error",
        tractor_core::report::Severity::Error => "error",
        tractor_core::report::Severity::Warning => "warning",
        tractor_core::report::Severity::Info => "notice",
    });
    let file   = group_file.unwrap_or(&rm.file);
    let message = match &rm.hint {
        Some(hint) => format!("{} (hint: {})", reason, hint),
        None => reason.to_string(),
    };
    let message = escape_github_message(&message);
    if file.is_empty() {
        // No real file — annotations don't make sense, just log the error.
        // GitHub Actions captures stderr/stdout in the action logs.
        let prefix = rm.origin.map_or("tractor", |o| o.as_str());
        out.push_str(&format!("{}: {}: {}\n", prefix, level, message));
    } else {
        let file = normalize_path(file);
        out.push_str(&format!(
            "::{level} file={file},line={line},endLine={end_line},col={col},endColumn={end_col}::{message}\n",
            level    = level,
            file     = file,
            line     = rm.line,
            end_line = rm.end_line,
            col      = rm.column,
            end_col  = rm.end_column,
            message  = message,
        ));
    }
}

/// Escape characters that break GitHub Actions workflow command parsing.
fn escape_github_message(s: &str) -> String {
    s.replace('%', "%25").replace('\r', "%0D").replace('\n', "%0A")
}
