//! Per-kind transformations for env (.env file format, parsed via
//! `tree-sitter-bash`).
//!
//! Each function is a `Rule::Custom` target. Pure flattens stay as
//! data in `rules.rs` (`Rule::Flatten { … }`).

use xot::{Xot, Node as XotNode};

use crate::transform::{TransformAction, helpers::*};
use crate::transform::data_keys::*;

use super::output::{COMMENT, DOCUMENT};

/// `program` (bash root) → `<document>`. Strip text children.
pub fn program(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_renamed(node, DOCUMENT);
    remove_text_children(xot, node)?;
    Ok(TransformAction::Continue)
}

/// `declaration_command` — `export KEY=…`. Strip the `export`
/// keyword text and flatten so the inner `variable_assignment`
/// surfaces.
pub fn declaration_command(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    remove_text_children(xot, node)?;
    Ok(TransformAction::Flatten)
}

/// `concatenation` — adjacent value parts. Strip text, flatten so the
/// pieces bubble up to the variable_assignment value extractor.
pub fn concatenation(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    remove_text_children(xot, node)?;
    Ok(TransformAction::Flatten)
}

/// `variable_assignment` — extract key + value, rebuild as
/// `<KEY>value</KEY>`.
///
/// The bash AST shape:
/// ```xml
/// <variable_assignment>
///   <name><variable_name>KEY</variable_name></name>
///   =
///   <value><word>val</word></value>
/// </variable_assignment>
/// ```
pub fn variable_assignment(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let var_name = extract_variable_name(xot, node);
    let var_value = extract_variable_value(xot, node);

    if let Some(name) = var_name {
        rename_to_key(xot, node, &name);

        // Copy the value child's source span onto the renamed node so
        // `--set` replaces only the value portion.
        let value_child = xot.children(node).find(|&c| {
            get_element_name(xot, c).as_deref() == Some("value")
        });
        if let Some(vc) = value_child {
            xot.with_source_location_from(node, vc);
        }

        let children: Vec<XotNode> = xot.children(node).collect();
        for child in children {
            xot.detach(child)?;
        }

        if let Some(value) = var_value {
            let text_node = xot.new_text(&value);
            xot.append(node, text_node)?;
        }
    }
    Ok(TransformAction::Done)
}

/// `comment` → `<comment>` with the `#` prefix stripped.
pub fn comment(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    if let Some(text) = get_text_content(xot, node) {
        let stripped = text.strip_prefix('#')
            .unwrap_or(&text)
            .trim_start()
            .to_string();

        let all_children: Vec<XotNode> = xot.children(node).collect();
        for c in all_children {
            xot.detach(c)?;
        }
        let text_node = xot.new_text(&stripped);
        xot.append(node, text_node)?;
    }

    xot.with_renamed(node, COMMENT);
    Ok(TransformAction::Done)
}

// =============================================================================
// Helpers
// =============================================================================

/// Extract the variable name from a variable_assignment node.
/// Looks inside the `name` field wrapper for `variable_name`.
fn extract_variable_name(xot: &Xot, node: XotNode) -> Option<String> {
    for child in xot.children(node) {
        if let Some(child_name) = get_element_name(xot, child) {
            if child_name == "variable_name" {
                return get_text_content(xot, child).map(|s| s.trim().to_string());
            }
            if child_name == "name" {
                for grandchild in xot.children(child) {
                    if let Some(gname) = get_element_name(xot, grandchild) {
                        if gname == "variable_name" {
                            return get_text_content(xot, grandchild).map(|s| s.trim().to_string());
                        }
                    }
                }
                return get_text_content(xot, child).map(|s| s.trim().to_string());
            }
        }
    }
    None
}

/// Extract the value from a variable_assignment node.
fn extract_variable_value(xot: &Xot, node: XotNode) -> Option<String> {
    for child in xot.children(node) {
        if let Some(child_name) = get_element_name(xot, child) {
            if child_name == "value" {
                return extract_value_content(xot, child);
            }
        }
    }
    None
}

/// Recursively extract text content from a value node, stripping
/// quotes and joining concatenated parts.
fn extract_value_content(xot: &Xot, node: XotNode) -> Option<String> {
    let mut parts = Vec::new();
    collect_value_text(xot, node, &mut parts);
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(""))
    }
}

fn collect_value_text(xot: &Xot, node: XotNode, parts: &mut Vec<String>) {
    for child in xot.children(node) {
        if let Some(text) = xot.text_str(child) {
            let trimmed = text.trim();
            if trimmed == "\"" || trimmed == "'" || trimmed == "$" {
                continue;
            }
            if !text.is_empty() {
                parts.push(text.to_string());
            }
        } else if let Some(child_name) = get_element_name(xot, child) {
            match child_name.as_str() {
                "word" | "number" | "string_content" | "variable_name" => {
                    if let Some(text) = get_text_content(xot, child) {
                        parts.push(text);
                    }
                }
                "raw_string" => {
                    if let Some(text) = get_text_content(xot, child) {
                        let stripped = text.strip_prefix('\'')
                            .and_then(|s| s.strip_suffix('\''))
                            .unwrap_or(&text);
                        parts.push(stripped.to_string());
                    }
                }
                "string" | "concatenation" | "simple_expansion" | "expansion"
                | "command_substitution" => {
                    collect_value_text(xot, child, parts);
                }
                _ => {
                    if let Some(text) = get_text_content(xot, child) {
                        parts.push(text);
                    }
                }
            }
        }
    }
}
