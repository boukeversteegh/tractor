//! JSON transform logic
//!
//! Provides two transforms for dual-branch output:
//! - `ast_transform`: Normalizes TreeSitter JSON nodes into a unified AST vocabulary
//!   (object/array/property/key/value/string/number/bool/null)
//! - `data_transform`: Projects into query-friendly data view where object keys
//!   become element names and scalar values become text content.
//!
//! TreeSitter JSON grammar produces:
//!   document, object, pair (field="key" + <value>), array,
//!   string (with string_content child), number, true, false, null
//!
//! AST view example for `{"name": "John"}`:
//! ```xml
//! <object>
//!   <property>
//!     <key><string>name</string></key>
//!     <value><string>John</string></value>
//!   </property>
//! </object>
//! ```
//!
//! Data view example:
//! ```xml
//! <name>John</name>
//! ```

use xot::{Xot, Node as XotNode};
use crate::xot_transform::{TransformAction, helpers::*};
use crate::output::syntax_highlight::SyntaxCategory;

// =============================================================================
// AST Transform — normalized vocabulary
// =============================================================================

/// Normalize TreeSitter JSON AST into unified vocabulary.
///
/// Mapping:
///   document     → flatten
///   object       → <object> (already correct)
///   array        → <array> (already correct)
///   pair         → <property>, wrap key child in <key>
///   string       → <string>, extract string_content text
///   number       → <number> (already correct)
///   true/false   → <bool>
///   null         → <null> (already correct)
///   string_content → flatten (promote text to parent)
///   punctuation  → remove
pub fn ast_transform(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let kind = match get_kind(xot, node) {
        Some(k) => k,
        None => return Ok(TransformAction::Continue),
    };

    match kind.as_str() {
        // Top-level document wrapper: flatten
        "document" => {
            remove_text_children(xot, node)?;
            Ok(TransformAction::Flatten)
        }

        // object/array: keep name, just remove punctuation
        "object" | "array" => {
            remove_text_children(xot, node)?;
            Ok(TransformAction::Continue)
        }

        // pair → <property>
        // The key child has field="key" attribute, wrap it in a <key> element.
        // The value child is already wrapped in <value> by WRAPPED_FIELDS.
        "pair" => {
            rename(xot, node, "property");
            remove_text_children(xot, node)?;

            // Find the key child (has field="key" attr) and wrap in <key> element
            let children: Vec<XotNode> = xot.children(node)
                .filter(|&c| xot.element(c).is_some())
                .collect();
            for child in children {
                if let Some(field) = get_attr(xot, child, "field") {
                    if field == "key" {
                        let key_name = get_name(xot, "key");
                        // Copy start/end from child to wrapper
                        let start_val = get_attr(xot, child, "start");
                        let end_val = get_attr(xot, child, "end");
                        let wrapper = xot.new_element(key_name);
                        if let Some(sv) = start_val {
                            set_attr(xot, wrapper, "start", &sv);
                        }
                        if let Some(ev) = end_val {
                            set_attr(xot, wrapper, "end", &ev);
                        }
                        xot.insert_before(child, wrapper)?;
                        xot.detach(child)?;
                        xot.append(wrapper, child)?;
                        remove_attr(xot, child, "field");
                        break;
                    }
                }
            }

            Ok(TransformAction::Continue)
        }

        // string → <string>, extract content from string_content child
        "string" => {
            rename(xot, node, "string");
            // Replace children with just the string_content text
            let content = extract_string_content(xot, node);
            let all_children: Vec<XotNode> = xot.children(node).collect();
            for c in all_children {
                xot.detach(c)?;
            }
            if let Some(text) = content {
                let text_node = xot.new_text(&text);
                xot.append(node, text_node)?;
            }
            Ok(TransformAction::Done)
        }

        // string_content: shouldn't normally be reached since string handles it,
        // but flatten just in case
        "string_content" => {
            Ok(TransformAction::Flatten)
        }

        // number: already correct
        "number" => {
            remove_text_children_except_content(xot, node)?;
            Ok(TransformAction::Done)
        }

        // true/false → <bool>
        "true" | "false" => {
            rename(xot, node, "bool");
            Ok(TransformAction::Done)
        }

        // null: already correct
        "null" => {
            Ok(TransformAction::Done)
        }

        _ => Ok(TransformAction::Continue),
    }
}

/// Extract the text content from a string node's string_content child
fn extract_string_content(xot: &Xot, string_node: XotNode) -> Option<String> {
    for child in xot.children(string_node) {
        if let Some(name) = get_element_name(xot, child) {
            if name == "string_content" {
                return get_text_content(xot, child);
            }
        }
    }
    None
}

/// Remove text children that are punctuation (keep actual content text)
fn remove_text_children_except_content(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    // For leaf nodes like <number> that already have correct text, this is a no-op.
    // Only remove text that looks like punctuation.
    let to_remove: Vec<XotNode> = xot.children(node)
        .filter(|&child| {
            if let Some(text) = xot.text_str(child) {
                let trimmed = text.trim();
                matches!(trimmed, "{" | "}" | "[" | "]" | "," | ":" | "\"")
            } else {
                false
            }
        })
        .collect();
    for child in to_remove {
        xot.detach(child)?;
    }
    Ok(())
}

// =============================================================================
// Data Transform — query-friendly projection
// =============================================================================

/// Project JSON into query-friendly data view.
///
/// Object keys become element names, arrays use repeated elements,
/// scalars become text content.
pub fn data_transform(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let kind = match get_kind(xot, node) {
        Some(k) => k,
        None => return Ok(TransformAction::Continue),
    };

    match kind.as_str() {
        // document: flatten
        "document" => {
            remove_text_children(xot, node)?;
            Ok(TransformAction::Flatten)
        }

        // object: flatten (promote property elements to parent)
        "object" => {
            remove_text_children(xot, node)?;
            Ok(TransformAction::Flatten)
        }

        // pair: extract key, rename element to the key name
        "pair" => {
            transform_data_pair(xot, node)
        }

        // array: handle based on context
        // If under a named pair, flatten so items repeat with parent key name.
        // If anonymous (top-level or nested array), wrap items in <item>.
        "array" => {
            transform_data_array(xot, node)
        }

        // string: extract string_content, flatten to promote text to parent
        "string" => {
            let content = extract_string_content(xot, node);
            let all_children: Vec<XotNode> = xot.children(node).collect();
            for c in all_children {
                xot.detach(c)?;
            }
            if let Some(text) = content {
                let text_node = xot.new_text(&text);
                xot.append(node, text_node)?;
            }
            Ok(TransformAction::Flatten)
        }

        "string_content" => {
            Ok(TransformAction::Flatten)
        }

        // number: flatten to promote text to parent
        "number" => {
            Ok(TransformAction::Flatten)
        }

        // true/false/null: flatten text to parent
        "true" | "false" | "null" => {
            Ok(TransformAction::Flatten)
        }

        _ => Ok(TransformAction::Continue),
    }
}

/// Transform a pair into a named data element.
///
/// Extract the key text, sanitize it for XML, rename the pair element to that key.
/// Remove the key child and value wrapper, keeping the value content.
fn transform_data_pair(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    // Extract key text from the key child
    if let Some(key_text) = extract_pair_key_text(xot, node) {
        let safe_name = sanitize_xml_name(&key_text);
        rename(xot, node, &safe_name);

        // Remove the key child and colon/punctuation text
        let children: Vec<XotNode> = xot.children(node).collect();
        for child in children {
            if xot.text_str(child).is_some() {
                // Remove ":" and other text nodes
                xot.detach(child)?;
            } else if let Some(field) = get_attr(xot, child, "field") {
                if field == "key" {
                    // Remove the key child entirely
                    xot.detach(child)?;
                }
            }
        }

        // If key was sanitized, add <key> child with original text
        if safe_name != key_text {
            prepend_element_with_text(xot, node, "key", &key_text)?;
        }

        // Flatten the <value> wrapper if present (promote its children)
        let children: Vec<XotNode> = xot.children(node)
            .filter(|&c| xot.element(c).is_some())
            .collect();
        for child in children {
            if let Some(name) = get_element_name(xot, child) {
                if name == "value" {
                    flatten_node(xot, child)?;
                    break;
                }
            }
        }
    }

    Ok(TransformAction::Continue)
}

/// Extract key text from a pair's key child (string with field="key")
fn extract_pair_key_text(xot: &Xot, pair_node: XotNode) -> Option<String> {
    for child in xot.children(pair_node) {
        if let Some(field) = get_attr(xot, child, "field") {
            if field == "key" {
                // The key is a <string> with a <string_content> child
                return extract_string_content(xot, child)
                    .or_else(|| get_text_content(xot, child));
            }
        }
    }
    None
}

/// Transform a JSON array for data view.
///
/// Wraps each element child in an `<item>` element so that scalars don't
/// get flattened into merged text. Then flattens the array wrapper.
///
/// Result: array items become `<item>` children of the parent element.
fn transform_data_array(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    remove_text_children(xot, node)?;

    // Wrap each element child in <item>
    let children: Vec<XotNode> = xot.children(node)
        .filter(|&c| xot.element(c).is_some())
        .collect();

    let item_name = get_name(xot, "item");
    for child in children {
        let item = xot.new_element(item_name);
        // Copy span from child to <item>
        if let Some(sv) = get_attr(xot, child, "start") {
            set_attr(xot, item, "start", &sv);
        }
        if let Some(ev) = get_attr(xot, child, "end") {
            set_attr(xot, item, "end", &ev);
        }
        xot.insert_before(child, item)?;
        xot.detach(child)?;
        xot.append(item, child)?;
    }

    // Flatten the array wrapper — <item> elements promote to parent.
    // Children of <item> (the original scalars/objects/arrays) will be
    // processed recursively by the walker since Flatten walks children first.
    Ok(TransformAction::Flatten)
}

/// Sanitize a string to be a valid XML element name.
/// Replaces invalid characters with underscores.
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
