//! Raw TreeSitter XML output mode
//!
//! Emits complete tree-sitter AST as XML with mixed content:
//! - Named nodes become XML elements
//! - Anonymous nodes (operators, keywords, punctuation) become text content

use std::io::Write;

/// Write TreeSitter node to XML in raw format
pub fn write_node(out: &mut impl Write, node: tree_sitter::Node, source: &str, indent: usize, use_color: bool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    write_node_with_field(out, node, source, indent, use_color, None)
}

/// Write TreeSitter node with optional field name from parent
fn write_node_with_field(out: &mut impl Write, node: tree_sitter::Node, source: &str, indent: usize, use_color: bool, field_name: Option<&str>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let indent_str = "  ".repeat(indent);
    let kind = node.kind();

    // Anonymous nodes are emitted as text content by the parent
    if !node.is_named() {
        return Ok(());
    }

    let start = node.start_position();
    let end = node.end_position();

    // Build attributes including optional field name
    let start_line = (start.row + 1).to_string();
    let start_col = (start.column + 1).to_string();
    let end_line = (end.row + 1).to_string();
    let end_col = (end.column + 1).to_string();

    let mut attrs: Vec<(&str, &str)> = vec![
        ("startLine", &start_line),
        ("startCol", &start_col),
        ("endLine", &end_line),
        ("endCol", &end_col),
    ];

    if let Some(field) = field_name {
        attrs.push(("field", field));
    }

    // Check if this is a leaf node (no children at all)
    let child_count = node.child_count();

    if child_count == 0 {
        // Leaf node - include text content
        let text = node.utf8_text(source.as_bytes()).unwrap_or("");
        write!(out, "{}", indent_str)?;
        write_element_with_attrs_and_text(out, kind, &attrs, Some(text), use_color)?;
        writeln!(out)?;
    } else {
        // Node with children - emit all children (named as elements, anonymous as text)
        write!(out, "{}", indent_str)?;
        write_element_open_with_attrs(out, kind, &attrs, use_color)?;

        // Recurse into ALL children, emitting anonymous nodes as text
        let mut cursor = node.walk();
        cursor.goto_first_child();
        loop {
            let child = cursor.node();
            if child.is_named() {
                // Named node - recurse
                writeln!(out)?;
                let child_field = cursor.field_name();
                write_node_with_field(out, child, source, indent + 1, use_color, child_field)?;
            } else {
                // Anonymous node - emit as text content
                let text = child.utf8_text(source.as_bytes()).unwrap_or("");
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    write_text_content(out, trimmed)?;
                }
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }

        writeln!(out)?;
        write_tag_close(out, kind, indent, use_color)?;
    }

    Ok(())
}

/// Write text content, using CDATA if it contains special characters
fn write_text_content(out: &mut impl Write, text: &str) -> std::io::Result<()> {
    if text.contains('<') || text.contains('>') || text.contains('&') {
        write!(out, "<![CDATA[{}]]>", text)
    } else {
        write!(out, "{}", text)
    }
}

// Helper functions for XML output

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
     .replace('<', "&lt;")
     .replace('>', "&gt;")
     .replace('"', "&quot;")
}

fn write_element_open_with_attrs(out: &mut impl Write, name: &str, attrs: &[(&str, &str)], _use_color: bool) -> std::io::Result<()> {
    write!(out, "<{}", escape_xml(name))?;
    for (attr_name, attr_value) in attrs {
        write!(out, " {}=\"{}\"", attr_name, escape_xml(attr_value))?;
    }
    write!(out, ">")?;
    Ok(())
}

fn write_element_with_attrs_and_text(out: &mut impl Write, name: &str, attrs: &[(&str, &str)], text: Option<&str>, _use_color: bool) -> std::io::Result<()> {
    let escaped_name = escape_xml(name);
    write!(out, "<{}", escaped_name)?;
    for (attr_name, attr_value) in attrs {
        write!(out, " {}=\"{}\"", attr_name, escape_xml(attr_value))?;
    }
    write!(out, ">")?;
    if let Some(t) = text {
        write!(out, "{}", escape_xml(t))?;
    }
    write!(out, "</{}>", escaped_name)?;
    Ok(())
}

fn write_tag_close(out: &mut impl Write, name: &str, indent: usize, _use_color: bool) -> std::io::Result<()> {
    let indent_str = "  ".repeat(indent);
    writeln!(out, "{}</{}>", indent_str, name)
}
