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

/// Extract and decode the full content of a JSON string node for data view.
///
/// TreeSitter splits JSON strings into `string_content` and `escape_sequence`
/// child nodes. This function reassembles them, decoding escape sequences.
pub(crate) fn extract_decoded_string_content(xot: &Xot, string_node: XotNode) -> Option<String> {
    let mut result = String::new();
    let mut found_content = false;
    for child in xot.children(string_node) {
        if let Some(name) = get_element_name(xot, child) {
            match name.as_str() {
                "string_content" => {
                    if let Some(text) = get_text_content(xot, child) {
                        result.push_str(&text);
                        found_content = true;
                    }
                }
                "escape_sequence" => {
                    if let Some(text) = get_text_content(xot, child) {
                        result.push_str(&decode_json_escapes(&text));
                        found_content = true;
                    }
                }
                _ => {}
            }
        }
    }
    if found_content { Some(result) } else { None }
}

/// Decode JSON string escape sequences into their actual characters.
///
/// Handles: `\\`, `\"`, `\/`, `\b`, `\f`, `\n`, `\r`, `\t`, `\uXXXX`
pub(crate) fn decode_json_escapes(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('\\') => result.push('\\'),
                Some('"') => result.push('"'),
                Some('/') => result.push('/'),
                Some('b') => result.push('\u{0008}'),
                Some('f') => result.push('\u{000C}'),
                Some('n') => result.push('\n'),
                Some('r') => result.push('\r'),
                Some('t') => result.push('\t'),
                Some('u') => {
                    let hex: String = chars.by_ref().take(4).collect();
                    if let Ok(code) = u32::from_str_radix(&hex, 16) {
                        if let Some(ch) = char::from_u32(code) {
                            result.push(ch);
                        }
                    }
                }
                Some(other) => {
                    result.push('\\');
                    result.push(other);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(c);
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
    fn test_decode_json_escapes() {
        assert_eq!(decode_json_escapes("hello"), "hello");
        assert_eq!(decode_json_escapes(r"hello\nworld"), "hello\nworld");
        assert_eq!(decode_json_escapes(r"tab\there"), "tab\there");
        assert_eq!(decode_json_escapes(r"back\\slash"), "back\\slash");
        assert_eq!(decode_json_escapes(r#"say \"hi\""#), "say \"hi\"");
        assert_eq!(decode_json_escapes(r"\u0041"), "A");
        assert_eq!(decode_json_escapes(r"a\u00e9b"), "a\u{00e9}b"); // é
    }

}
