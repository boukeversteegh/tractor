//! Per-kind transformations for YAML, both transforms.
//!
//! Each function is a `Rule::Custom` target. YAML's two transforms
//! (`syntax_transform` and `data_transform`) share this module — the
//! `syntax_*` and `data_*` prefixes mark which transform a handler
//! belongs to.

use xot::{Xot, Node as XotNode};

use crate::transform::{TransformAction, helpers::*};
use crate::transform::data_keys::*;

use super::{strip_quotes, strip_quotes_from_node, normalize_block_scalar,
            decode_yaml_double_quote_escapes, decode_yaml_single_quote_escapes};
use super::input::YamlKind;
use super::output::*;

// =============================================================================
// Shared
// =============================================================================

/// Continue without changes — used for grammar leaves the transform
/// has nothing to say about (directives, escape sequences, …).
pub fn passthrough(_xot: &mut Xot, _node: XotNode) -> Result<TransformAction, xot::Error> {
    Ok(TransformAction::Continue)
}

/// Strip punctuation text children (`{`, `}`, `[`, `]`, `,`, `:`),
/// then continue into element children.
pub fn strip_punct_continue(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    remove_text_children(xot, node)?;
    Ok(TransformAction::Continue)
}

/// Strip punctuation, then promote children to the parent.
pub fn strip_punct_flatten(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    remove_text_children(xot, node)?;
    Ok(TransformAction::Flatten)
}

// =============================================================================
// Syntax branch
// =============================================================================

/// `block_mapping` / `flow_mapping` → `<object>`.
pub fn syntax_mapping(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_renamed(node, OBJECT);
    remove_text_children(xot, node)?;
    Ok(TransformAction::Continue)
}

/// `block_mapping_pair` / `flow_pair` → `<property>`, with the key
/// child wrapped in `<key>`.
pub fn syntax_pair(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_renamed(node, PROPERTY);
    remove_text_children(xot, node)?;

    let children: Vec<XotNode> = xot.children(node)
        .filter(|&c| xot.element(c).is_some())
        .collect();
    for child in children {
        if let Some(field) = get_attr(xot, child, "field") {
            if field == "key" {
                let key_name = get_name(xot, KEY);
                let wrapper = xot.new_element(key_name);
                xot.with_source_location_from(wrapper, child)
                    .with_wrap_child(child, wrapper)?
                    .with_removed_attr(child, "field");
                break;
            }
        }
    }

    Ok(TransformAction::Continue)
}

/// `block_sequence` / `flow_sequence` → `<array>`.
pub fn syntax_sequence(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_renamed(node, ARRAY);
    remove_text_children(xot, node)?;
    Ok(TransformAction::Continue)
}

/// Quoted scalar → `<string>`, quotes stripped.
pub fn syntax_quoted_string(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_renamed(node, STRING);
    strip_quotes_from_node(xot, node)?;
    Ok(TransformAction::Done)
}

/// Block scalar (`|` / `>`) → `<string>`, indicator stripped.
pub fn syntax_block_scalar(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_renamed(node, STRING);
    normalize_block_scalar(xot, node)?;
    Ok(TransformAction::Done)
}

/// `string_scalar` → `<string>`. (Plain scalar typed as string.)
pub fn syntax_string_scalar(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_renamed(node, STRING);
    Ok(TransformAction::Done)
}

/// `integer_scalar` / `float_scalar` → `<number>`.
pub fn syntax_number(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_renamed(node, NUMBER);
    Ok(TransformAction::Done)
}

/// `boolean_scalar` → `<bool>`.
pub fn syntax_bool(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_renamed(node, BOOL);
    Ok(TransformAction::Done)
}

/// `null_scalar` → `<null>`.
pub fn syntax_null(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_renamed(node, NULL);
    Ok(TransformAction::Done)
}

// =============================================================================
// Data branch
// =============================================================================

/// `block_mapping_pair` / `flow_pair` → element renamed to the key
/// text. Values become text content; sequences repeat the parent.
pub fn data_pair(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    if let Some(key) = extract_pair_key_text(xot, node) {
        let safe_name = rename_to_key(xot, node, &key);

        xot.with_attr(node, "field", &safe_name);

        // Copy the value child's source span onto the renamed pair.
        let value_child = get_element_children(xot, node).into_iter()
            .find(|&c| get_attr(xot, c, "field").as_deref() != Some("key"));
        if let Some(vc) = value_child {
            xot.with_source_location_from(node, vc);
        }

        // Drop key child + colon text.
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

        // Tag with a normalised scalar kind for the renderer.
        let value_kind = xot.children(node)
            .find(|&c| xot.element(c).is_some())
            .and_then(|c| get_kind(xot, c).and_then(|kind| kind.parse::<YamlKind>().ok()));
        if let Some(vk) = value_kind {
            match vk {
                YamlKind::StringScalar | YamlKind::DoubleQuoteScalar | YamlKind::SingleQuoteScalar
                | YamlKind::BlockScalar => {
                    xot.with_attr(node, "kind", "string");
                }
                YamlKind::IntegerScalar | YamlKind::FloatScalar => {
                    xot.with_attr(node, "kind", "number");
                }
                YamlKind::BooleanScalar => {
                    xot.with_attr(node, "kind", "boolean");
                }
                YamlKind::NullScalar => {
                    xot.with_attr(node, "kind", "null");
                }
                _ => {}
            }
        }

        // Sequence values + un-sanitised key: flatten so siblings repeat.
        if safe_name == key && has_sequence_child(xot, node) {
            return Ok(TransformAction::Flatten);
        }
    }
    Ok(TransformAction::Continue)
}

/// `block_sequence_item` → renamed to the ancestor key (or `<item>`),
/// span copied from the first element child (excludes the `- ` prefix).
pub fn data_sequence_item(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let wrapper = find_ancestor_key_name(xot, node).unwrap_or_else(|| ITEM.to_string());
    xot.with_renamed(node, &wrapper);
    let first_child = get_element_children(xot, node).into_iter().next();
    if let Some(child) = first_child {
        xot.with_source_location_from(node, child);
    }
    remove_text_children(xot, node)?;
    Ok(TransformAction::Continue)
}

/// `flow_sequence` → rename `flow_node` children to the ancestor key
/// (or `<item>`), then flatten the sequence wrapper.
pub fn data_flow_sequence(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    remove_text_children(xot, node)?;

    let wrapper = find_ancestor_key_name(xot, node).unwrap_or_else(|| ITEM.to_string());

    let children: Vec<XotNode> = xot.children(node)
        .filter(|&c| xot.element(c).is_some())
        .collect();
    for child in children {
        if let Some(name) = get_element_name(xot, child) {
            if name.parse::<YamlKind>().ok() == Some(YamlKind::FlowNode) {
                xot.with_renamed(child, &wrapper);
            }
        }
    }

    Ok(TransformAction::Flatten)
}

/// `double_quote_scalar` → quote-stripped + escape-decoded text,
/// promoted to parent.
pub fn data_double_quote(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    strip_quotes_from_node(xot, node)?;
    if let Some(text) = get_text_content(xot, node) {
        let decoded = decode_yaml_double_quote_escapes(&text);
        if decoded != text {
            xot.with_only_text(node, &decoded)?;
        }
    }
    Ok(TransformAction::Flatten)
}

/// `single_quote_scalar` → quote-stripped + `''` un-escaped.
pub fn data_single_quote(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    strip_quotes_from_node(xot, node)?;
    if let Some(text) = get_text_content(xot, node) {
        let decoded = decode_yaml_single_quote_escapes(&text);
        if decoded != text {
            xot.with_only_text(node, &decoded)?;
        }
    }
    Ok(TransformAction::Flatten)
}

/// Block scalar (`|` / `>`) → indicator stripped, content promoted.
pub fn data_block_scalar(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    normalize_block_scalar(xot, node)?;
    Ok(TransformAction::Flatten)
}

// =============================================================================
// Helpers
// =============================================================================

/// Extract the key text from a mapping pair's key child.
fn extract_pair_key_text(xot: &Xot, mapping_pair: XotNode) -> Option<String> {
    for child in xot.children(mapping_pair) {
        if let Some(field) = get_attr(xot, child, "field") {
            if field == "key" {
                return collect_deep_text(xot, child);
            }
        }
    }
    None
}

/// Recursively collect text content; strip surrounding quotes if present.
fn collect_deep_text(xot: &Xot, node: XotNode) -> Option<String> {
    let mut text = String::new();
    for child in xot.children(node) {
        if let Some(t) = xot.text_str(child) {
            text.push_str(t);
        }
    }
    let trimmed = text.trim().to_string();
    if !trimmed.is_empty() {
        return Some(strip_quotes(&trimmed));
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
