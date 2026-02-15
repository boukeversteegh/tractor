//! YAML syntax transform — normalized vocabulary
//!
//! Transforms TreeSitter YAML nodes into a unified syntax tree using the same
//! vocabulary as JSON: object/array/property/key/value/string/number/bool/null.

use xot::{Xot, Node as XotNode};
use crate::xot_transform::{TransformAction, helpers::*};
use super::{strip_quotes_from_node, normalize_block_scalar};

/// Normalize TreeSitter YAML into unified syntax vocabulary.
pub fn syntax_transform(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let kind = match get_kind(xot, node) {
        Some(k) => k,
        None => return Ok(TransformAction::Continue),
    };

    match kind.as_str() {
        // Top-level wrappers: flatten
        "stream" => {
            remove_text_children(xot, node)?;
            Ok(TransformAction::Flatten)
        }

        // Document: keep in syntax branch (consistent with JSON)
        "document" => {
            remove_text_children(xot, node)?;
            Ok(TransformAction::Continue)
        }

        // Structural wrappers to flatten
        "block_node" | "flow_node" => {
            remove_text_children(xot, node)?;
            Ok(TransformAction::Flatten)
        }

        // Mappings → <object>
        "block_mapping" | "flow_mapping" => {
            rename(xot, node, "object");
            remove_text_children(xot, node)?;
            Ok(TransformAction::Continue)
        }

        // Mapping pairs → <property>
        "block_mapping_pair" | "flow_pair" => {
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
                        let wrapper = xot.new_element(key_name);
                        if let Some(sv) = get_attr(xot, child, "start") {
                            set_attr(xot, wrapper, "start", &sv);
                        }
                        if let Some(ev) = get_attr(xot, child, "end") {
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

        // Sequences → <array>
        "block_sequence" | "flow_sequence" => {
            rename(xot, node, "array");
            remove_text_children(xot, node)?;
            Ok(TransformAction::Continue)
        }

        // Sequence items: flatten (children become direct array children)
        "block_sequence_item" => {
            remove_text_children(xot, node)?;
            Ok(TransformAction::Flatten)
        }

        // Plain scalars: flatten (wrapper around typed scalar children)
        "plain_scalar" => {
            remove_text_children(xot, node)?;
            Ok(TransformAction::Flatten)
        }

        // Quoted scalars → <string> (strip quotes)
        "double_quote_scalar" | "single_quote_scalar" => {
            rename(xot, node, "string");
            strip_quotes_from_node(xot, node)?;
            Ok(TransformAction::Done)
        }

        // Block scalars → <string>
        "block_scalar" => {
            rename(xot, node, "string");
            normalize_block_scalar(xot, node)?;
            Ok(TransformAction::Done)
        }

        // Typed scalars
        "integer_scalar" => {
            rename(xot, node, "number");
            Ok(TransformAction::Done)
        }
        "float_scalar" => {
            rename(xot, node, "number");
            Ok(TransformAction::Done)
        }
        "boolean_scalar" => {
            rename(xot, node, "bool");
            Ok(TransformAction::Done)
        }
        "null_scalar" => {
            rename(xot, node, "null");
            Ok(TransformAction::Done)
        }

        // String scalar (generic)
        "string_scalar" => {
            rename(xot, node, "string");
            Ok(TransformAction::Done)
        }

        // Anchors/tags/aliases: flatten
        "anchor" | "tag" => {
            remove_text_children(xot, node)?;
            Ok(TransformAction::Flatten)
        }
        "alias" => {
            remove_text_children(xot, node)?;
            Ok(TransformAction::Flatten)
        }
        "alias_name" | "anchor_name" => {
            Ok(TransformAction::Flatten)
        }

        // Comments: remove
        "comment" => {
            remove_text_children(xot, node)?;
            Ok(TransformAction::Flatten)
        }

        _ => Ok(TransformAction::Continue),
    }
}
