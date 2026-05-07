//! Unified XML renderer with color support
//!
//! Renders xot nodes to strings with optional ANSI color codes.
//! This replaces the regex-based colorization hack.

use xot::{Xot, Node, Value};
use std::collections::HashSet;
use crate::xpath::XmlNode;

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
    /// Whether to include metadata attributes (start, end, kind, field)
    pub include_meta: bool,
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
    /// Source language for syntax highlighting (e.g. "csharp", "rust").
    /// Used by text-format renderers for -v source and -v lines.
    pub language: Option<String>,
    /// Shape-only rendering: emit element names + queryable markers/attrs
    /// but suppress source text content. Lets reviewers focus on tree
    /// structure changes; text preservation is enforced separately by
    /// `tests/text_preservation.rs`.
    pub shape_only: bool,
}

impl RenderOptions {
    pub fn new() -> Self {
        RenderOptions {
            use_color: false,
            include_meta: true,
            indent: "  ".to_string(),
            max_depth: None,
            highlights: None,
            pretty_print: true,
            language: None,
            shape_only: false,
        }
    }

    pub fn with_color(mut self, use_color: bool) -> Self {
        self.use_color = use_color;
        self
    }

    pub fn with_meta(mut self, include: bool) -> Self {
        self.include_meta = include;
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

    pub fn with_language(mut self, language: Option<String>) -> Self {
        self.language = language;
        self
    }

    pub fn with_shape_only(mut self, shape_only: bool) -> Self {
        self.shape_only = shape_only;
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

/// Extract (line, col) from line/column attributes
fn extract_position(xot: &Xot, node: Node) -> Option<(u32, u32)> {
    let attrs = xot.attributes(node);
    let mut line: Option<u32> = None;
    let mut col: Option<u32> = None;
    for (attr_name_id, attr_value) in attrs.iter() {
        match xot.local_name_str(attr_name_id) {
            "line" => line = attr_value.parse().ok(),
            "column" => col = attr_value.parse().ok(),
            _ => {}
        }
    }
    Some((line?, col?))
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

                // Render text content
                if let Value::Text(text) = xot.value(children[0]) {
                    output.push_str(&escape_xml(text.get()));
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
                    output.push_str(&format!("<!-- ... ({} children) -->\n", child_count));
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

        // Skip metadata attributes unless --meta is on. `list` is the
        // renderer-internal cardinality signal (read by xml_to_json
        // post-iter-139, ignored by everything else): hidden alongside
        // tree-sitter `field=` and source-location attributes.
        if !options.include_meta {
            if matches!(
                attr_name,
                "line" | "column" | "end_line" | "end_column"
                | "kind" | "field" | "list"
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

// ---------------------------------------------------------------------------
// XmlNode-native rendering (no XML string parsing)
// ---------------------------------------------------------------------------

/// Render an XmlNode tree to a string with optional colors.
///
/// This is the native-IR equivalent of `render_xml_string` — it renders
/// directly from the XmlNode tree without parsing an XML string.
pub fn render_xml_node(node: &XmlNode, options: &RenderOptions) -> String {
    let mut output = String::new();
    render_xml_node_recursive(node, options, 0, &mut output);
    output
}

/// Count all descendant elements in an XmlNode tree
fn count_xml_node_descendants(node: &XmlNode) -> usize {
    match node {
        XmlNode::Element { children, .. } => {
            let mut count = 0;
            for child in children {
                if matches!(child, XmlNode::Element { .. }) {
                    count += 1;
                    count += count_xml_node_descendants(child);
                }
            }
            count
        }
        _ => 0,
    }
}

/// Extract (line, col) from line/column attributes in an XmlNode element
fn extract_xml_node_position(attrs: &[(String, String)]) -> Option<(u32, u32)> {
    let mut line: Option<u32> = None;
    let mut col: Option<u32> = None;
    for (k, v) in attrs {
        match k.as_str() {
            "line" => line = v.parse().ok(),
            "column" => col = v.parse().ok(),
            _ => {}
        }
    }
    Some((line?, col?))
}

fn render_xml_node_recursive(
    node: &XmlNode,
    options: &RenderOptions,
    depth: usize,
    output: &mut String,
) {
    let indent = if options.pretty_print {
        options.indent.repeat(depth)
    } else {
        String::new()
    };

    match node {
        XmlNode::Element { name, attributes, children } => {
            let truncate_children = options.max_depth.map_or(false, |max| depth >= max);

            let is_highlighted = options.highlights.as_ref().map_or(false, |highlights| {
                extract_xml_node_position(attributes)
                    .map_or(false, |pos| highlights.contains(&pos))
            });

            output.push_str(&indent);

            if is_highlighted && options.use_color {
                output.push_str(ansi::BG_YELLOW);
                output.push_str(ansi::BLACK);
                output.push_str(ansi::BOLD);
            }

            // Opening tag
            render_xml_node_open_tag(name, attributes, options, is_highlighted, output);

            if children.is_empty() {
                // Self-closing
                if !is_highlighted && options.use_color { output.push_str(ansi::DIM); }
                output.push_str("/>");
                if options.use_color { output.push_str(ansi::RESET); }
                if options.pretty_print { output.push('\n'); }
            } else if children.len() == 1 && matches!(&children[0], XmlNode::Text(_)) {
                // Single text child — inline
                if !is_highlighted && options.use_color { output.push_str(ansi::DIM); }
                output.push('>');
                if is_highlighted && options.use_color { output.push_str(ansi::RESET); }

                if let XmlNode::Text(text) = &children[0] {
                    output.push_str(&escape_xml(text));
                }

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
                if options.use_color { output.push_str(ansi::RESET); }
                if options.pretty_print { output.push('\n'); }
            } else if truncate_children {
                let child_count = count_xml_node_descendants(node);
                if !is_highlighted && options.use_color { output.push_str(ansi::DIM); }
                output.push('>');
                if options.use_color { output.push_str(ansi::RESET); }
                if options.pretty_print {
                    output.push('\n');
                    let child_indent = options.indent.repeat(depth + 1);
                    output.push_str(&child_indent);
                    if options.use_color { output.push_str(ansi::DIM); }
                    output.push_str(&format!("<!-- ... ({} children) -->\n", child_count));
                    if options.use_color { output.push_str(ansi::RESET); }
                }

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
                if options.use_color { output.push_str(ansi::RESET); }
                if options.pretty_print { output.push('\n'); }
            } else {
                // Multiple children
                if !is_highlighted && options.use_color { output.push_str(ansi::DIM); }
                output.push('>');
                if options.use_color { output.push_str(ansi::RESET); }
                if options.pretty_print { output.push('\n'); }

                for child in children {
                    render_xml_node_recursive(child, options, depth + 1, output);
                }

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
                if options.use_color { output.push_str(ansi::RESET); }
                if options.pretty_print { output.push('\n'); }
            }
        }
        XmlNode::Text(text) => {
            if options.pretty_print {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    output.push_str(&indent);
                    output.push_str(&escape_xml(trimmed));
                    output.push('\n');
                }
            } else {
                output.push_str(&escape_xml(text));
            }
        }
        XmlNode::Comment(text) => {
            output.push_str(&indent);
            if options.use_color { output.push_str(ansi::DIM); }
            output.push_str("<!--");
            output.push_str(text);
            output.push_str("-->");
            if options.use_color { output.push_str(ansi::RESET); }
            if options.pretty_print { output.push('\n'); }
        }
        XmlNode::ProcessingInstruction { target, data } => {
            output.push_str(&indent);
            if options.use_color { output.push_str(ansi::DIM); }
            output.push_str("<?");
            output.push_str(target);
            if let Some(d) = data {
                output.push(' ');
                output.push_str(d);
            }
            output.push_str("?>");
            if options.use_color { output.push_str(ansi::RESET); }
            if options.pretty_print { output.push('\n'); }
        }
        // XPath data variants — render as pretty-printed JSON
        XmlNode::Map { .. } | XmlNode::Array { .. } | XmlNode::Number(_)
        | XmlNode::Boolean(_) | XmlNode::Null => {
            let json_val = crate::output::xml_node_to_json(node, options.max_depth);
            let rendered = serde_json::to_string_pretty(&json_val).unwrap_or_default();
            if options.pretty_print {
                for line in rendered.lines() {
                    output.push_str(&indent);
                    output.push_str(line);
                    output.push('\n');
                }
            } else {
                output.push_str(&rendered);
            }
        }
    }
}

fn render_xml_node_open_tag(
    name: &str,
    attributes: &[(String, String)],
    options: &RenderOptions,
    is_highlighted: bool,
    output: &mut String,
) {
    if !is_highlighted && options.use_color { output.push_str(ansi::DIM); }
    output.push('<');
    if !is_highlighted && options.use_color { output.push_str(ansi::RESET); }

    if !is_highlighted && options.use_color { output.push_str(ansi::BLUE); }
    output.push_str(name);
    if !is_highlighted && options.use_color { output.push_str(ansi::RESET); }

    for (attr_name, attr_value) in attributes {
        if !options.include_meta {
            if matches!(
                attr_name.as_str(),
                "line" | "column" | "end_line" | "end_column"
                | "kind" | "field" | "list"
            ) {
                continue;
            }
        }

        output.push(' ');
        if !is_highlighted && options.use_color { output.push_str(ansi::CYAN); }
        output.push_str(attr_name);
        if !is_highlighted && options.use_color { output.push_str(ansi::RESET); }

        if !is_highlighted && options.use_color { output.push_str(ansi::DIM); }
        output.push_str("=\"");
        if !is_highlighted && options.use_color { output.push_str(ansi::RESET); }

        if !is_highlighted && options.use_color { output.push_str(ansi::YELLOW); }
        output.push_str(&escape_xml(attr_value));
        if !is_highlighted && options.use_color { output.push_str(ansi::RESET); }

        if !is_highlighted && options.use_color { output.push_str(ansi::DIM); }
        output.push('"');
        if !is_highlighted && options.use_color { output.push_str(ansi::RESET); }
    }
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
            .with_meta(false);

        assert!(opts.use_color);
        assert!(!opts.include_meta);
    }

    #[test]
    fn test_max_depth_truncation() {
        let xml = r#"<root><a><b><c>text</c></b></a></root>"#;
        let opts = RenderOptions::new()
            .with_max_depth(Some(1));  // root=0, a=1, b=2 (truncated)

        let output = render_xml_string(xml, &opts);

        // Should contain truncation comment with count
        assert!(output.contains("<!-- ... (2 children) -->"));
        // Should not contain the deeply nested elements
        assert!(!output.contains("<b>"));
        assert!(!output.contains("<c>"));
    }
}

// ---------------------------------------------------------------------------
// XmlNode → compact XML string (no pretty-printing, no colors)
// ---------------------------------------------------------------------------

/// Serialize an XmlNode to a compact XML string (no formatting, no colors).
///
/// Used by the report serializer to emit `tree` as an XML string in
/// structured output formats (JSON/YAML snapshots, etc.).
pub fn xml_node_to_string(node: &XmlNode) -> String {
    let mut out = String::new();
    write_xml_compact(node, &mut out);
    out
}

fn write_xml_compact(node: &XmlNode, out: &mut String) {
    match node {
        XmlNode::Element { name, attributes, children } => {
            out.push('<');
            out.push_str(name);
            for (k, v) in attributes {
                out.push(' ');
                out.push_str(k);
                out.push_str("=\"");
                out.push_str(&escape_xml_attr(v));
                out.push('"');
            }
            if children.is_empty() {
                out.push_str("/>");
            } else {
                out.push('>');
                for child in children {
                    write_xml_compact(child, out);
                }
                out.push_str("</");
                out.push_str(name);
                out.push('>');
            }
        }
        XmlNode::Text(text) => {
            out.push_str(&escape_xml_text(text));
        }
        XmlNode::Comment(text) => {
            out.push_str("<!--");
            out.push_str(text);
            out.push_str("-->");
        }
        XmlNode::ProcessingInstruction { target, data } => {
            out.push_str("<?");
            out.push_str(target);
            if let Some(d) = data {
                out.push(' ');
                out.push_str(d);
            }
            out.push_str("?>");
        }
        // XPath data variants — compact JSON
        XmlNode::Map { .. } | XmlNode::Array { .. } | XmlNode::Number(_)
        | XmlNode::Boolean(_) | XmlNode::Null => {
            let json_val = crate::output::xml_node_to_json(node, None);
            out.push_str(&serde_json::to_string(&json_val).unwrap_or_default());
        }
    }
}

fn escape_xml_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_xml_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
