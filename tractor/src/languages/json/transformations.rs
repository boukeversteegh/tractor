//! Per-kind transformations for JSON, both transforms.
//!
//! Each function is a `Rule::Custom` target. JSON's two transforms
//! (`syntax_transform` and `data_transform`) share this module — the
//! `syntax_*` and `data_*` prefixes mark which transform a handler
//! belongs to.

use xot::{Xot, Node as XotNode};

use crate::transform::{TransformAction, helpers::*};
use crate::transform::data_keys::*;

use super::{extract_string_content, extract_decoded_string_content};
use super::input::JsonKind;
use super::output::*;

// =============================================================================
// Shared
// =============================================================================

/// Strip JSON punctuation children (`{`, `}`, `[`, `]`, `,`, `:`),
/// then continue into element children. Used for `object` / `array`
/// in the syntax branch.
pub fn strip_punct_continue(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    remove_text_children(xot, node)?;
    Ok(TransformAction::Continue)
}

/// Strip punctuation, then promote children to the parent. Used for
/// `document` / `object` flattening in both branches.
pub fn strip_punct_flatten(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    remove_text_children(xot, node)?;
    Ok(TransformAction::Flatten)
}

// =============================================================================
// Syntax branch
// =============================================================================

/// `pair` → `<property>`, with the key child wrapped in `<key>`. The
/// builder already wraps the value child in `<value>` via field
/// wrappings.
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

/// `string` keeps its name; replace its children with the extracted
/// `string_content` text. Stops recursion (children are gone).
pub fn syntax_string(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let content = extract_string_content(xot, node);
    if let Some(text) = content {
        xot.with_only_text(node, &text)?;
    } else {
        xot.with_detached_children(node)?;
    }
    Ok(TransformAction::Done)
}

/// `number` keeps its name; remove punctuation text children but keep
/// the numeric content. Stops recursion.
pub fn syntax_number(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
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
    Ok(TransformAction::Done)
}

/// `true` / `false` → `<bool>`. Stops recursion.
pub fn syntax_bool(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_renamed(node, BOOL);
    Ok(TransformAction::Done)
}

/// `null` keeps its name; stop recursion.
pub fn syntax_null(_xot: &mut Xot, _node: XotNode) -> Result<TransformAction, xot::Error> {
    Ok(TransformAction::Done)
}

// =============================================================================
// Data branch
// =============================================================================

/// `pair` → element renamed to the key text. Values become text
/// content (via inner flattening), arrays repeat the parent name.
pub fn data_pair(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    if let Some(key_text) = extract_pair_key_text(xot, node) {
        let safe_name = rename_to_key(xot, node, &key_text);

        // The `field=` attribute is consumed by the mutation/upsert
        // logic (see `tractor/src/mutation/xpath_upsert.rs`) to
        // identify data-pair elements. NOT consumed by JSON output
        // post-iter-139, but the attribute is load-bearing for
        // data-tree write paths.
        xot.with_attr(node, "field", &safe_name);

        // Drop key child + colon/punctuation text
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

        // Flatten the <value> wrapper, copying its span first.
        let children: Vec<XotNode> = xot.children(node)
            .filter(|&c| xot.element(c).is_some())
            .collect();
        for child in children {
            if let Some(name) = get_element_name(xot, child) {
                if name == "value" {
                    xot.with_source_location_from(node, child);
                    flatten_node(xot, child)?;
                    break;
                }
            }
        }

        // Tag with scalar kind for the renderer.
        let value_kind = xot.children(node)
            .find(|&c| xot.element(c).is_some())
            .and_then(|c| get_kind(xot, c).and_then(|kind| kind.parse::<JsonKind>().ok()));
        if let Some(vk) = value_kind {
            match vk {
                JsonKind::String => { xot.with_attr(node, "kind", "string"); }
                JsonKind::Number => { xot.with_attr(node, "kind", "number"); }
                JsonKind::True => { xot.with_attr(node, "kind", "true"); }
                JsonKind::False => { xot.with_attr(node, "kind", "false"); }
                JsonKind::Null => { xot.with_attr(node, "kind", "null"); }
                _ => {}
            }
        }

        // Array values with un-sanitised key: flatten so siblings repeat.
        if safe_name == key_text && has_sequence_child(xot, node) {
            return Ok(TransformAction::Flatten);
        }
    }

    Ok(TransformAction::Continue)
}

/// `array` → wrap each item in `<key_name>` (or `<item>` if no
/// ancestor key), then flatten so the items become siblings of the
/// pair-named parent.
pub fn data_array(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    remove_text_children(xot, node)?;

    let wrapper = find_ancestor_key_name(xot, node).unwrap_or_else(|| ITEM.to_string());
    let wrapper_name = get_name(xot, &wrapper);

    let children: Vec<XotNode> = xot.children(node)
        .filter(|&c| xot.element(c).is_some())
        .collect();

    for child in children {
        let item = xot.new_element(wrapper_name);
        xot.with_source_location_from(item, child)
            .with_wrap_child(child, item)?;
    }

    Ok(TransformAction::Flatten)
}

/// `string` → decoded text content, then promote text to parent.
pub fn data_string(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let content = extract_decoded_string_content(xot, node);
    if let Some(text) = content {
        xot.with_only_text(node, &text)?;
    } else {
        xot.with_detached_children(node)?;
    }
    Ok(TransformAction::Flatten)
}

// =============================================================================
// Helpers
// =============================================================================

/// Extract key text from a pair's key child (string with field="key").
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
