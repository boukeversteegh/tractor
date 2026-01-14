//! Rust transform logic

use xot::{Xot, Node as XotNode};
use crate::xot_transform::{TransformAction, helpers::*};

/// Transform a Rust AST node
pub fn transform(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let kind = match get_element_name(xot, node) {
        Some(k) => k,
        None => return Ok(TransformAction::Continue),
    };

    match kind.as_str() {
        "expression_statement" => Ok(TransformAction::Skip),
        "block" | "declaration_list" => Ok(TransformAction::Flatten),

        // Name wrappers - inline identifier text directly
        // <name><identifier>foo</identifier></name> -> <name>foo</name>
        "name" => {
            if let Some(parent) = get_parent(xot, node) {
                let parent_kind = get_element_name(xot, parent).unwrap_or_default();
                if matches!(parent_kind.as_str(),
                    "function_item" | "struct_item" | "enum_item" | "trait_item" | "mod_item" | "type_item"
                    | "function" | "struct" | "enum" | "trait" | "mod" | "typedef"
                ) {
                    let children: Vec<_> = xot.children(node).collect();
                    for child in children {
                        if let Some(child_name) = get_element_name(xot, child) {
                            if child_name == "identifier" || child_name == "type_identifier" {
                                if let Some(text) = get_text_content(xot, child) {
                                    let all_children: Vec<_> = xot.children(node).collect();
                                    for c in all_children {
                                        xot.detach(c)?;
                                    }
                                    let text_node = xot.new_text(&text);
                                    xot.append(node, text_node)?;
                                    return Ok(TransformAction::Done);
                                }
                            }
                        }
                    }
                }
            }
            Ok(TransformAction::Continue)
        }

        // Visibility modifier (pub, pub(crate), etc.)
        "visibility_modifier" => {
            if let Some(text) = get_text_content(xot, node) {
                let text = text.trim();
                // Extract just "pub" from "pub(crate)" etc.
                let modifier = if text.starts_with("pub") { "pub" } else { text };
                rename(xot, node, modifier);
                remove_text_children(xot, node)?;
                remove_attr(xot, node, "start");
                remove_attr(xot, node, "end");
                return Ok(TransformAction::Done);
            }
            Ok(TransformAction::Continue)
        }

        "binary_expression" | "unary_expression" => {
            extract_operator(xot, node)?;
            if let Some(new_name) = map_element_name(&kind) {
                rename(xot, node, new_name);
            }
            Ok(TransformAction::Continue)
        }

        // let declarations - extract mut modifier
        "let_declaration" => {
            extract_modifiers(xot, node)?;
            rename(xot, node, "let");
            Ok(TransformAction::Continue)
        }

        "identifier" => {
            let classification = classify_identifier(xot, node);
            rename(xot, node, classification);
            Ok(TransformAction::Continue)
        }
        "type_identifier" | "primitive_type" => {
            rename(xot, node, "type");
            Ok(TransformAction::Continue)
        }

        _ => {
            if let Some(new_name) = map_element_name(&kind) {
                rename(xot, node, new_name);
            }
            Ok(TransformAction::Continue)
        }
    }
}

fn map_element_name(kind: &str) -> Option<&'static str> {
    match kind {
        "source_file" => Some("file"),
        "function_item" => Some("function"),
        "impl_item" => Some("impl"),
        "struct_item" => Some("struct"),
        "enum_item" => Some("enum"),
        "trait_item" => Some("trait"),
        "mod_item" => Some("mod"),
        "use_declaration" => Some("use"),
        "const_item" => Some("const"),
        "static_item" => Some("static"),
        "type_item" => Some("typedef"),
        "parameters" => Some("params"),
        "parameter" => Some("param"),
        "self_parameter" => Some("self"),
        "reference_type" => Some("ref"),
        "generic_type" => Some("generic"),
        "scoped_type_identifier" => Some("path"),
        "return_expression" => Some("return"),
        "if_expression" => Some("if"),
        "else_clause" => Some("else"),
        "for_expression" => Some("for"),
        "while_expression" => Some("while"),
        "loop_expression" => Some("loop"),
        "match_expression" => Some("match"),
        "match_arm" => Some("arm"),
        "call_expression" => Some("call"),
        "method_call_expression" => Some("methodcall"),
        "field_expression" => Some("field"),
        "index_expression" => Some("index"),
        "binary_expression" => Some("binary"),
        "unary_expression" => Some("unary"),
        "closure_expression" => Some("closure"),
        "await_expression" => Some("await"),
        "try_expression" => Some("try"),
        "macro_invocation" => Some("macro"),
        "string_literal" => Some("string"),
        "raw_string_literal" => Some("rawstring"),
        "integer_literal" => Some("int"),
        "float_literal" => Some("float"),
        "boolean_literal" => Some("bool"),
        _ => None,
    }
}

fn extract_operator(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    let texts = get_text_children(xot, node);
    let operator = texts.iter().find(|t| {
        !t.chars().all(|c| matches!(c, '(' | ')' | ',' | ';' | '{' | '}' | '[' | ']'))
    });
    if let Some(op) = operator {
        set_attr(xot, node, "op", op);
    }
    Ok(())
}

fn extract_modifiers(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    let texts = get_text_children(xot, node);
    const MODIFIERS: &[&str] = &["mut", "async", "unsafe", "const"];

    let found: Vec<&str> = texts.iter()
        .filter_map(|t| MODIFIERS.iter().find(|&&m| m == t).copied())
        .collect();

    for modifier in found.into_iter().rev() {
        prepend_empty_element(xot, node, modifier)?;
    }
    Ok(())
}

fn classify_identifier(xot: &Xot, node: XotNode) -> &'static str {
    let parent = match get_parent(xot, node) {
        Some(p) => p,
        None => return "name",
    };
    let parent_kind = get_element_name(xot, parent).unwrap_or_default();

    // Check if followed by parameter list
    let siblings = get_following_siblings(xot, node);
    let has_param_sibling = siblings.iter().any(|&s| {
        get_element_name(xot, s)
            .map(|n| matches!(n.as_str(), "parameters"))
            .unwrap_or(false)
    });

    match parent_kind.as_str() {
        "function_item" if has_param_sibling => "name",
        "struct_item" | "enum_item" | "trait_item" | "mod_item" | "type_item" => "name",
        "let_declaration" => "name",
        "parameter" => "name",
        _ => "type",
    }
}
