//! TOML transform logic
//!
//! Maps the TOML data structure to XML elements, similar to the YAML transform.
//! Table/pair keys become element names, scalar values become text content,
//! and array items become `<item>` elements.
//!
//! Example:
//! ```toml
//! [database]
//! host = "localhost"
//! port = 5432
//! ```
//! Becomes:
//! ```xml
//! <database>
//!   <host>localhost</host>
//!   <port>5432</port>
//! </database>
//! ```
//! Queryable as: `//database/host[.='localhost']`

use xot::{Xot, Node as XotNode};
use crate::xot_transform::{TransformAction, helpers::*};
use crate::output::syntax_highlight::SyntaxCategory;

/// Transform a TOML AST node into a data-structure-oriented XML tree
pub fn transform(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let kind = match get_element_name(xot, node) {
        Some(k) => k,
        None => return Ok(TransformAction::Continue),
    };

    match kind.as_str() {
        // Key-value pairs: rename to the key text
        "pair" => {
            transform_pair(xot, node)
        }

        // Table headers [key]: rename to the key text
        "table" => {
            transform_table(xot, node)
        }

        // Table array elements [[key]]: rename to the key text
        "table_array_element" => {
            transform_table_array_element(xot, node)
        }

        // Arrays: remove punctuation, wrap values as <item>
        "array" => {
            transform_array(xot, node)
        }

        // Inline tables: remove punctuation, flatten
        "inline_table" => {
            remove_text_children(xot, node)?;
            Ok(TransformAction::Flatten)
        }

        // Strings: strip quotes and promote text
        "string" => {
            strip_quotes_from_node(xot, node)?;
            Ok(TransformAction::Flatten)
        }

        // Scalar values: flatten to promote text content to parent
        "integer" | "float" | "boolean"
        | "local_date" | "local_date_time" | "local_time"
        | "offset_date_time" => {
            Ok(TransformAction::Flatten)
        }

        // Keys: flatten to promote text
        "bare_key" | "quoted_key" => {
            Ok(TransformAction::Flatten)
        }

        // Dotted keys: flatten to promote text
        "dotted_key" => {
            Ok(TransformAction::Flatten)
        }

        // Comments: remove entirely
        "comment" => {
            remove_text_children(xot, node)?;
            Ok(TransformAction::Flatten)
        }

        // Escape sequences: flatten to promote text
        "escape_sequence" => {
            Ok(TransformAction::Flatten)
        }

        // Document root: clean up text children
        "document" => {
            remove_text_children(xot, node)?;
            Ok(TransformAction::Continue)
        }

        _ => Ok(TransformAction::Continue),
    }
}

/// Transform a pair by extracting the key and renaming the element.
fn transform_pair(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    if let Some(key_info) = extract_pair_key(xot, node) {
        let segments = key_info.segments;

        // Remove key child and `=` text
        let children: Vec<XotNode> = xot.children(node).collect();
        for child in children {
            if xot.text_str(child).is_some() {
                // Remove "=" and whitespace text nodes
                xot.detach(child)?;
            } else if let Some(name) = get_element_name(xot, child) {
                if name == "bare_key" || name == "quoted_key" || name == "dotted_key" {
                    xot.detach(child)?;
                }
            }
        }

        if segments.len() == 1 {
            // Simple key: rename pair to the key name
            let safe_name = sanitize_xml_name(&segments[0]);
            rename(xot, node, &safe_name);

            if safe_name != segments[0] {
                prepend_element_with_text(xot, node, "key", &segments[0])?;
            }
        } else {
            // Dotted key: create nested elements a.b.c → <a><b><c>value</c></b></a>
            let safe_name = sanitize_xml_name(segments.last().unwrap());
            rename(xot, node, &safe_name);

            if safe_name != *segments.last().unwrap() {
                prepend_element_with_text(xot, node, "key", segments.last().unwrap())?;
            }

            // Wrap in parent elements for preceding segments (innermost to outermost)
            wrap_in_nested_elements(xot, node, &segments[..segments.len() - 1])?;
        }
    }
    Ok(TransformAction::Continue)
}

/// Transform a table header [key] by extracting the key and renaming.
fn transform_table(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    if let Some(key_info) = extract_table_key(xot, node) {
        let segments = key_info.segments;

        // Remove bracket text and key children
        let children: Vec<XotNode> = xot.children(node).collect();
        for child in children {
            if xot.text_str(child).is_some() {
                xot.detach(child)?;
            } else if let Some(name) = get_element_name(xot, child) {
                if name == "bare_key" || name == "quoted_key" || name == "dotted_key" {
                    xot.detach(child)?;
                }
            }
        }

        // Rename to last segment
        let safe_name = sanitize_xml_name(segments.last().unwrap());
        rename(xot, node, &safe_name);

        if safe_name != *segments.last().unwrap() {
            prepend_element_with_text(xot, node, "key", segments.last().unwrap())?;
        }

        // For dotted keys, wrap in parent elements
        if segments.len() > 1 {
            wrap_in_nested_elements(xot, node, &segments[..segments.len() - 1])?;
        }
    }
    Ok(TransformAction::Continue)
}

/// Transform a table array element [[key]] by extracting the key and renaming.
fn transform_table_array_element(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    if let Some(key_info) = extract_table_key(xot, node) {
        let segments = key_info.segments;

        // Remove bracket text and key children
        let children: Vec<XotNode> = xot.children(node).collect();
        for child in children {
            if xot.text_str(child).is_some() {
                xot.detach(child)?;
            } else if let Some(name) = get_element_name(xot, child) {
                if name == "bare_key" || name == "quoted_key" || name == "dotted_key" {
                    xot.detach(child)?;
                }
            }
        }

        // Rename to "item" and wrap in element named after the key
        rename(xot, node, "item");

        // Wrap: for [[servers]] → <servers><item>...</item></servers>
        // For [[a.b]] → <a><b><item>...</item></b></a>
        wrap_in_nested_elements(xot, node, &segments)?;
    }
    Ok(TransformAction::Continue)
}

/// Transform an array by removing punctuation and renaming children to "item"
fn transform_array(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    // Remove punctuation text ([, ,, ])
    remove_text_children(xot, node)?;

    // Wrap each element child in an <item> element
    let children: Vec<XotNode> = xot.children(node)
        .filter(|&c| xot.element(c).is_some())
        .collect();

    for child in children {
        // Create an <item> wrapper
        let item_name = xot.add_name("item");
        let item = xot.new_element(item_name);
        xot.insert_before(child, item)?;
        xot.detach(child)?;
        xot.append(item, child)?;
    }

    Ok(TransformAction::Flatten)
}

/// Key information extracted from a TOML node
struct KeyInfo {
    /// Key segments (for dotted keys, multiple segments; otherwise one)
    segments: Vec<String>,
}

/// Extract key information from a pair node.
/// The key is the first bare_key, quoted_key, or dotted_key child.
fn extract_pair_key(xot: &Xot, node: XotNode) -> Option<KeyInfo> {
    for child in xot.children(node) {
        if let Some(name) = get_element_name(xot, child) {
            match name.as_str() {
                "bare_key" => {
                    let text = get_text_content(xot, child)?;
                    return Some(KeyInfo { segments: vec![text.trim().to_string()] });
                }
                "quoted_key" => {
                    let text = get_text_content(xot, child)?;
                    let stripped = strip_quotes(text.trim());
                    return Some(KeyInfo { segments: vec![stripped] });
                }
                "dotted_key" => {
                    return extract_dotted_key_segments(xot, child);
                }
                _ => {}
            }
        }
    }
    None
}

/// Extract key information from a table or table_array_element node.
/// The key is the bare_key, quoted_key, or dotted_key after the bracket text.
fn extract_table_key(xot: &Xot, node: XotNode) -> Option<KeyInfo> {
    // Same extraction logic - find first key child
    extract_pair_key(xot, node)
}

/// Extract segments from a dotted_key node (handles recursive nesting)
fn extract_dotted_key_segments(xot: &Xot, node: XotNode) -> Option<KeyInfo> {
    let mut segments = Vec::new();
    collect_key_segments(xot, node, &mut segments);

    if segments.is_empty() {
        None
    } else {
        Some(KeyInfo { segments })
    }
}

/// Recursively collect key segments from a dotted_key or its children
fn collect_key_segments(xot: &Xot, node: XotNode, segments: &mut Vec<String>) {
    for child in xot.children(node) {
        if let Some(name) = get_element_name(xot, child) {
            match name.as_str() {
                "bare_key" => {
                    if let Some(text) = get_text_content(xot, child) {
                        segments.push(text.trim().to_string());
                    }
                }
                "quoted_key" => {
                    if let Some(text) = get_text_content(xot, child) {
                        segments.push(strip_quotes(text.trim()));
                    }
                }
                "dotted_key" => {
                    // Recurse into nested dotted_key
                    collect_key_segments(xot, child, segments);
                }
                _ => {}
            }
        }
    }
}

/// Wrap a node in nested elements for dotted key segments.
/// For segments ["a", "b"] and node N, creates: <a><b>N</b></a>
fn wrap_in_nested_elements(xot: &mut Xot, node: XotNode, segments: &[String]) -> Result<(), xot::Error> {
    let mut current = node;

    // Wrap from innermost to outermost
    for segment in segments.iter().rev() {
        let safe_name = sanitize_xml_name(segment);
        let wrapper_name = xot.add_name(&safe_name);
        let wrapper = xot.new_element(wrapper_name);
        xot.insert_before(current, wrapper)?;
        xot.detach(current)?;
        xot.append(wrapper, current)?;
        current = wrapper;
    }

    Ok(())
}

/// Strip surrounding quotes from a string
fn strip_quotes(s: &str) -> String {
    // Multi-line strings first (""" or ''')
    if (s.starts_with("\"\"\"") && s.ends_with("\"\"\"")) ||
       (s.starts_with("'''") && s.ends_with("'''")) {
        return s[3..s.len() - 3].trim_start_matches('\n').to_string();
    }
    // Single-line strings
    if (s.starts_with('"') && s.ends_with('"')) ||
       (s.starts_with('\'') && s.ends_with('\'')) {
        return s[1..s.len() - 1].to_string();
    }
    s.to_string()
}

/// Strip quotes from a string node's text content
fn strip_quotes_from_node(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    // Collect all text from children
    let mut raw_text = String::new();
    for child in xot.children(node) {
        if let Some(t) = xot.text_str(child) {
            raw_text.push_str(t);
        }
    }

    if raw_text.is_empty() {
        return Ok(());
    }

    let stripped = strip_quotes(&raw_text);

    // Replace all children with new text
    let all_children: Vec<XotNode> = xot.children(node).collect();
    for c in all_children {
        xot.detach(c)?;
    }
    let text_node = xot.new_text(&stripped);
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
        assert_eq!(sanitize_xml_name("123"), "_123");
        assert_eq!(sanitize_xml_name("key with spaces"), "key_with_spaces");
        assert_eq!(sanitize_xml_name(""), "_");
    }

    #[test]
    fn test_strip_quotes() {
        assert_eq!(strip_quotes("\"hello\""), "hello");
        assert_eq!(strip_quotes("'world'"), "world");
        assert_eq!(strip_quotes("plain"), "plain");
        assert_eq!(strip_quotes("\"\"\"multi\nline\"\"\""), "multi\nline");
        assert_eq!(strip_quotes("'''literal'''"), "literal");
    }
}
