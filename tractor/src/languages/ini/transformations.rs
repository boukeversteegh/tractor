//! Per-kind transformations for INI.
//!
//! Each function is a `Rule::Custom` target. Simple flattens stay as
//! data in `rules.rs` (`Rule::Flatten { … }`), not here.

use xot::{Xot, Node as XotNode};

use crate::transform::{TransformAction, helpers::*};
use crate::transform::data_keys::*;

use super::output::COMMENT;

/// `document` → strip punctuation text children, then continue into
/// section / setting / comment children.
pub fn document(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    remove_text_children(xot, node)?;
    Ok(TransformAction::Continue)
}

/// `section` → element renamed to the section's name (extracted from
/// its `section_name` child). The bracket-and-name preamble is dropped.
pub fn section(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    if let Some(name) = extract_section_name(xot, node) {
        rename_to_key(xot, node, &name);

        let children: Vec<XotNode> = xot.children(node).collect();
        for child in children {
            if xot.text_str(child).is_some() {
                xot.detach(child)?;
            } else if let Some(child_name) = get_element_name(xot, child) {
                if child_name == "section_name" {
                    xot.detach(child)?;
                }
            }
        }
    }
    Ok(TransformAction::Continue)
}

/// `section_name` → strip text children + flatten. The parent
/// `section` handler usually detaches this child first; if the parser
/// produces a section_name without a section parent, this preserves
/// the original passthrough semantics.
pub fn section_name(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    remove_text_children(xot, node)?;
    Ok(TransformAction::Flatten)
}

/// `setting` → element renamed to the key. The `setting_value`
/// child's source span is copied to the renamed pair so `--set`
/// replaces only the value portion.
pub fn setting(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    if let Some(key) = extract_setting_name(xot, node) {
        rename_to_key(xot, node, &key);

        let value_child = xot.children(node).find(|&c| {
            get_element_name(xot, c).as_deref() == Some("setting_value")
        });
        if let Some(vc) = value_child {
            xot.with_source_location_from(node, vc);
        }

        let children: Vec<XotNode> = xot.children(node).collect();
        for child in children {
            if xot.text_str(child).is_some() {
                xot.detach(child)?;
            } else if let Some(child_name) = get_element_name(xot, child) {
                if child_name == "setting_name" {
                    xot.detach(child)?;
                }
            }
        }

        trim_value_text(xot, node)?;
    }
    Ok(TransformAction::Continue)
}

/// `comment` → `<comment>` with the text content (the `#` or `;`
/// prefix is stripped during parse — the actual text lives in a
/// `text` element child).
pub fn comment(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let mut comment_text = None;
    for child in xot.children(node) {
        if let Some(name) = get_element_name(xot, child) {
            if name == "text" {
                comment_text = get_text_content(xot, child).map(|s| s.trim().to_string());
            }
        }
    }

    let all_children: Vec<XotNode> = xot.children(node).collect();
    for c in all_children {
        xot.detach(c)?;
    }
    if let Some(text) = comment_text {
        let text_node = xot.new_text(&text);
        xot.append(node, text_node)?;
    }

    xot.with_renamed(node, COMMENT);
    Ok(TransformAction::Done)
}

// =============================================================================
// Helpers
// =============================================================================

/// The `section_name` node contains: `[`, `<text>name</text>`, `]`.
/// We extract the text from the `text` element child.
fn extract_section_name(xot: &Xot, node: XotNode) -> Option<String> {
    for child in xot.children(node) {
        if let Some(name) = get_element_name(xot, child) {
            if name == "section_name" {
                for grandchild in xot.children(child) {
                    if let Some(gname) = get_element_name(xot, grandchild) {
                        if gname == "text" {
                            if let Some(text) = get_text_content(xot, grandchild) {
                                let trimmed = text.trim().to_string();
                                if !trimmed.is_empty() {
                                    return Some(trimmed);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

fn extract_setting_name(xot: &Xot, node: XotNode) -> Option<String> {
    for child in xot.children(node) {
        if let Some(name) = get_element_name(xot, child) {
            if name == "setting_name" {
                return get_text_content(xot, child).map(|s| s.trim().to_string());
            }
        }
    }
    None
}

/// Trim whitespace from setting_value text within a setting node.
fn trim_value_text(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    let children: Vec<XotNode> = xot.children(node).collect();
    for child in children {
        if let Some(name) = get_element_name(xot, child) {
            if name == "setting_value" {
                if let Some(text) = get_text_content(xot, child) {
                    let trimmed = text.trim().to_string();
                    let all: Vec<XotNode> = xot.children(child).collect();
                    for c in all {
                        xot.detach(c)?;
                    }
                    let text_node = xot.new_text(&trimmed);
                    xot.append(child, text_node)?;
                }
            }
        }
    }
    Ok(())
}
