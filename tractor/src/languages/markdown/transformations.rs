//! Per-kind transformations for Markdown.
//!
//! Most kinds are pure renames or detaches (handled as data in
//! `rules.rs`). The handlers here cover the kinds that need
//! introspection of children — heading-level / list-type detection
//! and language extraction from code fences.

use xot::{Xot, Node as XotNode};

use crate::transform::{TransformAction, helpers::*};

use super::input::MdKind;
use super::output::*;

/// `atx_heading` → `<heading>` with `<h1/>`..`<h6/>` empty marker
/// derived from the `atx_h*_marker` child.
pub fn atx_heading(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let level = detect_heading_level(xot, node);
    xot.with_renamed(node, HEADING);
    if let Some(lvl) = level {
        let level_name = format!("h{}", lvl);
        xot.with_prepended_marker_from(node, &level_name, node)?;
    }
    Ok(TransformAction::Continue)
}

/// `setext_heading` → `<heading>` with `<h1/>` or `<h2/>` derived
/// from the underline child (`===` or `---`).
pub fn setext_heading(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let level = detect_setext_level(xot, node);
    xot.with_renamed(node, HEADING);
    if let Some(lvl) = level {
        let level_name = format!("h{}", lvl);
        xot.with_prepended_marker_from(node, &level_name, node)?;
    }
    Ok(TransformAction::Continue)
}

/// `fenced_code_block` → `<code_block>` with a fresh `<language>`
/// element prepended (extracted from the `info_string` child, which
/// is detached separately).
pub fn fenced_code_block(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let lang = detect_code_language(xot, node);
    xot.with_renamed(node, CODE_BLOCK);
    if let Some(lang) = lang {
        xot.with_prepended_element_with_text(node, LANGUAGE, &lang)?;
    }
    Ok(TransformAction::Continue)
}

/// `list` → `<list>` with `<ordered/>` or `<unordered/>` empty
/// marker derived from the first list_item's marker child.
pub fn list(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let list_type = detect_list_type(xot, node);
    xot.with_renamed(node, LIST);
    if let Some(t) = list_type {
        xot.with_prepended_marker_from(node, t, node)?;
    }
    Ok(TransformAction::Continue)
}

/// `thematic_break` → `<hr/>` (text children — the `---` literal —
/// stripped). Stops recursion.
pub fn thematic_break(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_renamed(node, HR);
    remove_text_children(xot, node)?;
    Ok(TransformAction::Done)
}

// =============================================================================
// Helpers
// =============================================================================

fn detect_heading_level(xot: &Xot, node: XotNode) -> Option<u8> {
    for child in xot.children(node) {
        if let Some(name) = get_element_name(xot, child) {
            match name.parse::<MdKind>().ok() {
                Some(MdKind::AtxH1Marker) => return Some(1),
                Some(MdKind::AtxH2Marker) => return Some(2),
                Some(MdKind::AtxH3Marker) => return Some(3),
                Some(MdKind::AtxH4Marker) => return Some(4),
                Some(MdKind::AtxH5Marker) => return Some(5),
                Some(MdKind::AtxH6Marker) => return Some(6),
                _ => {}
            }
        }
    }
    None
}

fn detect_setext_level(xot: &Xot, node: XotNode) -> Option<u8> {
    for child in xot.children(node) {
        if let Some(name) = get_element_name(xot, child) {
            match name.parse::<MdKind>().ok() {
                Some(MdKind::SetextH1Underline) => return Some(1),
                Some(MdKind::SetextH2Underline) => return Some(2),
                _ => {}
            }
        }
    }
    None
}

fn detect_code_language(xot: &Xot, node: XotNode) -> Option<String> {
    for child in xot.children(node) {
        if let Some(name) = get_element_name(xot, child) {
            if name.parse::<MdKind>().ok() == Some(MdKind::InfoString) {
                if let Some(text) = get_text_content(xot, child) {
                    let lang = text.trim().to_string();
                    if !lang.is_empty() {
                        return Some(lang);
                    }
                }
                for grandchild in xot.children(child) {
                    if let Some(gname) = get_element_name(xot, grandchild) {
                        if gname == LANGUAGE {
                            if let Some(text) = get_text_content(xot, grandchild) {
                                let lang = text.trim().to_string();
                                if !lang.is_empty() {
                                    return Some(lang);
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

fn detect_list_type(xot: &Xot, node: XotNode) -> Option<&'static str> {
    for child in xot.children(node) {
        if let Some(name) = get_element_name(xot, child) {
            if name.parse::<MdKind>().ok() == Some(MdKind::ListItem) {
                for marker in xot.children(child) {
                    if let Some(mname) = get_element_name(xot, marker) {
                        match mname.parse::<MdKind>().ok() {
                            Some(MdKind::ListMarkerPlus | MdKind::ListMarkerMinus | MdKind::ListMarkerStar) => {
                                return Some(UNORDERED);
                            }
                            Some(MdKind::ListMarkerDot | MdKind::ListMarkerParenthesis) => {
                                return Some(ORDERED);
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    None
}
