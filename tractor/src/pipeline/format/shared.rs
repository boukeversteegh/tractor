//! Shared helpers used across multiple format renderers.

use std::path::Path;
use tractor_core::{normalize_path, Match};

pub fn to_absolute_path(path: &str) -> String {
    let p = Path::new(path);
    let absolute = if p.is_absolute() {
        p.to_path_buf()
    } else if let Ok(cwd) = std::env::current_dir() {
        cwd.join(p)
    } else {
        p.to_path_buf()
    };
    normalize_path(&absolute.to_string_lossy())
}

pub fn append_source_context(output: &mut String, m: &Match) {
    if m.source_lines.is_empty() || m.line == 0 {
        return;
    }
    let start_line     = m.line as usize;
    let end_line       = (m.end_line as usize).min(m.source_lines.len());
    let line_count     = end_line.saturating_sub(start_line) + 1;
    let line_num_width = end_line.to_string().len();

    if line_count == 1 && start_line <= m.source_lines.len() {
        let source_line   = m.source_lines[start_line - 1].trim_end_matches('\r');
        let underline_len = (m.end_column as usize).saturating_sub(m.column as usize).max(1);
        let padding       = " ".repeat(line_num_width + 3 + (m.column as usize).saturating_sub(1));
        let underline     = format!("^{}", "~".repeat(underline_len.saturating_sub(1)));
        output.push_str(&format!("{:>width$} | {}\n", start_line, source_line, width = line_num_width));
        output.push_str(&format!("{}{}\n", padding, underline));
    } else if line_count <= 6 {
        for i in start_line..=end_line {
            if i <= m.source_lines.len() {
                let source_line = m.source_lines[i - 1].trim_end_matches('\r');
                let marker = if i == start_line || i == end_line { ">" } else { " " };
                output.push_str(&format!("{:>width$} {}| {}\n", i, marker, source_line, width = line_num_width));
            }
        }
    } else {
        for i in start_line..start_line + 2 {
            if i <= m.source_lines.len() {
                let source_line = m.source_lines[i - 1].trim_end_matches('\r');
                output.push_str(&format!("{:>width$} >| {}\n", i, source_line, width = line_num_width));
            }
        }
        output.push_str(&format!("{:>width$}  | ... ({} more lines)\n", "...", line_count - 4, width = line_num_width));
        for i in (end_line - 1)..=end_line {
            if i <= m.source_lines.len() {
                let source_line = m.source_lines[i - 1].trim_end_matches('\r');
                output.push_str(&format!("{:>width$} >| {}\n", i, source_line, width = line_num_width));
            }
        }
    }
    output.push('\n');
}
