//! .env file transform logic
//!
//! Uses the bash grammar to parse .env files, then transforms the AST
//! into a clean key-value structure. Comments are preserved.
//!
//! Example:
//! ```env
//! # Database config
//! DB_HOST=localhost
//! DB_PORT=5432
//! SECRET="my-secret"
//! export API_URL=https://example.com
//! ```
//! Becomes:
//! ```xml
//! <document>
//!   <comment>Database config</comment>
//!   <DB_HOST>localhost</DB_HOST>
//!   <DB_PORT>5432</DB_PORT>
//!   <SECRET>my-secret</SECRET>
//!   <API_URL>https://example.com</API_URL>
//! </document>
//! ```
//! Queryable as: `//DB_HOST[.='localhost']`

use xot::{Xot, Node as XotNode};
use crate::xot_transform::{TransformAction, helpers::*};
use crate::output::syntax_highlight::SyntaxCategory;

/// Transform a bash AST node into an env-file-oriented XML tree
pub fn transform(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let kind = match get_element_name(xot, node) {
        Some(k) => k,
        None => return Ok(TransformAction::Continue),
    };

    match kind.as_str() {
        // Root program node: rename to "document", remove text children
        "program" => {
            rename(xot, node, "document");
            remove_text_children(xot, node)?;
            Ok(TransformAction::Continue)
        }

        // Variable assignments: rename to the variable name
        "variable_assignment" => {
            transform_variable_assignment(xot, node)
        }

        // export KEY=VALUE: flatten to expose the inner variable_assignment
        "declaration_command" => {
            remove_text_children(xot, node)?;
            Ok(TransformAction::Flatten)
        }

        // Comments: keep as <comment>, strip the # prefix
        "comment" => {
            transform_comment(xot, node)
        }

        // Variable name: flatten to promote text to parent
        "variable_name" => {
            Ok(TransformAction::Flatten)
        }

        // Value wrapper nodes: flatten
        "word" | "number" | "raw_string" | "ansii_c_string" => {
            Ok(TransformAction::Flatten)
        }

        // Strings: strip quotes and flatten
        "string" | "simple_expansion" | "expansion" => {
            Ok(TransformAction::Flatten)
        }

        // String content: flatten to promote text
        "string_content" => {
            Ok(TransformAction::Flatten)
        }

        // Concatenation: flatten to combine parts
        "concatenation" => {
            remove_text_children(xot, node)?;
            Ok(TransformAction::Flatten)
        }

        _ => Ok(TransformAction::Continue),
    }
}

/// Transform a variable_assignment node: extract key name and value,
/// rebuild as `<KEY>value</KEY>`.
///
/// The bash AST structure is:
/// ```xml
/// <variable_assignment>
///   <name><variable_name>KEY</variable_name></name>
///   =
///   <value><word>val</word></value>
/// </variable_assignment>
/// ```
fn transform_variable_assignment(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let var_name = extract_variable_name(xot, node);
    let var_value = extract_variable_value(xot, node);

    if let Some(name) = var_name {
        let safe_name = sanitize_xml_name(&name);
        rename(xot, node, &safe_name);

        // Remove all children and replace with value text
        let children: Vec<XotNode> = xot.children(node).collect();
        for child in children {
            xot.detach(child)?;
        }

        if let Some(value) = var_value {
            let text_node = xot.new_text(&value);
            xot.append(node, text_node)?;
        }

        // If name was sanitized, store original key as attribute
        if safe_name != name {
            set_attr(xot, node, "key", &name);
        }
    }
    Ok(TransformAction::Done)
}

/// Extract the variable name from a variable_assignment node.
/// Looks inside the `name` field wrapper for `variable_name`.
fn extract_variable_name(xot: &Xot, node: XotNode) -> Option<String> {
    for child in xot.children(node) {
        if let Some(child_name) = get_element_name(xot, child) {
            // Direct variable_name child
            if child_name == "variable_name" {
                return get_text_content(xot, child).map(|s| s.trim().to_string());
            }
            // Field wrapper: <name><variable_name>KEY</variable_name></name>
            if child_name == "name" {
                for grandchild in xot.children(child) {
                    if let Some(gname) = get_element_name(xot, grandchild) {
                        if gname == "variable_name" {
                            return get_text_content(xot, grandchild).map(|s| s.trim().to_string());
                        }
                    }
                }
                // Fallback: text content of name wrapper itself
                return get_text_content(xot, child).map(|s| s.trim().to_string());
            }
        }
    }
    None
}

/// Extract the value from a variable_assignment node.
/// Handles plain words, numbers, and quoted strings.
///
/// AST structures:
/// - `<value><word>text</word></value>` → "text"
/// - `<value><number>123</number></value>` → "123"
/// - `<value><string>"<string_content>text</string_content>"</string></value>` → "text"
fn extract_variable_value(xot: &Xot, node: XotNode) -> Option<String> {
    for child in xot.children(node) {
        if let Some(child_name) = get_element_name(xot, child) {
            if child_name == "value" {
                return extract_value_content(xot, child);
            }
        }
    }
    None
}

/// Recursively extract text content from a value node, stripping quotes.
fn extract_value_content(xot: &Xot, node: XotNode) -> Option<String> {
    let mut parts = Vec::new();
    collect_value_text(xot, node, &mut parts);
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(""))
    }
}

/// Collect text content from value nodes, skipping quote characters.
fn collect_value_text(xot: &Xot, node: XotNode, parts: &mut Vec<String>) {
    for child in xot.children(node) {
        if let Some(text) = xot.text_str(child) {
            // Skip standalone quote characters
            let trimmed = text.trim();
            if trimmed == "\"" || trimmed == "'" || trimmed == "$" {
                continue;
            }
            if !text.is_empty() {
                parts.push(text.to_string());
            }
        } else if let Some(child_name) = get_element_name(xot, child) {
            match child_name.as_str() {
                // These contain the actual text content
                "word" | "number" | "string_content" | "variable_name" => {
                    if let Some(text) = get_text_content(xot, child) {
                        parts.push(text);
                    }
                }
                // Raw strings (single-quoted): strip surrounding quotes
                "raw_string" => {
                    if let Some(text) = get_text_content(xot, child) {
                        let stripped = text.strip_prefix('\'')
                            .and_then(|s| s.strip_suffix('\''))
                            .unwrap_or(&text);
                        parts.push(stripped.to_string());
                    }
                }
                // Recurse into wrapper/composite nodes
                "string" | "concatenation" | "simple_expansion" | "expansion"
                | "command_substitution" => {
                    collect_value_text(xot, child, parts);
                }
                _ => {
                    // Fallback: try to get text content
                    if let Some(text) = get_text_content(xot, child) {
                        parts.push(text);
                    }
                }
            }
        }
    }
}

/// Transform a comment node: strip the # prefix, keep text
fn transform_comment(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    // The comment node contains text like "# Database config"
    // Strip the leading "# " prefix
    if let Some(text) = get_text_content(xot, node) {
        let stripped = text.strip_prefix('#')
            .unwrap_or(&text)
            .trim_start()
            .to_string();

        // Replace children with stripped text
        let all_children: Vec<XotNode> = xot.children(node).collect();
        for c in all_children {
            xot.detach(c)?;
        }
        let text_node = xot.new_text(&stripped);
        xot.append(node, text_node)?;
    }

    rename(xot, node, "comment");
    Ok(TransformAction::Done)
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
        assert_eq!(sanitize_xml_name("DB_HOST"), "DB_HOST");
        assert_eq!(sanitize_xml_name("foo-bar"), "foo-bar");
        assert_eq!(sanitize_xml_name("123"), "_123");
        assert_eq!(sanitize_xml_name(""), "_");
    }
}
