//! JSON transform logic
//!
//! Provides two transforms for dual-branch output:
//! - `syntax`: Normalizes TreeSitter JSON nodes into a unified syntax vocabulary
//!   (object/array/property/key/value/string/number/bool/null)
//! - `data`: Projects into query-friendly data view where object keys
//!   become element names and scalar values become text content.

pub mod syntax;
pub mod data;

use xot::{Xot, Node as XotNode};
use crate::xot_transform::helpers::*;
use crate::output::syntax_highlight::SyntaxCategory;

pub use syntax::syntax_transform;
pub use data::data_transform;

/// Backwards-compatible alias for the syntax transform
pub fn ast_transform(xot: &mut Xot, node: XotNode) -> Result<crate::xot_transform::TransformAction, xot::Error> {
    syntax_transform(xot, node)
}

/// Extract the text content from a string node's string_content child
pub(crate) fn extract_string_content(xot: &Xot, string_node: XotNode) -> Option<String> {
    for child in xot.children(string_node) {
        if let Some(name) = get_element_name(xot, child) {
            if name == "string_content" {
                return get_text_content(xot, child);
            }
        }
    }
    None
}

/// Sanitize a string to be a valid XML element name.
/// Replaces invalid characters with underscores.
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

/// Map element names to syntax categories for highlighting
pub fn syntax_category(element: &str) -> SyntaxCategory {
    match element {
        "object" | "array" => SyntaxCategory::Keyword,
        "string" => SyntaxCategory::String,
        "number" => SyntaxCategory::Number,
        "bool" | "null" => SyntaxCategory::Keyword,
        "property" | "key" | "value" => SyntaxCategory::Default,
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
}
