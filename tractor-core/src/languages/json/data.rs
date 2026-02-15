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
use super::{extract_string_content, extract_decoded_string_content};

// /specs/tractor-parse/dual-view/data-branch.md: Data Branch
/// Project JSON into query-friendly data view.
///
/// Object keys become element names, arrays repeat the parent key element,
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

        // /specs/tractor-parse/dual-view/data-branch/scalars.md: Scalar Values
        // string: extract decoded content (handles escape sequences), flatten to parent
        "string" => {
            let content = extract_decoded_string_content(xot, node);
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

// /specs/tractor-parse/dual-view/data-branch/objects.md: Object Key-to-Element Mapping
/// Transform a pair into a named data element.
fn transform_data_pair(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    if let Some(key_text) = extract_pair_key_text(xot, node) {
        let safe_name = rename_to_key(xot, node, &key_text);

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

        // /specs/tractor-parse/dual-view/data-branch/source-spans.md: Value-Oriented Source Spans
        // Flatten the <value> wrapper if present, copying its span first.
        let children: Vec<XotNode> = xot.children(node)
            .filter(|&c| xot.element(c).is_some())
            .collect();
        for child in children {
            if let Some(name) = get_element_name(xot, child) {
                if name == "value" {
                    copy_source_location(xot, child, node);
                    flatten_node(xot, child)?;
                    break;
                }
            }
        }

        // If value is an array and key wasn't sanitized, Flatten this pair
        // so that repeated key-named elements become siblings in the parent.
        if safe_name == key_text && has_sequence_child(xot, node) {
            return Ok(TransformAction::Flatten);
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

// /specs/tractor-parse/dual-view/data-branch/arrays.md: Array Representation
/// Transform a JSON array for data view.
fn transform_data_array(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    remove_text_children(xot, node)?;

    let wrapper = find_ancestor_key_name(xot, node).unwrap_or_else(|| "item".to_string());
    let wrapper_name = get_name(xot, &wrapper);

    let children: Vec<XotNode> = xot.children(node)
        .filter(|&c| xot.element(c).is_some())
        .collect();

    for child in children {
        let item = xot.new_element(wrapper_name);
        copy_source_location(xot, child, item);
        xot.insert_before(child, item)?;
        xot.detach(child)?;
        xot.append(item, child)?;
    }

    Ok(TransformAction::Flatten)
}
