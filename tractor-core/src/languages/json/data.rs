//! JSON data transform — query-friendly projection
//!
//! Object keys become element names, scalars become text content:
//!
//! ```json
//! {"name": "John"}
//! ```
//! Becomes:
//! ```xml
//! <name>John</name>
//! ```
//! Queryable as: `//data/name`

use xot::{Xot, Node as XotNode};
use crate::xot_transform::{TransformAction, helpers::*};
use super::{extract_string_content, sanitize_xml_name};

/// Project JSON into query-friendly data view.
///
/// Object keys become element names, arrays use <item> wrappers,
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

        // array: wrap items in <item>, then flatten
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
    if let Some(key_text) = extract_pair_key_text(xot, node) {
        let safe_name = sanitize_xml_name(&key_text);
        rename(xot, node, &safe_name);

        // Remove the key child and colon/punctuation text
        let children: Vec<XotNode> = xot.children(node).collect();
        for child in children {
            if xot.text_str(child).is_some() {
                xot.detach(child)?;
            } else if let Some(field) = get_attr(xot, child, "field") {
                if field == "key" {
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
fn transform_data_array(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    remove_text_children(xot, node)?;

    // Wrap each element child in <item>
    let children: Vec<XotNode> = xot.children(node)
        .filter(|&c| xot.element(c).is_some())
        .collect();

    let item_name = get_name(xot, "item");
    for child in children {
        let item = xot.new_element(item_name);
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

    Ok(TransformAction::Flatten)
}
