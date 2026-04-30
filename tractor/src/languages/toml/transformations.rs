//! Per-kind transformations for TOML.
//!
//! Each function is a `Rule::Custom` target. Pure flattens stay as
//! data in `rules.rs` (`Rule::Flatten { … }`).

use xot::{Xot, Node as XotNode};

use crate::transform::{TransformAction, helpers::*};
use crate::transform::data_keys::*;

use super::output::ITEM;
use super::{strip_quotes, strip_quotes_from_node};

/// Continue without changes.
pub fn passthrough(_xot: &mut Xot, _node: XotNode) -> Result<TransformAction, xot::Error> {
    Ok(TransformAction::Continue)
}

/// `document` — strip text children, continue into the table /
/// pair / comment children.
pub fn document(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    remove_text_children(xot, node)?;
    Ok(TransformAction::Continue)
}

/// `inline_table` — strip punctuation, then promote children.
pub fn inline_table(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    remove_text_children(xot, node)?;
    Ok(TransformAction::Flatten)
}

/// `comment` — drop the `#` and comment text, then flatten away.
pub fn comment(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    remove_text_children(xot, node)?;
    Ok(TransformAction::Flatten)
}

/// `string` — strip surrounding quotes, then promote text to parent.
pub fn string(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    strip_quotes_from_node(xot, node)?;
    Ok(TransformAction::Flatten)
}

/// `pair` — element renamed to the key. Dotted keys (`a.b.c = …`)
/// produce nested `<a><b><c>…</c></b></a>` wrappers.
pub fn pair(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    if let Some(key_info) = extract_pair_key(xot, node) {
        let segments = key_info.segments;

        let children: Vec<XotNode> = xot.children(node).collect();
        for child in children {
            if xot.text_str(child).is_some() {
                xot.detach(child)?;
            } else if let Some(name) = get_element_name(xot, child) {
                if name == "bare_key" || name == "quoted_key" || name == "dotted_key" {
                    xot.detach(child)?;
                }
            }
        }

        if segments.len() == 1 {
            rename_to_key(xot, node, &segments[0]);
        } else {
            rename_to_key(xot, node, segments.last().unwrap());
            wrap_in_nested_elements(xot, node, &segments[..segments.len() - 1])?;
        }
    }
    Ok(TransformAction::Continue)
}

/// `table` — `[a.b.c]` header → element renamed to `c`, wrapped in
/// `<a><b>…</b></a>`. The bracket-and-key preamble is dropped.
pub fn table(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    if let Some(key_info) = extract_table_key(xot, node) {
        let segments = key_info.segments;

        let children: Vec<XotNode> = xot.children(node).collect();
        for child in children {
            if xot.text_str(child).is_some() {
                xot.detach(child)?;
            } else if let Some(name) = get_element_name(xot, child) {
                if name == "bare_key" || name == "quoted_key" || name == "dotted_key" {
                    xot.detach(child)?;
                }
            }
        }

        rename_to_key(xot, node, segments.last().unwrap());

        if segments.len() > 1 {
            wrap_in_nested_elements(xot, node, &segments[..segments.len() - 1])?;
        }
    }
    Ok(TransformAction::Continue)
}

/// `table_array_element` — `[[a.b]]` entry → `<a><b><item>…</item></b></a>`.
pub fn table_array_element(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    if let Some(key_info) = extract_table_key(xot, node) {
        let segments = key_info.segments;

        let children: Vec<XotNode> = xot.children(node).collect();
        for child in children {
            if xot.text_str(child).is_some() {
                xot.detach(child)?;
            } else if let Some(name) = get_element_name(xot, child) {
                if name == "bare_key" || name == "quoted_key" || name == "dotted_key" {
                    xot.detach(child)?;
                }
            }
        }

        rename(xot, node, ITEM);
        wrap_in_nested_elements(xot, node, &segments)?;
    }
    Ok(TransformAction::Continue)
}

/// `array` — wrap each element child in `<item>`, then flatten the
/// array wrapper.
pub fn array(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    remove_text_children(xot, node)?;

    let children: Vec<XotNode> = xot.children(node)
        .filter(|&c| xot.element(c).is_some())
        .collect();

    for child in children {
        let item_name = xot.add_name(ITEM);
        let item = xot.new_element(item_name);
        xot.insert_before(child, item)?;
        xot.detach(child)?;
        xot.append(item, child)?;
    }

    Ok(TransformAction::Flatten)
}

// =============================================================================
// Helpers
// =============================================================================

/// Key information extracted from a TOML node.
struct KeyInfo {
    segments: Vec<String>,
}

/// Find the first bare_key / quoted_key / dotted_key child and
/// return its segments. Used by `pair`, `table`, and
/// `table_array_element`.
fn extract_pair_key(xot: &Xot, node: XotNode) -> Option<KeyInfo> {
    for child in xot.children(node) {
        if let Some(name) = get_element_name(xot, child) {
            match name.as_str() {
                "bare_key" => {
                    let text = get_text_content(xot, child)?;
                    return Some(KeyInfo { segments: vec![text.trim().to_string()] });
                }
                "quoted_key" => {
                    let text = get_text_content(xot, child)?;
                    let stripped = strip_quotes(text.trim());
                    return Some(KeyInfo { segments: vec![stripped] });
                }
                "dotted_key" => {
                    return extract_dotted_key_segments(xot, child);
                }
                _ => {}
            }
        }
    }
    None
}

fn extract_table_key(xot: &Xot, node: XotNode) -> Option<KeyInfo> {
    extract_pair_key(xot, node)
}

fn extract_dotted_key_segments(xot: &Xot, node: XotNode) -> Option<KeyInfo> {
    let mut segments = Vec::new();
    collect_key_segments(xot, node, &mut segments);
    if segments.is_empty() {
        None
    } else {
        Some(KeyInfo { segments })
    }
}

fn collect_key_segments(xot: &Xot, node: XotNode, segments: &mut Vec<String>) {
    for child in xot.children(node) {
        if let Some(name) = get_element_name(xot, child) {
            match name.as_str() {
                "bare_key" => {
                    if let Some(text) = get_text_content(xot, child) {
                        segments.push(text.trim().to_string());
                    }
                }
                "quoted_key" => {
                    if let Some(text) = get_text_content(xot, child) {
                        segments.push(strip_quotes(text.trim()));
                    }
                }
                "dotted_key" => {
                    collect_key_segments(xot, child, segments);
                }
                _ => {}
            }
        }
    }
}

/// Wrap a node in nested elements for dotted-key segments.
/// Segments `["a", "b"]` and node N produce: `<a><b>N</b></a>`.
fn wrap_in_nested_elements(xot: &mut Xot, node: XotNode, segments: &[String]) -> Result<(), xot::Error> {
    let mut current = node;

    for segment in segments.iter().rev() {
        let safe_name = sanitize_xml_name(segment);
        let wrapper_name = xot.add_name(&safe_name);
        let wrapper = xot.new_element(wrapper_name);
        xot.insert_before(current, wrapper)?;
        xot.detach(current)?;
        xot.append(wrapper, current)?;
        current = wrapper;
    }

    Ok(())
}
