//! Unified XML renderer with color support
//!
//! Renders xot nodes to strings with optional ANSI color codes.
//! This replaces the regex-based colorization hack.

use xot::{Xot, Node, Value};
use std::collections::HashSet;

/// ANSI color codes (following tractor brand guidelines)
pub mod ansi {
    pub const RESET: &str = "\x1b[0m";
    pub const DIM: &str = "\x1b[2m";
    pub const BOLD: &str = "\x1b[1m";
    pub const BLUE: &str = "\x1b[34m";   // Primary: element/tag names
    pub const CYAN: &str = "\x1b[36m";   // Secondary: attribute names
    pub const YELLOW: &str = "\x1b[33m"; // Accent: attribute values
    pub const BLACK: &str = "\x1b[30m";  // For highlight backgrounds
    pub const BG_YELLOW: &str = "\x1b[43m"; // For match highlights
}

/// Options for XML rendering
#[derive(Debug, Clone, Default)]
pub struct RenderOptions {
    /// Whether to use ANSI colors
    pub use_color: bool,
    /// Whether to include location attributes (start, end, etc.)
    pub include_locations: bool,
    /// Indentation string (default: 2 spaces)
    pub indent: String,
    /// Maximum depth to render (None = unlimited)
    pub max_depth: Option<usize>,
    /// Positions (line, col) to highlight as matches
    pub highlights: Option<HashSet<(u32, u32)>>,
    /// Pretty print with indentation and newlines (default: true).
    /// Set to false for XPath queries where formatting whitespace would
    /// corrupt string-value comparisons like `[.='exact match']`.
    pub pretty_print: bool,
}

impl RenderOptions {
    pub fn new() -> Self {
        RenderOptions {
            use_color: false,
            include_locations: true,
            indent: "  ".to_string(),
            max_depth: None,
            highlights: None,
            pretty_print: true,
        }
    }

    pub fn with_color(mut self, use_color: bool) -> Self {
        self.use_color = use_color;
        self
    }

    pub fn with_locations(mut self, include: bool) -> Self {
        self.include_locations = include;
        self
    }

    pub fn with_max_depth(mut self, depth: Option<usize>) -> Self {
        self.max_depth = depth;
        self
    }

    pub fn with_highlights(mut self, highlights: HashSet<(u32, u32)>) -> Self {
        self.highlights = Some(highlights);
        self
    }

    pub fn with_pretty_print(mut self, pretty_print: bool) -> Self {
        self.pretty_print = pretty_print;
        self
    }
}

/// Render an xot node to a string with optional colors
pub fn render_node(xot: &Xot, node: Node, options: &RenderOptions) -> String {
    let mut output = String::new();
    render_node_recursive(xot, node, options, 0, &mut output);
    output
}

/// Render with XML declaration
pub fn render_document(xot: &Xot, node: Node, options: &RenderOptions) -> String {
    let mut output = String::new();

    // Add XML declaration
    if options.use_color {
        output.push_str(ansi::DIM);
    }
    output.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
    if options.use_color {
        output.push_str(ansi::RESET);
    }
    if options.pretty_print {
        output.push('\n');
    }

    // Render the document content
    // Skip the document node itself and render its children
    if let Value::Document = xot.value(node) {
        for child in xot.children(node) {
            render_node_recursive(xot, child, options, 0, &mut output);
        }
    } else {
        render_node_recursive(xot, node, options, 0, &mut output);
    }

    output
}

/// Count all descendant elements (recursive)
fn count_descendants(xot: &Xot, node: Node) -> usize {
    let mut count = 0;
    for child in xot.children(node) {
        if matches!(xot.value(child), Value::Element(_)) {
            count += 1;
            count += count_descendants(xot, child);
        }
    }
    count
}

/// Extract (line, col) from "start" attribute value like "5:10"
fn extract_position(xot: &Xot, node: Node) -> Option<(u32, u32)> {
    let attrs = xot.attributes(node);
    for (attr_name_id, attr_value) in attrs.iter() {
        if xot.local_name_str(attr_name_id) == "start" {
            let parts: Vec<&str> = attr_value.split(':').collect();
            if parts.len() == 2 {
                if let (Ok(line), Ok(col)) = (parts[0].parse(), parts[1].parse()) {
                    return Some((line, col));
                }
            }
        }
    }
    None
}

fn render_node_recursive(
    xot: &Xot,
    node: Node,
    options: &RenderOptions,
    depth: usize,
    output: &mut String,
) {
    let indent = if options.pretty_print {
        options.indent.repeat(depth)
    } else {
        String::new()
    };

    match xot.value(node) {
        Value::Document => {
            // Render children of document
            for child in xot.children(node) {
                render_node_recursive(xot, child, options, depth, output);
            }
        }
        Value::Element(element) => {
            let name = xot.local_name_str(element.name());

            // Check if children should be truncated (at max depth)
            let truncate_children = options.max_depth.map_or(false, |max| depth >= max);

            // Check if this element should be highlighted
            let is_highlighted = options.highlights.as_ref().map_or(false, |highlights| {
                extract_position(xot, node)
                    .map_or(false, |pos| highlights.contains(&pos))
            });

            // Opening tag
            output.push_str(&indent);

            // Start highlight
            if is_highlighted && options.use_color {
                output.push_str(ansi::BG_YELLOW);
                output.push_str(ansi::BLACK);
                output.push_str(ansi::BOLD);
            }

            render_open_tag(xot, node, name, options, is_highlighted, output);

            // Check if element has children
            let children: Vec<_> = xot.children(node).collect();

            if children.is_empty() {
                // Self-closing tag
                if !is_highlighted && options.use_color {
                    output.push_str(ansi::DIM);
                }
                output.push_str("/>");
                if options.use_color {
                    output.push_str(ansi::RESET);
                }
                if options.pretty_print {
                    output.push('\n');
                }
            } else if children.len() == 1 && matches!(xot.value(children[0]), Value::Text(_)) {
                // Single text child - render inline
                if !is_highlighted && options.use_color {
                    output.push_str(ansi::DIM);
                }
                output.push('>');
                if is_highlighted && options.use_color {
                    output.push_str(ansi::RESET);
                }

                // Render text content (strip trailing newlines in pretty-print mode
                // since those are typically parser artifacts like in line comments)
                if let Value::Text(text) = xot.value(children[0]) {
                    let text_str = text.get();
                    let content = if options.pretty_print {
                        text_str.trim_end_matches('\n').trim_end_matches('\r')
                    } else {
                        text_str
                    };
                    output.push_str(&escape_xml(content));
                }

                // Closing tag
                if is_highlighted && options.use_color {
                    output.push_str(ansi::BG_YELLOW);
                    output.push_str(ansi::BLACK);
                    output.push_str(ansi::BOLD);
                } else if options.use_color {
                    output.push_str(ansi::DIM);
                }
                output.push_str("</");
                if !is_highlighted && options.use_color {
                    output.push_str(ansi::RESET);
                    output.push_str(ansi::BLUE);
                }
                output.push_str(name);
                if !is_highlighted && options.use_color {
                    output.push_str(ansi::RESET);
                    output.push_str(ansi::DIM);
                }
                output.push('>');
                if options.use_color {
                    output.push_str(ansi::RESET);
                }
                if options.pretty_print {
                    output.push('\n');
                }
            } else if truncate_children {
                // At max depth - show truncation comment instead of children
                let child_count = count_descendants(xot, node);
                if !is_highlighted && options.use_color {
                    output.push_str(ansi::DIM);
                }
                output.push('>');
                if options.use_color {
                    output.push_str(ansi::RESET);
                }
                if options.pretty_print {
                    output.push('\n');

                    // Truncation comment
                    let child_indent = options.indent.repeat(depth + 1);
                    output.push_str(&child_indent);
                    if options.use_color {
                        output.push_str(ansi::DIM);
                    }
                    output.push_str(&format!("<!-- ... ({} more) -->\n", child_count));
                    if options.use_color {
                        output.push_str(ansi::RESET);
                    }
                }

                // Closing tag
                output.push_str(&indent);
                if is_highlighted && options.use_color {
                    output.push_str(ansi::BG_YELLOW);
                    output.push_str(ansi::BLACK);
                    output.push_str(ansi::BOLD);
                } else if options.use_color {
                    output.push_str(ansi::DIM);
                }
                output.push_str("</");
                if !is_highlighted && options.use_color {
                    output.push_str(ansi::RESET);
                    output.push_str(ansi::BLUE);
                }
                output.push_str(name);
                if !is_highlighted && options.use_color {
                    output.push_str(ansi::RESET);
                    output.push_str(ansi::DIM);
                }
                output.push('>');
                if options.use_color {
                    output.push_str(ansi::RESET);
                }
                if options.pretty_print {
                    output.push('\n');
                }
            } else {
                // Multiple children or element children
                if !is_highlighted && options.use_color {
                    output.push_str(ansi::DIM);
                }
                output.push('>');
                if options.use_color {
                    output.push_str(ansi::RESET);
                }
                if options.pretty_print {
                    output.push('\n');
                }

                // Render children
                for child in children {
                    render_node_recursive(xot, child, options, depth + 1, output);
                }

                // Closing tag
                output.push_str(&indent);
                if is_highlighted && options.use_color {
                    output.push_str(ansi::BG_YELLOW);
                    output.push_str(ansi::BLACK);
                    output.push_str(ansi::BOLD);
                } else if options.use_color {
                    output.push_str(ansi::DIM);
                }
                output.push_str("</");
                if !is_highlighted && options.use_color {
                    output.push_str(ansi::RESET);
                    output.push_str(ansi::BLUE);
                }
                output.push_str(name);
                if !is_highlighted && options.use_color {
                    output.push_str(ansi::RESET);
                    output.push_str(ansi::DIM);
                }
                output.push('>');
                if options.use_color {
                    output.push_str(ansi::RESET);
                }
                if options.pretty_print {
                    output.push('\n');
                }
            }
        }
        Value::Text(text) => {
            // Standalone text node (unusual but handle it)
            let text_str = text.get();
            if options.pretty_print {
                // In pretty print mode, trim whitespace-only text nodes
                let trimmed = text_str.trim();
                if !trimmed.is_empty() {
                    output.push_str(&indent);
                    output.push_str(&escape_xml(trimmed));
                    output.push('\n');
                }
            } else {
                // Non-pretty mode: preserve exact text content for XPath matching
                output.push_str(&escape_xml(text_str));
            }
        }
        Value::Comment(comment) => {
            output.push_str(&indent);
            if options.use_color {
                output.push_str(ansi::DIM);
            }
            output.push_str("<!--");
            output.push_str(comment.get());
            output.push_str("-->");
            if options.use_color {
                output.push_str(ansi::RESET);
            }
            if options.pretty_print {
                output.push('\n');
            }
        }
        Value::ProcessingInstruction(pi) => {
            output.push_str(&indent);
            if options.use_color {
                output.push_str(ansi::DIM);
            }
            output.push_str("<?");
            let target_str = xot.local_name_str(pi.target());
            output.push_str(target_str);
            if let Some(d) = pi.data() {
                output.push(' ');
                output.push_str(d);
            }
            output.push_str("?>");
            if options.use_color {
                output.push_str(ansi::RESET);
            }
            if options.pretty_print {
                output.push('\n');
            }
        }
        _ => {
            // Namespace nodes, attribute nodes (handled separately), etc.
        }
    }
}

fn render_open_tag(
    xot: &Xot,
    node: Node,
    name: &str,
    options: &RenderOptions,
    is_highlighted: bool,
    output: &mut String,
) {
    // Opening bracket (skip color if highlighted - already has background)
    if !is_highlighted && options.use_color {
        output.push_str(ansi::DIM);
    }
    output.push('<');
    if !is_highlighted && options.use_color {
        output.push_str(ansi::RESET);
    }

    // Element name
    if !is_highlighted && options.use_color {
        output.push_str(ansi::BLUE);
    }
    output.push_str(name);
    if !is_highlighted && options.use_color {
        output.push_str(ansi::RESET);
    }

    // Attributes
    let attrs = xot.attributes(node);
    for (attr_name_id, attr_value) in attrs.iter() {
        let attr_name = xot.local_name_str(attr_name_id);

        // Skip location and internal attributes if not wanted
        if !options.include_locations {
            if matches!(
                attr_name,
                "start" | "end" | "startLine" | "startCol" | "endLine" | "endCol" | "kind"
            ) {
                continue;
            }
        }

        output.push(' ');

        // Attribute name
        if !is_highlighted && options.use_color {
            output.push_str(ansi::CYAN);
        }
        output.push_str(attr_name);
        if !is_highlighted && options.use_color {
            output.push_str(ansi::RESET);
        }

        // Equals and opening quote
        if !is_highlighted && options.use_color {
            output.push_str(ansi::DIM);
        }
        output.push_str("=\"");
        if !is_highlighted && options.use_color {
            output.push_str(ansi::RESET);
        }

        // Attribute value
        if !is_highlighted && options.use_color {
            output.push_str(ansi::YELLOW);
        }
        output.push_str(&escape_xml(attr_value));
        if !is_highlighted && options.use_color {
            output.push_str(ansi::RESET);
        }

        // Closing quote
        if !is_highlighted && options.use_color {
            output.push_str(ansi::DIM);
        }
        output.push('"');
        if !is_highlighted && options.use_color {
            output.push_str(ansi::RESET);
        }
    }
}

/// Escape XML special characters
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Render an XML string with colors using proper tree-walking
/// This is a drop-in replacement for the regex-based colorize_xml
pub fn render_xml_string(xml: &str, options: &RenderOptions) -> String {
    if xml.is_empty() {
        return xml.to_string();
    }

    // Try to parse as a complete document
    let mut xot = Xot::new();

    // Try parsing as-is first
    if let Ok(doc) = xot.parse(xml) {
        return render_node(&xot, doc, options);
    }

    // If that fails, try wrapping in a root element (for fragments)
    let wrapped = format!("<_root_>{}</_root_>", xml);
    if let Ok(doc) = xot.parse(&wrapped) {
        // Render children of _root_, not the wrapper itself
        if let Ok(doc_el) = xot.document_element(doc) {
            let mut output = String::new();
            for child in xot.children(doc_el) {
                render_node_recursive(&xot, child, options, 0, &mut output);
            }
            return output;
        }
    }

    // If all else fails, return original string (maybe add warning?)
    xml.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_xml() {
        assert_eq!(escape_xml("hello"), "hello");
        assert_eq!(escape_xml("<tag>"), "&lt;tag&gt;");
        assert_eq!(escape_xml("a & b"), "a &amp; b");
        assert_eq!(escape_xml("\"quoted\""), "&quot;quoted&quot;");
    }

    #[test]
    fn test_render_options_builder() {
        let opts = RenderOptions::new()
            .with_color(true)
            .with_locations(false);

        assert!(opts.use_color);
        assert!(!opts.include_locations);
    }

    #[test]
    fn test_max_depth_truncation() {
        let xml = r#"<root><a><b><c>text</c></b></a></root>"#;
        let opts = RenderOptions::new()
            .with_max_depth(Some(1));  // root=0, a=1, b=2 (truncated)

        let output = render_xml_string(xml, &opts);

        // Should contain truncation comment with count
        assert!(output.contains("<!-- ... (2 more) -->"));
        // Should not contain the deeply nested elements
        assert!(!output.contains("<b>"));
        assert!(!output.contains("<c>"));
    }
}
