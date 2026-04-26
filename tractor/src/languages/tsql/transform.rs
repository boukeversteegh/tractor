//! T-SQL (Microsoft SQL Server) transform logic

use xot::{Xot, Node as XotNode};
use crate::transform::{TransformAction, helpers::*};
use crate::transform::operators::{prepend_op_element, is_operator_marker};
use crate::output::syntax_highlight::SyntaxCategory;

use super::semantic::*;


/// Transform a T-SQL AST node.
///
/// Dispatch is split in two:
///   1. If the node carries a `kind` attribute (set by the builder from
///      the original tree-sitter kind), match on that — it never changes
///      mid-walk, so an arm like `"identifier"` always wins.
///   2. Otherwise the node is a builder-inserted field wrapper
///      (`<name>`, `<value>`, `<left>`, `<right>`, `<then>`, …). Match
///      on the element name for the few wrappers we need to handle.
pub fn transform(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let kind = match get_kind(xot, node) {
        Some(k) => k,
        None => {
            // Builder-inserted wrapper (no `kind` attribute).
            let name = get_element_name(xot, node).unwrap_or_default();
            return match name.as_str() {
                // Field wrappers tsql doesn't need around expressions —
                // flatten so the inner expression bubbles up.
                "value" | "left" | "right" => Ok(TransformAction::Skip),

                // <name> wrapper — inline identifier text (with bracket
                // stripping and @var detection).
                "name" => {
                    let children: Vec<_> = xot.children(node).collect();
                    for child in children {
                        if let Some(child_name) = get_element_name(xot, child) {
                            if child_name == "identifier" {
                                if let Some(text) = get_text_content(xot, child) {
                                    let all_children: Vec<_> = xot.children(node).collect();
                                    for c in all_children {
                                        xot.detach(c)?;
                                    }
                                    if text.starts_with('@') {
                                        // @variable → <var>variable</var>
                                        let text_node = xot.new_text(&text[1..]);
                                        xot.append(node, text_node)?;
                                        rename(xot, node, VAR);
                                    } else {
                                        let clean = strip_brackets(&text);
                                        let text_node = xot.new_text(&clean);
                                        xot.append(node, text_node)?;
                                    }
                                    return Ok(TransformAction::Done);
                                }
                            }
                        }
                    }
                    Ok(TransformAction::Continue)
                }
                _ => Ok(TransformAction::Continue),
            };
        }
    };

    match kind.as_str() {
        // Use Flatten for term instead of Skip to avoid freed-node issue
        "term" => Ok(TransformAction::Flatten),

        // Flatten select_expression children into select
        "select_expression" => Ok(TransformAction::Flatten),

        // Remove keyword_* nodes - they're just SQL keywords (SELECT, FROM, etc.)
        k if k.starts_with("keyword_") => {
            detach(xot, node)?;
            Ok(TransformAction::Done)
        }

        // Remove op_unary_other (e.g., # prefix on temp tables) - the # is kept in text
        "op_unary_other" => {
            detach(xot, node)?;
            Ok(TransformAction::Done)
        }

        // Unary expressions - handle #temp_table references
        "unary_expression" => {
            transform_unary(xot, node)?;
            Ok(TransformAction::Continue)
        }

        // Binary expressions - extract operator
        "binary_expression" => {
            extract_operator(xot, node)?;
            rename(xot, node, COMPARE);
            Ok(TransformAction::Continue)
        }

        // BETWEEN expression - rename and clean up
        "between_expression" => {
            rename(xot, node, BETWEEN);
            Ok(TransformAction::Continue)
        }

        // Assignment (UPDATE SET Name = value) - extract operator
        "assignment" => {
            extract_operator(xot, node)?;
            rename(xot, node, ASSIGN);
            Ok(TransformAction::Continue)
        }

        // Identifiers - classify based on context and content
        "identifier" => {
            transform_identifier(xot, node)?;
            Ok(TransformAction::Done)
        }

        // Rename to standard tractor conventions
        _ => {
            if let Some(new_name) = map_element_name(&kind) {
                rename(xot, node, new_name);
            }
            Ok(TransformAction::Continue)
        }
    }
}

/// Strip T-SQL bracket delimiters from identifier text: [dbo] → dbo
fn strip_brackets(text: &str) -> String {
    if text.starts_with('[') && text.ends_with(']') && text.len() >= 2 {
        text[1..text.len() - 1].to_string()
    } else {
        text.to_string()
    }
}

/// Transform an identifier node based on its content and context
fn transform_identifier(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    let text = match get_text_content(xot, node) {
        Some(t) => t,
        None => {
            rename(xot, node, NAME);
            return Ok(());
        }
    };

    // Check field attribute for special roles
    if let Some(field_val) = get_attr(xot, node, "field") {
        match field_val.as_str() {
            "alias" => {
                let clean = strip_brackets(&text);
                replace_text(xot, node, &clean);
                rename(xot, node, ALIAS);
                return Ok(());
            }
            "schema" => {
                let clean = strip_brackets(&text);
                replace_text(xot, node, &clean);
                rename(xot, node, SCHEMA);
                return Ok(());
            }
            _ => {}
        }
    }

    // Classify by content prefix
    if text.starts_with('@') {
        // @variable → <var>variable</var>
        let var_name = &text[1..];
        replace_text(xot, node, var_name);
        rename(xot, node, VAR);
    } else {
        // Regular identifier - strip brackets and rename to "name"
        let clean = strip_brackets(&text);
        replace_text(xot, node, &clean);
        rename(xot, node, NAME);
    }

    Ok(())
}

/// Replace all text content of a node
fn replace_text(xot: &mut Xot, node: XotNode, new_text: &str) {
    // Remove existing text children
    let text_children: Vec<_> = xot.children(node)
        .filter(|&c| xot.text_str(c).is_some())
        .collect();
    for c in text_children {
        let _ = xot.detach(c);
    }
    let text_node = xot.new_text(new_text);
    let _ = xot.append(node, text_node);
}

/// Transform unary expressions - specifically handles #temp_table
fn transform_unary(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    // Check if this is a # prefix (temp table)
    let children: Vec<_> = xot.children(node).collect();
    let mut is_temp = false;
    for &child in &children {
        if let Some(child_name) = get_element_name(xot, child) {
            if child_name == "op_unary_other" {
                if let Some(text) = get_text_content(xot, child) {
                    if text.trim() == "#" {
                        is_temp = true;
                    }
                }
            }
        }
    }

    if is_temp {
        // Extract the operand field name and prefix with #
        for &child in &children {
            if let Some(field_val) = get_attr(xot, child, "field") {
                if field_val == "operand" {
                    // Get the identifier text from inside the field/name/identifier chain
                    if let Some(inner_text) = get_deep_identifier_text(xot, child) {
                        // Replace the whole unary expression with a temp_ref
                        let all_children: Vec<_> = xot.children(node).collect();
                        for c in all_children {
                            xot.detach(c)?;
                        }
                        let text_node = xot.new_text(&format!("#{}", inner_text));
                        xot.append(node, text_node)?;
                        rename(xot, node, TEMP);
                        return Ok(());
                    }
                }
            }
        }
    }

    Ok(())
}

/// Recursively find identifier text deep in a node tree
fn get_deep_identifier_text(xot: &Xot, node: XotNode) -> Option<String> {
    // Check if this node itself has text
    if let Some(name) = get_element_name(xot, node) {
        if name == "identifier" {
            return get_text_content(xot, node);
        }
    }
    // Search children
    for child in xot.children(node) {
        if xot.element(child).is_some() {
            if let Some(text) = get_deep_identifier_text(xot, child) {
                return Some(text);
            }
        }
    }
    None
}

/// Map tree-sitter node kinds to semantic element names.
///
/// Derived from `semantic::KINDS` — the catalogue is the single source
/// of truth, this is just the rename projection.
fn map_element_name(kind: &str) -> Option<&'static str> {
    super::semantic::rename_target(kind)
}

fn extract_operator(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    let texts = get_text_children(xot, node);
    let operator = texts.iter().find(|t| {
        let trimmed = t.trim();
        !trimmed.is_empty()
            && !trimmed.chars().all(|c| matches!(c, '(' | ')' | ',' | ';' | '{' | '}' | '[' | ']' | '.'))
    });
    if let Some(op) = operator {
        prepend_op_element(xot, node, op.trim())?;
    }
    Ok(())
}

/// Map a transformed element name to a syntax category for highlighting.
///
/// Consults the per-name NODES table first (one source of truth);
/// falls back to cross-cutting rules for names not in NODES.
pub fn syntax_category(element: &str) -> SyntaxCategory {
    if let Some(spec) = super::semantic::spec(element) {
        return spec.syntax;
    }
    match element {
        // Raw tree-sitter kinds / builder wrappers not in NODES:
        "order_by" | "group_by" => SyntaxCategory::Keyword,
        "create_table" | "create_function" => SyntaxCategory::Keyword,
        _ if is_operator_marker(element) => SyntaxCategory::Operator,
        _ => SyntaxCategory::Default,
    }
}

#[cfg(test)]
mod tests {
    use crate::languages::tsql::semantic::NODES;

    #[test]
    fn no_duplicate_node_names() {
        let mut names: Vec<&str> = NODES.iter().map(|n| n.name).collect();
        names.sort();
        let total = names.len();
        names.dedup();
        assert_eq!(names.len(), total, "duplicate NODES entry");
    }

    #[test]
    fn no_unused_role() {
        for n in NODES {
            assert!(
                n.marker || n.container,
                "<{}> is neither marker nor container — dead entry?",
                n.name,
            );
        }
    }
}
