//! Output formatters for different output modes

use crate::xpath::Match;
use crate::output::xml_renderer::{render_xml_string, RenderOptions};
use crate::output::syntax_highlight::{extract_syntax_spans, highlight_source, highlight_lines};
use regex::Regex;
use serde::Serialize;

/// Output format options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// XML of matched nodes
    Xml,
    /// Full source lines containing the match
    Lines,
    /// Exact matched source (column-precise)
    Source,
    /// Text content of matched node
    Value,
    /// GCC-style file:line:col: message
    Gcc,
    /// JSON array with match details
    Json,
    /// Number of matches
    Count,
}

impl OutputFormat {
    /// Parse format from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "xml" => Some(OutputFormat::Xml),
            "lines" => Some(OutputFormat::Lines),
            "source" => Some(OutputFormat::Source),
            "value" => Some(OutputFormat::Value),
            "gcc" => Some(OutputFormat::Gcc),
            "json" => Some(OutputFormat::Json),
            "count" => Some(OutputFormat::Count),
            _ => None,
        }
    }

    /// Get list of all valid format names
    pub fn valid_formats() -> &'static [&'static str] {
        &["xml", "lines", "source", "value", "gcc", "json", "count"]
    }
}

/// Options for output formatting
#[derive(Debug, Clone, Default)]
pub struct OutputOptions {
    /// Custom message template for GCC format
    pub message: Option<String>,
    /// Whether to use color in output
    pub use_color: bool,
    /// Whether to strip location metadata from XML
    pub strip_locations: bool,
    /// Maximum depth for XML rendering
    pub max_depth: Option<usize>,
    /// Whether to pretty print XML (default: true)
    pub pretty_print: bool,
}

/// JSON output structure
#[derive(Serialize)]
struct JsonMatch {
    file: String,
    line: u32,
    column: u32,
    value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

/// Format matches according to the specified format
pub fn format_matches(matches: &[Match], format: OutputFormat, options: &OutputOptions) -> String {
    match format {
        OutputFormat::Xml => format_xml(matches, options),
        OutputFormat::Lines => format_lines(matches, options),
        OutputFormat::Source => format_source(matches, options),
        OutputFormat::Value => format_value(matches),
        OutputFormat::Gcc => format_gcc(matches, options),
        OutputFormat::Json => format_json(matches, options),
        OutputFormat::Count => format_count(matches),
    }
}

fn format_xml(matches: &[Match], options: &OutputOptions) -> String {
    let mut output = String::new();
    let render_opts = RenderOptions::new()
        .with_color(options.use_color)
        .with_locations(!options.strip_locations)
        .with_max_depth(options.max_depth)
        .with_pretty_print(options.pretty_print);

    for m in matches {
        if let Some(ref xml) = m.xml_fragment {
            // Use proper tree-walking renderer for colorization
            let rendered = render_xml_string(xml, &render_opts);
            output.push_str(&rendered);
            // Add newline separator between matches if pretty printing
            if options.pretty_print && !rendered.ends_with('\n') {
                output.push('\n');
            }
        } else {
            // Fallback to value if no XML fragment
            output.push_str(&m.value);
            if options.pretty_print {
                output.push('\n');
            }
        }
    }
    output
}

fn format_lines(matches: &[Match], options: &OutputOptions) -> String {
    let mut output = String::new();
    for m in matches {
        let lines = m.get_source_lines_range();
        let lines_vec: Vec<String> = lines.iter().map(|l| l.trim_end_matches('\r').to_string()).collect();

        if options.use_color && m.xml_fragment.is_some() {
            // Apply syntax highlighting
            let spans = extract_syntax_spans(m.xml_fragment.as_ref().unwrap());
            if !spans.is_empty() {
                let highlighted = highlight_lines(&lines_vec, &spans, m.line, m.end_line);
                output.push_str(&highlighted);
                output.push('\n');
            } else {
                // No spans extracted, output plain
                for line in &lines_vec {
                    output.push_str(line);
                    output.push('\n');
                }
            }
        } else {
            // No color - output plain lines
            for line in &lines_vec {
                output.push_str(line);
                output.push('\n');
            }
        }
    }
    output
}

fn format_source(matches: &[Match], options: &OutputOptions) -> String {
    let mut output = String::new();
    for m in matches {
        let snippet = m.extract_source_snippet();

        if options.use_color && m.xml_fragment.is_some() && !snippet.is_empty() {
            // Apply syntax highlighting
            let spans = extract_syntax_spans(m.xml_fragment.as_ref().unwrap());
            if !spans.is_empty() {
                let highlighted = highlight_source(
                    &snippet,
                    &spans,
                    m.line,
                    m.column,
                    m.end_line,
                    m.end_column,
                );
                output.push_str(&highlighted);
                output.push('\n');
            } else {
                // No spans extracted, output plain
                output.push_str(&snippet);
                output.push('\n');
            }
        } else {
            // No color - output plain snippet
            output.push_str(&snippet);
            output.push('\n');
        }
    }
    output
}

fn format_value(matches: &[Match]) -> String {
    let mut output = String::new();
    for m in matches {
        output.push_str(&m.value);
        output.push('\n');
    }
    output
}

fn format_gcc(matches: &[Match], options: &OutputOptions) -> String {
    let mut output = String::new();
    for m in matches {
        let msg = format_message(
            options.message.as_deref().unwrap_or("match"),
            m,
        );
        output.push_str(&format!(
            "{}:{}:{}: error: {}\n",
            m.file, m.line, m.column, msg
        ));

        // Add source context
        if !m.source_lines.is_empty() && m.line > 0 {
            let start_line = m.line as usize;
            let end_line = (m.end_line as usize).min(m.source_lines.len());
            let line_count = end_line - start_line + 1;
            let line_num_width = end_line.to_string().len();

            if line_count == 1 && start_line <= m.source_lines.len() {
                let source_line = m.source_lines[start_line - 1].trim_end_matches('\r');
                output.push_str(&format!(
                    "{:>width$} | {}\n",
                    start_line,
                    source_line,
                    width = line_num_width
                ));

                // Add caret/underline
                let caret_col = (m.column as usize).saturating_sub(1);
                let underline_len = (m.end_column as usize).saturating_sub(m.column as usize).max(1);
                let padding = " ".repeat(line_num_width + 3 + caret_col);
                let underline = format!("^{}", "~".repeat(underline_len.saturating_sub(1)));
                output.push_str(&format!("{}{}\n", padding, underline));
            } else if line_count <= 6 {
                for i in start_line..=end_line {
                    if i <= m.source_lines.len() {
                        let source_line = m.source_lines[i - 1].trim_end_matches('\r');
                        let marker = if i == start_line || i == end_line { ">" } else { " " };
                        output.push_str(&format!(
                            "{:>width$} {}| {}\n",
                            i,
                            marker,
                            source_line,
                            width = line_num_width
                        ));
                    }
                }
            } else {
                // Show first 2, ellipsis, last 2
                for i in start_line..start_line + 2 {
                    if i <= m.source_lines.len() {
                        let source_line = m.source_lines[i - 1].trim_end_matches('\r');
                        output.push_str(&format!(
                            "{:>width$} >| {}\n",
                            i,
                            source_line,
                            width = line_num_width
                        ));
                    }
                }
                output.push_str(&format!(
                    "{:>width$}  | ... ({} more lines)\n",
                    "...",
                    line_count - 4,
                    width = line_num_width
                ));
                for i in (end_line - 1)..=end_line {
                    if i <= m.source_lines.len() {
                        let source_line = m.source_lines[i - 1].trim_end_matches('\r');
                        output.push_str(&format!(
                            "{:>width$} >| {}\n",
                            i,
                            source_line,
                            width = line_num_width
                        ));
                    }
                }
            }
            output.push('\n');
        }
    }
    output
}

fn format_json(matches: &[Match], options: &OutputOptions) -> String {
    let json_matches: Vec<JsonMatch> = matches
        .iter()
        .map(|m| JsonMatch {
            file: m.file.clone(),
            line: m.line,
            column: m.column,
            value: m.value.clone(),
            message: options.message.clone(),
        })
        .collect();

    serde_json::to_string_pretty(&json_matches).unwrap_or_else(|_| "[]".to_string())
}

fn format_count(matches: &[Match]) -> String {
    format!("{}\n", matches.len())
}

/// Format a message template by replacing placeholders
pub fn format_message(template: &str, m: &Match) -> String {
    if !template.contains('{') {
        return template.to_string();
    }

    let re = Regex::new(r"\{([^}]+)\}").unwrap();
    re.replace_all(template, |caps: &regex::Captures| {
        let expr = &caps[1];
        match expr {
            "value" => truncate(&m.value, 50),
            "line" => m.line.to_string(),
            "col" => m.column.to_string(),
            "file" => m.file.clone(),
            // For XPath expressions like {ancestor::class/name}, we'd need the XML context
            // For now, return the placeholder
            _ => format!("{{{}}}", expr),
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

    #[test]
    fn test_format_count() {
        let matches = vec![
            Match::new("test.cs".to_string(), "value1".to_string()),
            Match::new("test.cs".to_string(), "value2".to_string()),
        ];
        assert_eq!(format_count(&matches), "2\n");
    }

    #[test]
    fn test_format_value() {
        let matches = vec![
            Match::new("test.cs".to_string(), "Foo".to_string()),
            Match::new("test.cs".to_string(), "Bar".to_string()),
        ];
        assert_eq!(format_value(&matches), "Foo\nBar\n");
    }

    #[test]
    fn test_format_message() {
        let m = Match::with_location(
            "test.cs".to_string(),
            10,
            5,
            10,
            15,
            "MyMethod".to_string(),
            vec![],
        );
        assert_eq!(format_message("found {value} at line {line}", &m), "found MyMethod at line 10");
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("short", 50), "short");
        assert_eq!(truncate("this is a very long string that should be truncated", 20), "this is a very lo...");
    }
}
