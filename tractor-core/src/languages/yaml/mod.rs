//! YAML transform logic
//!
//! Provides two transforms for dual-branch output:
//! - `syntax`: Normalizes TreeSitter YAML nodes into a unified syntax vocabulary
//!   (object/array/property/key/value/string/number/bool/null)
//! - `data`: Projects into query-friendly data view where mapping keys
//!   become element names and scalar values become text content.
//!
//! Data view example:
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
//! Queryable as: `//data/foo/bar[.='baz']`

pub mod syntax;
pub mod data;

use xot::{Xot, Node as XotNode};
use crate::xot_transform::TransformAction;
use crate::output::syntax_highlight::SyntaxCategory;

pub use syntax::syntax_transform;
pub use data::data_transform;

/// Backwards-compatible alias for the syntax transform
pub fn ast_transform(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    syntax_transform(xot, node)
}

// =============================================================================
// Shared helpers used by both syntax and data transforms
// =============================================================================

/// Strip surrounding quotes from a string
pub(crate) fn strip_quotes(s: &str) -> String {
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

/// Strip quotes from a quoted scalar node's text content
pub(crate) fn strip_quotes_from_node(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
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
pub(crate) fn normalize_block_scalar(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
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

    let lines: Vec<&str> = raw_text.lines().collect();
    if lines.len() <= 1 {
        return Ok(());
    }

    let content_lines = &lines[1..];
    let result = if content_lines.is_empty() {
        String::new()
    } else {
        let min_indent = content_lines
            .iter()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.len() - l.trim_start().len())
            .min()
            .unwrap_or(0);

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

    let all_children: Vec<XotNode> = xot.children(node).collect();
    for c in all_children {
        xot.detach(c)?;
    }
    let text_node = xot.new_text(&result);
    xot.append(node, text_node)?;
    Ok(())
}

/// Sanitize a string to be a valid XML element name
pub(crate) fn sanitize_xml_name(name: &str) -> String {
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
