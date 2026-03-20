//! Per-match text output rendering.
//!
//! Provides three public functions that render a single `Match` to a string
//! for use in the plain-text output path:
//!
//! - `render_tree_match`  — the XML tree fragment (pretty-printed)
//! - `render_source_match` — the column-precise source snippet
//! - `render_lines_match`  — the source lines spanning the match
//!
//! All three accept a `RenderOptions` and respect `use_color` and `language`.

use crate::xpath::{Match, XmlNode};
use crate::output::xml_renderer::{render_xml_node, RenderOptions};
use crate::output::syntax_highlight::{extract_syntax_spans_from_xml_node, highlight_source, highlight_lines};
use crate::languages::get_syntax_category;
use regex::Regex;

/// Render the XML tree fragment for a single match.
///
/// Falls back to the match value if no XML fragment is available.
/// Always ends with a newline when `opts.pretty_print` is true.
pub fn render_tree_match(m: &Match, opts: &RenderOptions) -> String {
    if let Some(ref node) = m.xml_node {
        let rendered = render_xml_node(node, opts);
        if opts.pretty_print && !rendered.ends_with('\n') {
            format!("{}\n", rendered)
        } else {
            rendered
        }
    } else {
        let mut s = m.value.clone();
        if opts.pretty_print { s.push('\n'); }
        s
    }
}

/// Render the column-precise source snippet for a single match.
///
/// Applies syntax highlighting when `opts.use_color` is true and a language
/// is set in `opts.language`.
pub fn render_source_match(m: &Match, opts: &RenderOptions) -> String {
    let snippet = m.extract_source_snippet();
    if opts.use_color && m.xml_node.is_some() && !snippet.is_empty() {
        let category_fn = get_syntax_category(opts.language.as_deref().unwrap_or(""));
        let spans = extract_syntax_spans_from_xml_node(m.xml_node.as_ref().unwrap(), category_fn);
        if !spans.is_empty() {
            let highlighted = highlight_source(
                &snippet, &spans, m.line, m.column, m.end_line, m.end_column,
            );
            return format!("{}\n", highlighted);
        }
    }
    format!("{}\n", snippet)
}

/// Render the source lines spanning a single match.
///
/// Applies syntax highlighting when `opts.use_color` is true and a language
/// is set in `opts.language`.
pub fn render_lines_match(m: &Match, opts: &RenderOptions) -> String {
    let lines = m.get_source_lines_range();
    let lines_vec: Vec<String> = lines.iter()
        .map(|l| l.trim_end_matches('\r').to_string())
        .collect();
    if opts.use_color && m.xml_node.is_some() {
        let category_fn = get_syntax_category(opts.language.as_deref().unwrap_or(""));
        let spans = extract_syntax_spans_from_xml_node(m.xml_node.as_ref().unwrap(), category_fn);
        if !spans.is_empty() {
            return format!("{}\n", highlight_lines(&lines_vec, &spans, m.line, m.end_line));
        }
    }
    let mut out = String::new();
    for line in &lines_vec {
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Normalize a file path to use forward slashes.
pub fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

/// Render a pre-computed source snippet with optional syntax highlighting.
///
/// When `xml_node` is `Some`, uses it to extract syntax spans for highlighting.
/// Requires `opts.use_color` to be true for coloring to take effect.
pub fn render_source_precomputed(
    snippet: &str,
    xml_node: Option<&XmlNode>,
    line: u32,
    column: u32,
    end_line: u32,
    end_column: u32,
    opts: &RenderOptions,
) -> String {
    if opts.use_color {
        if let Some(node) = xml_node {
            let category_fn = get_syntax_category(opts.language.as_deref().unwrap_or(""));
            let spans = extract_syntax_spans_from_xml_node(node, category_fn);
            if !spans.is_empty() {
                let highlighted = highlight_source(snippet, &spans, line, column, end_line, end_column);
                return format!("{}\n", highlighted);
            }
        }
    }
    format!("{}\n", snippet)
}

/// Render source lines with line-number gutter, caret mark, and optional
/// syntax highlighting.
///
/// Lines should have trailing `\r` already stripped. When `xml_node` is
/// `Some`, syntax spans are extracted for highlighting.
///
/// Output for a single-line match:
/// ```text
/// 7 | public class Qux { }
///     ^~~~~~~~~~~~~~~~~~~~
/// ```
///
/// Multi-line (≤6 lines):
/// ```text
/// 1 >| public class Foo
/// 2  | {
/// 5 >| }
/// ```
pub fn render_lines(
    lines: &[String],
    xml_node: Option<&XmlNode>,
    start_line: u32,
    column: u32,
    end_line: u32,
    end_column: u32,
    opts: &RenderOptions,
) -> String {
    if lines.is_empty() || start_line == 0 {
        return String::new();
    }

    // Apply syntax highlighting to the lines if possible.
    let highlighted: Option<Vec<String>> = if opts.use_color {
        xml_node.and_then(|node| {
            let category_fn = get_syntax_category(opts.language.as_deref().unwrap_or(""));
            let spans = extract_syntax_spans_from_xml_node(node, category_fn);
            if spans.is_empty() {
                None
            } else {
                let h = highlight_lines(lines, &spans, start_line, end_line);
                Some(h.split('\n').map(|s| s.to_string()).collect())
            }
        })
    } else {
        None
    };
    let display: &[String] = highlighted.as_deref().unwrap_or(lines);

    let sl = start_line as usize;
    let el = (end_line as usize).min(sl + lines.len() - 1);
    let line_count = el.saturating_sub(sl) + 1;
    let lnw = el.to_string().len(); // line-number width

    let mut out = String::new();

    if line_count == 1 {
        let src = display.first().map(|s| s.as_str()).unwrap_or("");
        out.push_str(&format!("{:>w$} | {}\n", sl, src, w = lnw));
        let ul_len = (end_column as usize).saturating_sub(column as usize).max(1);
        let padding = " ".repeat(lnw + 3 + (column as usize).saturating_sub(1));
        let underline = format!("^{}", "~".repeat(ul_len.saturating_sub(1)));
        out.push_str(&format!("{}{}\n", padding, underline));
    } else if line_count <= 6 {
        for (i, line) in display.iter().enumerate().take(line_count) {
            let lineno = sl + i;
            let marker = if lineno == sl || lineno == el { ">" } else { " " };
            out.push_str(&format!("{:>w$} {}| {}\n", lineno, marker, line, w = lnw));
        }
    } else {
        // First 2 lines
        for i in 0..2 {
            if i < display.len() {
                out.push_str(&format!("{:>w$} >| {}\n", sl + i, &display[i], w = lnw));
            }
        }
        out.push_str(&format!("{:>w$}  | ... ({} more lines)\n", "...", line_count - 4, w = lnw));
        // Last 2 lines
        for i in (line_count - 2)..line_count {
            if i < display.len() {
                out.push_str(&format!("{:>w$} >| {}\n", sl + i, &display[i], w = lnw));
            }
        }
    }
    out.push('\n');
    out
}


/// Format a message template by replacing placeholders ({value}, {line}, {col}, {file}).
pub fn format_message(template: &str, m: &Match) -> String {
    if !template.contains('{') {
        return template.to_string();
    }
    let re = Regex::new(r"\{([^}]+)\}").unwrap();
    re.replace_all(template, |caps: &regex::Captures| {
        match &caps[1] {
            "value" => truncate(&m.value, 50),
            "line"  => m.line.to_string(),
            "col"   => m.column.to_string(),
            "file"  => normalize_path(&m.file),
            expr    => format!("{{{}}}", expr),
        }
    }).to_string()
}

fn truncate(s: &str, max_len: usize) -> String {
    let normalized: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.len() <= max_len {
        normalized
    } else {
        format!("{}...", &normalized[..max_len.saturating_sub(3)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_format_message() {
        let m = Match::with_location(
            "test.cs".to_string(), 10, 5, 10, 15,
            "MyMethod".to_string(),
            Arc::new(vec![]),
        );
        assert_eq!(
            format_message("found {value} at line {line}", &m),
            "found MyMethod at line 10"
        );
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("short", 50), "short");
        assert_eq!(
            truncate("this is a very long string that should be truncated", 20),
            "this is a very lo..."
        );
    }
}
