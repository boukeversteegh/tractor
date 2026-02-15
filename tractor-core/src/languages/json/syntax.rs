//! JSON syntax transform — normalized vocabulary
//!
//! Transforms TreeSitter JSON nodes into a unified syntax tree:
//!
//! ```xml
//! <object>
//!   <property>
//!     <key><string>name</string></key>
//!     <value><string>John</string></value>
//!   </property>
//! </object>
//! ```

use xot::{Xot, Node as XotNode};
use crate::xot_transform::{TransformAction, helpers::*};
use super::extract_string_content;

// /specs/tractor-parse/dual-view/syntax-branch/vocabulary.md: Unified Syntax Vocabulary
/// Normalize TreeSitter JSON into unified syntax vocabulary.
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
pub fn syntax_transform(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
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

        // string_content: flatten just in case
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

/// Remove text children that are punctuation (keep actual content text)
fn remove_text_children_except_content(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
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
