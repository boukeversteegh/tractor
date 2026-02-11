//! YAML transform logic
//!
//! Maps the YAML data structure (not syntax tree) to XML elements.
//! Mapping keys become element names, scalar values become text content,
//! and sequence items become `<item>` elements.
//!
//! Example:
//! ```yaml
//! foo:
//!   bar: baz
//! ```
//! Becomes:
//! ```xml
//! <foo>
//!   <bar>baz</bar>
//! </foo>
//! ```
//! Queryable as: `/foo/bar[.='baz']`

use xot::{Xot, Node as XotNode};
use crate::xot_transform::{TransformAction, helpers::*};
use crate::output::syntax_highlight::SyntaxCategory;

/// Transform a YAML AST node into a data-structure-oriented XML tree
pub fn transform(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let kind = match get_element_name(xot, node) {
        Some(k) => k,
        None => return Ok(TransformAction::Continue),
    };

    match kind.as_str() {
        // Mapping pairs: rename to the key text
        "block_mapping_pair" | "flow_pair" => {
            transform_mapping_pair(xot, node)
        }

        // Sequence items: rename to "item"
        "block_sequence_item" => {
            rename(xot, node, "item");
            remove_text_children(xot, node)?;
            Ok(TransformAction::Continue)
        }

        // Flow sequences: wrap each flow_node child as <item>, remove punctuation
        "flow_sequence" => {
            transform_flow_sequence(xot, node)
        }

        // Quoted scalars: strip quotes and promote text
        "double_quote_scalar" => {
            strip_quotes_from_node(xot, node)?;
            Ok(TransformAction::Flatten)
        }
        "single_quote_scalar" => {
            strip_quotes_from_node(xot, node)?;
            Ok(TransformAction::Flatten)
        }

        // Block scalars (| or >): strip indicator and normalize
        "block_scalar" => {
            normalize_block_scalar(xot, node)?;
            Ok(TransformAction::Flatten)
        }

        // Anchors/tags: flatten (walks children first, then promotes)
        "anchor" | "tag" => {
            remove_text_children(xot, node)?;
            Ok(TransformAction::Flatten)
        }

        // Anchor/alias names: flatten to promote text
        "alias_name" | "anchor_name" => {
            Ok(TransformAction::Flatten)
        }

        // Aliases: flatten (promotes children)
        "alias" => {
            remove_text_children(xot, node)?;
            Ok(TransformAction::Flatten)
        }

        // Wrapper nodes to flatten (remove wrapper, promote children)
        // Use Flatten (not Skip) to avoid xot text node consolidation panics
        // when nested nodes have text siblings
        "stream" | "document" | "block_node" | "block_mapping"
        | "flow_mapping" | "value" | "flow_node" | "plain_scalar"
        | "block_sequence" => {
            remove_text_children(xot, node)?;
            Ok(TransformAction::Flatten)
        }

        // Scalar values: flatten to promote text content to parent
        "string_scalar" | "integer_scalar" | "float_scalar"
        | "boolean_scalar" | "null_scalar" => {
            Ok(TransformAction::Flatten)
        }

        // Comments: remove entirely
        "comment" => {
            remove_text_children(xot, node)?;
            Ok(TransformAction::Flatten)
        }

        _ => Ok(TransformAction::Continue),
    }
}

/// Transform a mapping pair by extracting the key and renaming the element
fn transform_mapping_pair(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    if let Some(key) = extract_key_text(xot, node) {
        let safe_name = sanitize_xml_name(&key);
        rename(xot, node, &safe_name);

        // Remove key-related children and colon text
        let children: Vec<XotNode> = xot.children(node).collect();
        for child in children {
            if xot.text_str(child).is_some() {
                // Remove ":" and whitespace text nodes
                xot.detach(child)?;
            } else if let Some(field) = get_attr(xot, child, "field") {
                if field == "key" {
                    xot.detach(child)?;
                }
            }
        }
    }
    Ok(TransformAction::Continue)
}

/// Extract the key text from a mapping pair's key flow_node
fn extract_key_text(xot: &Xot, mapping_pair: XotNode) -> Option<String> {
    for child in xot.children(mapping_pair) {
        if let Some(field) = get_attr(xot, child, "field") {
            if field == "key" {
                return collect_deep_text(xot, child);
            }
        }
    }
    None
}

/// Recursively collect text content from a node tree
fn collect_deep_text(xot: &Xot, node: XotNode) -> Option<String> {
    // Check direct text children
    let mut text = String::new();
    for child in xot.children(node) {
        if let Some(t) = xot.text_str(child) {
            text.push_str(t);
        }
    }
    let trimmed = text.trim().to_string();
    if !trimmed.is_empty() {
        // Strip quotes if present
        let stripped = strip_quotes(&trimmed);
        return Some(stripped);
    }

    // Recurse into element children
    for child in xot.children(node) {
        if xot.element(child).is_some() {
            if let Some(t) = collect_deep_text(xot, child) {
                return Some(t);
            }
        }
    }
    None
}

/// Strip surrounding quotes from a string
fn strip_quotes(s: &str) -> String {
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

/// Transform a flow sequence by renaming flow_node children to "item"
fn transform_flow_sequence(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    // Remove punctuation text ([, ,, ])
    remove_text_children(xot, node)?;

    // Rename flow_node children to "item"
    let children: Vec<XotNode> = xot.children(node)
        .filter(|&c| xot.element(c).is_some())
        .collect();
    for child in children {
        if let Some(name) = get_element_name(xot, child) {
            if name == "flow_node" {
                rename(xot, child, "item");
            }
        }
    }

    Ok(TransformAction::Flatten)
}

/// Strip quotes from a quoted scalar node's text content
fn strip_quotes_from_node(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    let children: Vec<XotNode> = xot.children(node).collect();
    for child in children {
        let new_content = xot.text_str(child).map(|text| strip_quotes(text.trim()));
        if let Some(content) = new_content {
            let all_children: Vec<XotNode> = xot.children(node).collect();
            for c in all_children {
                xot.detach(c)?;
            }
            let text_node = xot.new_text(&content);
            xot.append(node, text_node)?;
            return Ok(());
        }
    }
    Ok(())
}

/// Normalize block scalar content (strip | or > indicator and un-indent)
fn normalize_block_scalar(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    // Collect text from children
    let raw_text = {
        let children: Vec<XotNode> = xot.children(node).collect();
        let mut text = String::new();
        for child in &children {
            if let Some(t) = xot.text_str(*child) {
                text.push_str(t);
            }
        }
        text
    };

    if raw_text.is_empty() {
        return Ok(());
    }

    // Block scalars start with | or > (possibly followed by a number for indent)
    // Split into lines, skip the indicator line, and un-indent
    let lines: Vec<&str> = raw_text.lines().collect();
    if lines.len() <= 1 {
        return Ok(());
    }

    let content_lines = &lines[1..];
    let result = if content_lines.is_empty() {
        String::new()
    } else {
        // Find minimum indentation
        let min_indent = content_lines
            .iter()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.len() - l.trim_start().len())
            .min()
            .unwrap_or(0);

        // Strip common indentation
        let normalized: Vec<&str> = content_lines
            .iter()
            .map(|l| {
                if l.len() >= min_indent {
                    &l[min_indent..]
                } else {
                    l.trim()
                }
            })
            .collect();

        normalized.join("\n")
    };

    // Replace all children with new text
    let all_children: Vec<XotNode> = xot.children(node).collect();
    for c in all_children {
        xot.detach(c)?;
    }
    let text_node = xot.new_text(&result);
    xot.append(node, text_node)?;
    Ok(())
}

/// Sanitize a string to be a valid XML element name
fn sanitize_xml_name(name: &str) -> String {
    if name.is_empty() {
        return "_".to_string();
    }

    let mut result = String::with_capacity(name.len());
    for (i, c) in name.chars().enumerate() {
        if i == 0 {
            if c.is_ascii_alphabetic() || c == '_' {
                result.push(c);
            } else {
                result.push('_');
                if c.is_ascii_alphanumeric() || c == '-' || c == '.' {
                    result.push(c);
                }
            }
        } else if c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' {
            result.push(c);
        } else {
            result.push('_');
        }
    }
    result
}

/// Map a transformed element name to a syntax category for highlighting
pub fn syntax_category(element: &str) -> SyntaxCategory {
    match element {
        "item" => SyntaxCategory::Keyword,
        _ => SyntaxCategory::Default,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_xml_name() {
        assert_eq!(sanitize_xml_name("foo"), "foo");
        assert_eq!(sanitize_xml_name("foo_bar"), "foo_bar");
        assert_eq!(sanitize_xml_name("foo-bar"), "foo-bar");
        assert_eq!(sanitize_xml_name("foo.bar"), "foo.bar");
        assert_eq!(sanitize_xml_name("123"), "_123");
        assert_eq!(sanitize_xml_name("key with spaces"), "key_with_spaces");
        assert_eq!(sanitize_xml_name(""), "_");
        assert_eq!(sanitize_xml_name("-hyphen"), "_-hyphen");
    }

    #[test]
    fn test_strip_quotes() {
        assert_eq!(strip_quotes("\"hello\""), "hello");
        assert_eq!(strip_quotes("'world'"), "world");
        assert_eq!(strip_quotes("plain"), "plain");
        assert_eq!(strip_quotes("\"\""), "");
    }
}
