//! Output formatters for different output modes

use crate::xpath::Match;
use crate::output::xml_renderer::{render_xml_string, RenderOptions};
use crate::output::syntax_highlight::{extract_syntax_spans_with_lang, highlight_source, highlight_lines};
use crate::languages::get_syntax_category;
use regex::Regex;

/// Internal dispatch enum for the text output path of `format_matches()`.
///
/// # OBSOLETE
///
/// This enum is a legacy internal detail. The canonical view-field representation
/// is `ViewField`/`ViewSet` in the tractor binary layer. `ViewSet::primary_output_format()`
/// is a compatibility shim that translates into this enum so the old text renderer
/// still works.
///
/// TODO: Eliminate this enum by inlining the per-variant dispatch directly into
/// `format_matches()` and replacing call sites with direct `ViewSet` checks.
/// That removes the shim and this type entirely.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextViewMode {
    /// XML of matched nodes
    Xml,
    /// Full source lines containing the match
    Lines,
    /// Exact matched source (column-precise)
    Source,
    /// Text content of matched node
    Value,
    /// Number of matches
    Count,
    /// Merged schema tree showing unique element paths
    Schema,
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
    /// Language for syntax highlighting (e.g., "csharp", "rust")
    pub language: Option<String>,
    /// Whether to use warning severity (for github format: ::warning instead of ::error)
    pub warning: bool,
}

/// Format matches according to the specified view format
pub fn format_matches(matches: &[Match], format: TextViewMode, options: &OutputOptions) -> String {
    match format {
        TextViewMode::Xml => format_xml(matches, options),
        TextViewMode::Lines => format_lines(matches, options),
        TextViewMode::Source => format_source(matches, options),
        TextViewMode::Value => format_value(matches),
        TextViewMode::Count => format_count(matches),
        TextViewMode::Schema => String::new(), // Handled separately - requires full XML aggregation
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
    // Get language-specific category function
    let category_fn = get_syntax_category(options.language.as_deref().unwrap_or(""));

    for m in matches {
        let lines = m.get_source_lines_range();
        let lines_vec: Vec<String> = lines.iter().map(|l| l.trim_end_matches('\r').to_string()).collect();

        if options.use_color && m.xml_fragment.is_some() {
            // Apply syntax highlighting with language-specific category mapping
            let spans = extract_syntax_spans_with_lang(m.xml_fragment.as_ref().unwrap(), category_fn);
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
    // Get language-specific category function
    let category_fn = get_syntax_category(options.language.as_deref().unwrap_or(""));

    for m in matches {
        let snippet = m.extract_source_snippet();

        if options.use_color && m.xml_fragment.is_some() && !snippet.is_empty() {
            // Apply syntax highlighting with language-specific category mapping
            let spans = extract_syntax_spans_with_lang(m.xml_fragment.as_ref().unwrap(), category_fn);
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

/// Normalize a file path to use forward slashes
pub fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn format_count(matches: &[Match]) -> String {
    format!("{}\n", matches.len())
}

/// Format a message template by replacing placeholders ({value}, {line}, {col}, {file}).
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
            "file" => normalize_path(&m.file),
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
            std::sync::Arc::new(vec![]),
        );
        assert_eq!(format_message("found {value} at line {line}", &m), "found MyMethod at line 10");
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("short", 50), "short");
        assert_eq!(truncate("this is a very long string that should be truncated", 20), "this is a very lo...");
    }
}
