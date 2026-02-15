//! YAML data transform — query-friendly projection
//!
//! Mapping keys become element names, scalars become text content:
//!
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

use xot::{Xot, Node as XotNode};
use crate::xot_transform::{TransformAction, helpers::*};
use super::{strip_quotes, strip_quotes_from_node, normalize_block_scalar,
            decode_yaml_double_quote_escapes, decode_yaml_single_quote_escapes};

// /specs/tractor-parse/dual-view/data-branch.md: Data Branch
/// Project YAML into query-friendly data view.
///
/// Mapping keys become element names, sequences repeat the parent key element,
/// scalars become text content.
pub fn data_transform(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let kind = match get_element_name(xot, node) {
        Some(k) => k,
        None => return Ok(TransformAction::Continue),
    };

    match kind.as_str() {
        // Mapping pairs: rename to the key text
        "block_mapping_pair" | "flow_pair" => {
            transform_mapping_pair(xot, node)
        }

        // /specs/tractor-parse/dual-view/data-branch/arrays.md: Array Representation
        "block_sequence_item" => {
            let wrapper = find_ancestor_key_name(xot, node)
                .unwrap_or_else(|| "item".to_string());
            rename(xot, node, &wrapper);
            // Copy first element child's span (excludes the "- " prefix)
            let first_child = get_element_children(xot, node).into_iter().next();
            if let Some(child) = first_child {
                copy_source_location(xot, child, node);
            }
            remove_text_children(xot, node)?;
            Ok(TransformAction::Continue)
        }

        // /specs/tractor-parse/dual-view/data-branch/arrays.md: Array Representation
        "flow_sequence" => {
            transform_flow_sequence(xot, node)
        }

        // /specs/tractor-parse/dual-view/data-branch/scalars.md: Scalar Values
        "double_quote_scalar" => {
            strip_quotes_from_node(xot, node)?;
            // Decode escape sequences in the stripped text
            if let Some(text) = get_text_content(xot, node) {
                let decoded = decode_yaml_double_quote_escapes(&text);
                if decoded != text {
                    let children: Vec<XotNode> = xot.children(node).collect();
                    for c in children { xot.detach(c)?; }
                    let text_node = xot.new_text(&decoded);
                    xot.append(node, text_node)?;
                }
            }
            Ok(TransformAction::Flatten)
        }
        "single_quote_scalar" => {
            strip_quotes_from_node(xot, node)?;
            // Decode '' → ' in single-quoted strings
            if let Some(text) = get_text_content(xot, node) {
                let decoded = decode_yaml_single_quote_escapes(&text);
                if decoded != text {
                    let children: Vec<XotNode> = xot.children(node).collect();
                    for c in children { xot.detach(c)?; }
                    let text_node = xot.new_text(&decoded);
                    xot.append(node, text_node)?;
                }
            }
            Ok(TransformAction::Flatten)
        }

        // Block scalars (| or >): strip indicator and normalize
        "block_scalar" => {
            normalize_block_scalar(xot, node)?;
            Ok(TransformAction::Flatten)
        }

        // Anchors/tags: flatten
        "anchor" | "tag" => {
            remove_text_children(xot, node)?;
            Ok(TransformAction::Flatten)
        }

        // Anchor/alias names: flatten to promote text
        "alias_name" | "anchor_name" => {
            Ok(TransformAction::Flatten)
        }

        // Aliases: flatten
        "alias" => {
            remove_text_children(xot, node)?;
            Ok(TransformAction::Flatten)
        }

        // Document: keep for multi-doc YAML support
        // (single-doc flattening is handled by the builder)
        "document" => {
            remove_text_children(xot, node)?;
            Ok(TransformAction::Continue)
        }

        // Wrapper nodes to flatten (remove wrapper, promote children)
        "stream" | "block_node" | "block_mapping"
        | "flow_mapping" | "value" | "flow_node" | "plain_scalar"
        | "block_sequence" => {
            remove_text_children(xot, node)?;
            Ok(TransformAction::Flatten)
        }

        // Scalar values: flatten to promote text content to parent
        "string_scalar" | "integer_scalar" | "float_scalar"
        | "boolean_scalar" | "null_scalar" => {
            Ok(TransformAction::Flatten)
        }

        // Comments: remove entirely
        "comment" => {
            remove_text_children(xot, node)?;
            Ok(TransformAction::Flatten)
        }

        _ => Ok(TransformAction::Continue),
    }
}

// /specs/tractor-parse/dual-view/data-branch/objects.md: Object Key-to-Element Mapping
/// Transform a mapping pair by extracting the key and renaming the element.
fn transform_mapping_pair(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    if let Some(key) = extract_key_text(xot, node) {
        let safe_name = rename_to_key(xot, node, &key);

        // /specs/tractor-parse/dual-view/data-branch/source-spans.md: Value-Oriented Source Spans
        // The value child is the first non-key, non-text element child.
        let value_child = get_element_children(xot, node).into_iter()
            .find(|&c| get_attr(xot, c, "field").as_deref() != Some("key"));
        if let Some(vc) = value_child {
            copy_source_location(xot, vc, node);
        }

        // Remove key-related children and colon text
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

        // If value is a sequence and key wasn't sanitized, Flatten this pair
        // so that repeated key-named elements become siblings in the parent.
        if safe_name == key && has_sequence_child(xot, node) {
            return Ok(TransformAction::Flatten);
        }
    }
    Ok(TransformAction::Continue)
}

/// Extract the key text from a mapping pair's key child
fn extract_key_text(xot: &Xot, mapping_pair: XotNode) -> Option<String> {
    for child in xot.children(mapping_pair) {
        if let Some(field) = get_attr(xot, child, "field") {
            if field == "key" {
                return collect_deep_text(xot, child);
            }
        }
    }
    None
}

/// Recursively collect text content from a node tree
fn collect_deep_text(xot: &Xot, node: XotNode) -> Option<String> {
    let mut text = String::new();
    for child in xot.children(node) {
        if let Some(t) = xot.text_str(child) {
            text.push_str(t);
        }
    }
    let trimmed = text.trim().to_string();
    if !trimmed.is_empty() {
        let stripped = strip_quotes(&trimmed);
        return Some(stripped);
    }

    for child in xot.children(node) {
        if xot.element(child).is_some() {
            if let Some(t) = collect_deep_text(xot, child) {
                return Some(t);
            }
        }
    }
    None
}

/// Transform a flow sequence by renaming flow_node children to ancestor key name or "item"
fn transform_flow_sequence(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    remove_text_children(xot, node)?;

    let wrapper = find_ancestor_key_name(xot, node)
        .unwrap_or_else(|| "item".to_string());

    let children: Vec<XotNode> = xot.children(node)
        .filter(|&c| xot.element(c).is_some())
        .collect();
    for child in children {
        if let Some(name) = get_element_name(xot, child) {
            if name == "flow_node" {
                rename(xot, child, &wrapper);
            }
        }
    }

    Ok(TransformAction::Flatten)
}
