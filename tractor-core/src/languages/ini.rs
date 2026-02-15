//! INI transform logic
//!
//! Maps the INI data structure to XML elements.
//! Section headers become named elements, settings become named child elements
//! with their values as text content.
//!
//! Example:
//! ```ini
//! [database]
//! host = localhost
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

/// Transform an INI AST node into a data-structure-oriented XML tree
pub fn transform(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let kind = match get_element_name(xot, node) {
        Some(k) => k,
        None => return Ok(TransformAction::Continue),
    };

    match kind.as_str() {
        // Sections: extract name from section_name child, rename to that
        "section" => {
            transform_section(xot, node)
        }

        // Section name: remove after parent extracts it
        "section_name" => {
            // Detached by parent transform; if still here, flatten
            remove_text_children(xot, node)?;
            Ok(TransformAction::Flatten)
        }

        // Settings (key=value pairs): rename to the key text
        "setting" => {
            transform_setting(xot, node)
        }

        // Setting name and value: flatten to promote text
        "setting_name" | "setting_value" => {
            Ok(TransformAction::Flatten)
        }

        // Text nodes inside section_name or comment: flatten
        "text" => {
            Ok(TransformAction::Flatten)
        }

        // Comments: keep as <comment> with text content, strip # or ; prefix
        "comment" => {
            transform_comment(xot, node)
        }

        // Document root: clean up text children
        "document" => {
            remove_text_children(xot, node)?;
            Ok(TransformAction::Continue)
        }

        _ => Ok(TransformAction::Continue),
    }
}

/// Transform a section by extracting the name from its section_name child
fn transform_section(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    if let Some(name) = extract_section_name(xot, node) {
        let safe_name = sanitize_xml_name(&name);
        rename(xot, node, &safe_name);

        // Remove the section_name child (already extracted)
        let children: Vec<XotNode> = xot.children(node).collect();
        for child in children {
            if xot.text_str(child).is_some() {
                xot.detach(child)?;
            } else if let Some(child_name) = get_element_name(xot, child) {
                if child_name == "section_name" {
                    xot.detach(child)?;
                }
            }
        }

        // If name was sanitized, store original key as attribute
        if safe_name != name {
            set_attr(xot, node, "key", &name);
        }
    }
    Ok(TransformAction::Continue)
}

/// Transform a setting by extracting the key name and promoting the value
fn transform_setting(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    if let Some(key) = extract_setting_name(xot, node) {
        let safe_name = sanitize_xml_name(&key);
        rename(xot, node, &safe_name);

        // Remove setting_name child and `=` text, keep setting_value
        let children: Vec<XotNode> = xot.children(node).collect();
        for child in children {
            if xot.text_str(child).is_some() {
                // Remove "=" and whitespace text
                xot.detach(child)?;
            } else if let Some(child_name) = get_element_name(xot, child) {
                if child_name == "setting_name" {
                    xot.detach(child)?;
                }
            }
        }

        // Trim the setting_value text content
        trim_value_text(xot, node)?;

        // If name was sanitized, store original key as attribute
        if safe_name != key {
            set_attr(xot, node, "key", &key);
        }
    }
    Ok(TransformAction::Continue)
}

/// Extract the section name from a section node's section_name child.
/// The section_name node contains: `[`, `<text>name</text>`, `]`.
/// We extract the text from the `text` element child.
fn extract_section_name(xot: &Xot, node: XotNode) -> Option<String> {
    for child in xot.children(node) {
        if let Some(name) = get_element_name(xot, child) {
            if name == "section_name" {
                // Find the `text` element child inside section_name
                for grandchild in xot.children(child) {
                    if let Some(gname) = get_element_name(xot, grandchild) {
                        if gname == "text" {
                            if let Some(text) = get_text_content(xot, grandchild) {
                                let trimmed = text.trim().to_string();
                                if !trimmed.is_empty() {
                                    return Some(trimmed);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// Extract the setting name from a setting node's setting_name child
fn extract_setting_name(xot: &Xot, node: XotNode) -> Option<String> {
    for child in xot.children(node) {
        if let Some(name) = get_element_name(xot, child) {
            if name == "setting_name" {
                return get_text_content(xot, child).map(|s| s.trim().to_string());
            }
        }
    }
    None
}

/// Transform a comment by extracting its text content.
/// The comment node contains: `#` or `;` text, then `<text>comment text</text>`.
fn transform_comment(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    // Extract comment text from the `text` element child
    let mut comment_text = None;
    for child in xot.children(node) {
        if let Some(name) = get_element_name(xot, child) {
            if name == "text" {
                comment_text = get_text_content(xot, child).map(|s| s.trim().to_string());
            }
        }
    }

    // Remove all children and replace with trimmed text
    let all_children: Vec<XotNode> = xot.children(node).collect();
    for c in all_children {
        xot.detach(c)?;
    }
    if let Some(text) = comment_text {
        let text_node = xot.new_text(&text);
        xot.append(node, text_node)?;
    }

    rename(xot, node, "comment");
    Ok(TransformAction::Done)
}

/// Trim whitespace from setting_value text within a node
fn trim_value_text(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    // Find setting_value children and trim their text
    let children: Vec<XotNode> = xot.children(node).collect();
    for child in children {
        if let Some(name) = get_element_name(xot, child) {
            if name == "setting_value" {
                // Get and trim the text content
                if let Some(text) = get_text_content(xot, child) {
                    let trimmed = text.trim().to_string();
                    let all: Vec<XotNode> = xot.children(child).collect();
                    for c in all {
                        xot.detach(c)?;
                    }
                    let text_node = xot.new_text(&trimmed);
                    xot.append(child, text_node)?;
                }
            }
        }
    }
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
        "comment" => SyntaxCategory::Comment,
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
}
