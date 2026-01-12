//! Unified XML renderer with color support
//!
//! Renders xot nodes to strings with optional ANSI color codes.
//! This replaces the regex-based colorization hack.

use xot::{Xot, Node, Value};

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
}

impl RenderOptions {
    pub fn new() -> Self {
        RenderOptions {
            use_color: false,
            include_locations: true,
            indent: "  ".to_string(),
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
    output.push('\n');

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

fn render_node_recursive(
    xot: &Xot,
    node: Node,
    options: &RenderOptions,
    depth: usize,
    output: &mut String,
) {
    let indent = options.indent.repeat(depth);

    match xot.value(node) {
        Value::Document => {
            // Render children of document
            for child in xot.children(node) {
                render_node_recursive(xot, child, options, depth, output);
            }
        }
        Value::Element(element) => {
            let name = xot.local_name_str(element.name());

            // Opening tag
            output.push_str(&indent);
            render_open_tag(xot, node, name, options, output);

            // Check if element has children
            let children: Vec<_> = xot.children(node).collect();

            if children.is_empty() {
                // Self-closing tag
                if options.use_color {
                    output.push_str(ansi::DIM);
                }
                output.push_str("/>");
                if options.use_color {
                    output.push_str(ansi::RESET);
                }
                output.push('\n');
            } else if children.len() == 1 && matches!(xot.value(children[0]), Value::Text(_)) {
                // Single text child - render inline
                if options.use_color {
                    output.push_str(ansi::DIM);
                }
                output.push('>');
                if options.use_color {
                    output.push_str(ansi::RESET);
                }

                // Render text content
                if let Value::Text(text) = xot.value(children[0]) {
                    output.push_str(&escape_xml(text.get()));
                }

                // Closing tag
                if options.use_color {
                    output.push_str(ansi::DIM);
                }
                output.push_str("</");
                if options.use_color {
                    output.push_str(ansi::RESET);
                    output.push_str(ansi::BLUE);
                }
                output.push_str(name);
                if options.use_color {
                    output.push_str(ansi::RESET);
                    output.push_str(ansi::DIM);
                }
                output.push('>');
                if options.use_color {
                    output.push_str(ansi::RESET);
                }
                output.push('\n');
            } else {
                // Multiple children or element children
                if options.use_color {
                    output.push_str(ansi::DIM);
                }
                output.push('>');
                if options.use_color {
                    output.push_str(ansi::RESET);
                }
                output.push('\n');

                // Render children
                for child in children {
                    render_node_recursive(xot, child, options, depth + 1, output);
                }

                // Closing tag
                output.push_str(&indent);
                if options.use_color {
                    output.push_str(ansi::DIM);
                }
                output.push_str("</");
                if options.use_color {
                    output.push_str(ansi::RESET);
                    output.push_str(ansi::BLUE);
                }
                output.push_str(name);
                if options.use_color {
                    output.push_str(ansi::RESET);
                    output.push_str(ansi::DIM);
                }
                output.push('>');
                if options.use_color {
                    output.push_str(ansi::RESET);
                }
                output.push('\n');
            }
        }
        Value::Text(text) => {
            // Standalone text node (unusual but handle it)
            let text_str = text.get();
            let trimmed = text_str.trim();
            if !trimmed.is_empty() {
                output.push_str(&indent);
                output.push_str(&escape_xml(trimmed));
                output.push('\n');
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
            output.push('\n');
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
            output.push('\n');
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
    output: &mut String,
) {
    // Opening bracket
    if options.use_color {
        output.push_str(ansi::DIM);
    }
    output.push('<');
    if options.use_color {
        output.push_str(ansi::RESET);
    }

    // Element name
    if options.use_color {
        output.push_str(ansi::BLUE);
    }
    output.push_str(name);
    if options.use_color {
        output.push_str(ansi::RESET);
    }

    // Attributes
    let attrs = xot.attributes(node);
    for (attr_name_id, attr_value) in attrs.iter() {
        let attr_name = xot.local_name_str(attr_name_id);

        // Skip location attributes if not wanted
        if !options.include_locations {
            if matches!(
                attr_name,
                "start" | "end" | "startLine" | "startCol" | "endLine" | "endCol"
            ) {
                continue;
            }
        }

        output.push(' ');

        // Attribute name
        if options.use_color {
            output.push_str(ansi::CYAN);
        }
        output.push_str(attr_name);
        if options.use_color {
            output.push_str(ansi::RESET);
        }

        // Equals and opening quote
        if options.use_color {
            output.push_str(ansi::DIM);
        }
        output.push_str("=\"");
        if options.use_color {
            output.push_str(ansi::RESET);
        }

        // Attribute value
        if options.use_color {
            output.push_str(ansi::YELLOW);
        }
        output.push_str(&escape_xml(attr_value));
        if options.use_color {
            output.push_str(ansi::RESET);
        }

        // Closing quote
        if options.use_color {
            output.push_str(ansi::DIM);
        }
        output.push('"');
        if options.use_color {
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
}
