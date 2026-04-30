//! Per-kind transformations for T-SQL.
//!
//! Each function is a `Rule::Custom` target — `rule(TsqlKind) -> Rule`
//! references these by name. Simple flattens / pure renames /
//! `extract op + rename` patterns live as data in `rule()` (see
//! `rules.rs`), not here.

use xot::{Xot, Node as XotNode};

use crate::transform::{TransformAction, helpers::*};

use super::input::TsqlKind;
use super::output::TsqlName::{self, Alias, Name, Schema, Temp, Var};

/// Kinds whose name happens to match our semantic vocabulary already
/// (currently just `comment`) or grammar leaves the transform never
/// rewrites.
pub fn passthrough(_xot: &mut Xot, _node: XotNode) -> Result<TransformAction, xot::Error> {
    Ok(TransformAction::Continue)
}

/// Builder-inserted field wrappers `<value>`, `<left>`, `<right>` —
/// tsql doesn't need them around expressions; flatten so the inner
/// expression bubbles up. Dispatched by element name from the
/// orchestrator (no `kind=` attribute).
pub fn skip(_xot: &mut Xot, _node: XotNode) -> Result<TransformAction, xot::Error> {
    Ok(TransformAction::Skip)
}

/// `<name>` field wrapper inserted by the builder. T-SQL-specific:
///   - identifiers starting with `@` become `<var>` (variables).
///   - bracket delimiters (`[dbo]` → `dbo`) are stripped.
///   - otherwise, inline the identifier text into the wrapper.
pub fn name_wrapper(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let children: Vec<_> = xot.children(node).collect();
    for child in children {
        if let Some(child_name) = get_element_name(xot, child) {
            if child_name.parse::<TsqlKind>().ok() == Some(TsqlKind::Identifier) {
                if let Some(text) = get_text_content(xot, child) {
                    let replacement = if text.starts_with('@') {
                        xot.with_renamed(node, Var);
                        text[1..].to_string()
                    } else {
                        strip_brackets(&text)
                    };
                    xot.with_only_text(node, &replacement)?;
                    return Ok(TransformAction::Done);
                }
            }
        }
    }
    Ok(TransformAction::Continue)
}

/// `identifier` — classify based on field role and text content:
///   - `field="alias"`  → rename `<alias>`, strip brackets
///   - `field="schema"` → rename `<schema>`, strip brackets
///   - `@var`           → rename `<var>`, drop the `@` sigil
///   - otherwise        → rename `<name>`, strip brackets
pub fn identifier(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let text = match get_text_content(xot, node) {
        Some(t) => t,
        None => {
            xot.with_renamed(node, Name);
            return Ok(TransformAction::Done);
        }
    };

    if let Some(field_val) = get_attr(xot, node, "field") {
        match field_val.parse::<TsqlName>().ok() {
            Some(Alias) => {
                let clean = strip_brackets(&text);
                replace_text(xot, node, &clean);
                xot.with_renamed(node, Alias);
                return Ok(TransformAction::Done);
            }
            Some(Schema) => {
                let clean = strip_brackets(&text);
                replace_text(xot, node, &clean);
                xot.with_renamed(node, Schema);
                return Ok(TransformAction::Done);
            }
            _ => {}
        }
    }

    if text.starts_with('@') {
        let var_name = &text[1..];
        replace_text(xot, node, var_name);
        xot.with_renamed(node, Var);
    } else {
        let clean = strip_brackets(&text);
        replace_text(xot, node, &clean);
        xot.with_renamed(node, Name);
    }
    Ok(TransformAction::Done)
}

/// `unary_expression` — `#temp_table` references. When the unary
/// operator is `#`, replace the whole expression with a `<temp>`
/// element holding `#name` text.
pub fn unary_expression(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    let children: Vec<_> = xot.children(node).collect();
    let mut is_temp = false;
    for &child in &children {
        if let Some(child_name) = get_element_name(xot, child) {
            if child_name.parse::<TsqlKind>().ok() == Some(TsqlKind::OpUnaryOther) {
                if let Some(text) = get_text_content(xot, child) {
                    if text.trim() == "#" {
                        is_temp = true;
                    }
                }
            }
        }
    }

    if is_temp {
        for &child in &children {
            if let Some(field_val) = get_attr(xot, child, "field") {
                if field_val == "operand" {
                    if let Some(inner_text) = get_deep_identifier_text(xot, child) {
                        let all_children: Vec<_> = xot.children(node).collect();
                        for c in all_children {
                            xot.detach(c)?;
                        }
                        let text_node = xot.new_text(&format!("#{}", inner_text));
                        xot.append(node, text_node)?;
                        xot.with_renamed(node, Temp);
                        return Ok(TransformAction::Done);
                    }
                }
            }
        }
    }

    Ok(TransformAction::Continue)
}

// ---------------------------------------------------------------------
// Local helpers used by handlers above.
// ---------------------------------------------------------------------

fn strip_brackets(text: &str) -> String {
    if text.starts_with('[') && text.ends_with(']') && text.len() >= 2 {
        text[1..text.len() - 1].to_string()
    } else {
        text.to_string()
    }
}

fn replace_text(xot: &mut Xot, node: XotNode, new_text: &str) {
    let text_children: Vec<_> = xot.children(node)
        .filter(|&c| xot.text_str(c).is_some())
        .collect();
    for c in text_children {
        let _ = xot.detach(c);
    }
    let text_node = xot.new_text(new_text);
    let _ = xot.append(node, text_node);
}

fn get_deep_identifier_text(xot: &Xot, node: XotNode) -> Option<String> {
    if let Some(name) = get_element_name(xot, node) {
        if name.parse::<TsqlKind>().ok() == Some(TsqlKind::Identifier) {
            return get_text_content(xot, node);
        }
    }
    for child in xot.children(node) {
        if xot.element(child).is_some() {
            if let Some(text) = get_deep_identifier_text(xot, child) {
                return Some(text);
            }
        }
    }
    None
}
