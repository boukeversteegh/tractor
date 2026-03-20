use tractor_core::report::{Report, ReportMatch};
use super::shared::to_absolute_path;

/// Render report matches in gcc format: `file:line:col: severity: reason`
pub fn render_gcc(report: &Report) -> String {
    let mut out = String::new();
    // Render from flat matches or grouped matches, whichever is populated.
    if let Some(ref groups) = report.groups {
        for g in groups {
            for rm in &g.matches {
                render_gcc_match(&mut out, rm, Some(&g.file));
            }
        }
    } else {
        for rm in &report.matches {
            render_gcc_match(&mut out, rm, None);
        }
    }
    out
}

fn render_gcc_match(out: &mut String, rm: &ReportMatch, group_file: Option<&str>) {
    let reason   = rm.reason.as_deref().unwrap_or("violation");
    let severity = rm.severity.map_or("error", |s| s.as_str());
    let file = group_file.unwrap_or(&rm.file);
    out.push_str(&format!(
        "{}:{}:{}: {}: {}\n",
        to_absolute_path(file), rm.line, rm.column, severity, reason
    ));
    append_source_context(out, rm);
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
        append_source_context(&mut out, rm);
    }
    out
}

/// Append source context with line-number gutter and caret mark beneath the
/// diagnostic line.  Only emits when the report match carries `lines` data.
fn append_source_context(output: &mut String, rm: &ReportMatch) {
    let lines = match rm.lines {
        Some(ref ls) if !ls.is_empty() => ls,
        _ => return,
    };
    if rm.line == 0 { return; }

    let start_line     = rm.line as usize;
    let end_line       = (rm.end_line as usize).min(start_line + lines.len() - 1);
    let line_count     = end_line.saturating_sub(start_line) + 1;
    let line_num_width = end_line.to_string().len();

    if line_count == 1 {
        let source_line   = &lines[0];
        let underline_len = (rm.end_column as usize).saturating_sub(rm.column as usize).max(1);
        let padding       = " ".repeat(line_num_width + 3 + (rm.column as usize).saturating_sub(1));
        let underline     = format!("^{}", "~".repeat(underline_len.saturating_sub(1)));
        output.push_str(&format!("{:>width$} | {}\n", start_line, source_line, width = line_num_width));
        output.push_str(&format!("{}{}\n", padding, underline));
    } else if line_count <= 6 {
        for (i, line) in lines.iter().enumerate().take(line_count) {
            let lineno = start_line + i;
            let marker = if lineno == start_line || lineno == end_line { ">" } else { " " };
            output.push_str(&format!("{:>width$} {}| {}\n", lineno, marker, line, width = line_num_width));
        }
    } else {
        // First 2 lines
        for i in 0..2 {
            let lineno = start_line + i;
            if i < lines.len() {
                output.push_str(&format!("{:>width$} >| {}\n", lineno, &lines[i], width = line_num_width));
            }
        }
        output.push_str(&format!("{:>width$}  | ... ({} more lines)\n", "...", line_count - 4, width = line_num_width));
        // Last 2 lines
        for i in (line_count - 2)..line_count {
            let lineno = start_line + i;
            if i < lines.len() {
                output.push_str(&format!("{:>width$} >| {}\n", lineno, &lines[i], width = line_num_width));
            }
        }
    }
    output.push('\n');
}
