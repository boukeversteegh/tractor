//! Python transform logic

use xot::{Xot, Node as XotNode};
use crate::xot_transform::{TransformAction, helpers::*};

/// Transform a Python AST node
pub fn transform(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let kind = match get_element_name(xot, node) {
        Some(k) => k,
        None => return Ok(TransformAction::Continue),
    };

    match kind.as_str() {
        "expression_statement" => Ok(TransformAction::Skip),
        "block" => Ok(TransformAction::Flatten),

        // Name wrappers - inline identifier text directly
        "name" => {
            if let Some(parent) = get_parent(xot, node) {
                let parent_kind = get_element_name(xot, parent).unwrap_or_default();
                if matches!(parent_kind.as_str(),
                    "function_definition" | "class_definition"
                    | "function" | "class"
                ) {
                    let children: Vec<_> = xot.children(node).collect();
                    for child in children {
                        if let Some(child_name) = get_element_name(xot, child) {
                            if child_name == "identifier" {
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

        // Binary/comparison operators
        "binary_operator" | "comparison_operator" | "boolean_operator"
        | "unary_operator" | "augmented_assignment" => {
            extract_operator(xot, node)?;
            if let Some(new_name) = map_element_name(&kind) {
                rename(xot, node, new_name);
            }
            Ok(TransformAction::Continue)
        }

        // Identifiers
        "identifier" => {
            let classification = classify_identifier(xot, node);
            rename(xot, node, classification);
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
        "module" => Some("module"),
        "class_definition" => Some("class"),
        "function_definition" => Some("function"),
        "decorated_definition" => Some("decorated"),
        "decorator" => Some("decorator"),
        "parameters" => Some("params"),
        "default_parameter" | "typed_parameter" | "typed_default_parameter" => Some("param"),
        "return_statement" => Some("return"),
        "if_statement" => Some("if"),
        "elif_clause" => Some("elif"),
        "else_clause" => Some("else"),
        "for_statement" => Some("for"),
        "while_statement" => Some("while"),
        "try_statement" => Some("try"),
        "except_clause" => Some("except"),
        "finally_clause" => Some("finally"),
        "with_statement" => Some("with"),
        "raise_statement" => Some("raise"),
        "pass_statement" => Some("pass"),
        "import_statement" => Some("import"),
        "import_from_statement" => Some("from"),
        "call" => Some("call"),
        "attribute" => Some("member"),
        "subscript" => Some("subscript"),
        "assignment" => Some("assign"),
        "augmented_assignment" => Some("augassign"),
        "binary_operator" => Some("binary"),
        "unary_operator" => Some("unary"),
        "comparison_operator" => Some("compare"),
        "boolean_operator" => Some("logical"),
        "conditional_expression" => Some("ternary"),
        "lambda" => Some("lambda"),
        "await" => Some("await"),
        "list_comprehension" => Some("listcomp"),
        "dictionary_comprehension" => Some("dictcomp"),
        "set_comprehension" => Some("setcomp"),
        "generator_expression" => Some("genexp"),
        "string" => Some("string"),
        "integer" => Some("int"),
        "float" => Some("float"),
        "true" => Some("true"),
        "false" => Some("false"),
        "none" => Some("none"),
        _ => None,
    }
}

fn extract_operator(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    let texts = get_text_children(xot, node);
    let operator = texts.iter().find(|t| {
        !t.chars().all(|c| matches!(c, '(' | ')' | ',' | ':' | '{' | '}' | '[' | ']'))
    });
    if let Some(op) = operator {
        prepend_element_with_text(xot, node, "op", op)?;
    }
    Ok(())
}

fn classify_identifier(xot: &Xot, node: XotNode) -> &'static str {
    let parent = match get_parent(xot, node) {
        Some(p) => p,
        None => return "name",
    };
    let parent_kind = get_element_name(xot, parent).unwrap_or_default();

    match parent_kind.as_str() {
        "function_definition" | "class_definition" => "name",
        "parameter" | "default_parameter" | "typed_parameter" => "name",
        "assignment" => "name",
        _ => "type",
    }
}
