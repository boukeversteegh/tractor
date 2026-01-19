//! Go transform logic

use xot::{Xot, Node as XotNode};
use crate::xot_transform::{TransformAction, helpers::*};
use crate::output::syntax_highlight::SyntaxCategory;

/// Transform a Go AST node
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
                    "function_declaration" | "method_declaration" | "type_spec"
                    | "function" | "method" | "typespec"
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

        "binary_expression" | "unary_expression" => {
            extract_operator(xot, node)?;
            if let Some(new_name) = map_element_name(&kind) {
                rename(xot, node, new_name);
            }
            Ok(TransformAction::Continue)
        }

        "identifier" => {
            let classification = classify_identifier(xot, node);
            rename(xot, node, classification);
            Ok(TransformAction::Continue)
        }
        "type_identifier" => {
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
        "package_clause" => Some("package"),
        "function_declaration" => Some("function"),
        "method_declaration" => Some("method"),
        "type_declaration" => Some("typedef"),
        "type_spec" => Some("typespec"),
        "struct_type" => Some("struct"),
        "interface_type" => Some("interface"),
        "const_declaration" => Some("const"),
        "var_declaration" => Some("var"),
        "parameter_list" => Some("params"),
        "parameter_declaration" => Some("param"),
        "pointer_type" => Some("pointer"),
        "slice_type" => Some("slice"),
        "map_type" => Some("map"),
        "channel_type" => Some("chan"),
        "return_statement" => Some("return"),
        "if_statement" => Some("if"),
        "else_clause" => Some("else"),
        "for_statement" => Some("for"),
        "range_clause" => Some("range"),
        "switch_statement" => Some("switch"),
        "case_clause" => Some("case"),
        "default_case" => Some("default"),
        "defer_statement" => Some("defer"),
        "go_statement" => Some("go"),
        "select_statement" => Some("select"),
        "call_expression" => Some("call"),
        "selector_expression" => Some("member"),
        "index_expression" => Some("index"),
        "composite_literal" => Some("literal"),
        "binary_expression" => Some("binary"),
        "unary_expression" => Some("unary"),
        "interpreted_string_literal" => Some("string"),
        "raw_string_literal" => Some("rawstring"),
        "int_literal" => Some("int"),
        "float_literal" => Some("float"),
        "true" => Some("true"),
        "false" => Some("false"),
        "nil" => Some("nil"),
        "field_identifier" => Some("field"),
        "package_identifier" => Some("pkg"),
        _ => None,
    }
}

fn extract_operator(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    let texts = get_text_children(xot, node);
    let operator = texts.iter().find(|t| {
        !t.chars().all(|c| matches!(c, '(' | ')' | ',' | ';' | '{' | '}' | '[' | ']'))
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
        "function_declaration" | "method_declaration" => "name",
        "type_spec" => "name",
        "parameter_declaration" => "name",
        "var_spec" | "const_spec" => "name",
        _ => "type",
    }
}

/// Map a transformed element name to a syntax category for highlighting
pub fn syntax_category(element: &str) -> SyntaxCategory {
    match element {
        // Identifiers
        "name" => SyntaxCategory::Identifier,
        "type" => SyntaxCategory::Type,
        "field" => SyntaxCategory::Identifier,
        "pkg" => SyntaxCategory::Identifier,

        // Literals
        "string" | "rawstring" => SyntaxCategory::String,
        "int" | "float" => SyntaxCategory::Number,
        "true" | "false" | "nil" => SyntaxCategory::Keyword,

        // Keywords - declarations
        "function" | "method" => SyntaxCategory::Keyword,
        "struct" | "interface" => SyntaxCategory::Keyword,
        "typedef" | "typespec" => SyntaxCategory::Keyword,
        "const" | "var" => SyntaxCategory::Keyword,
        "package" => SyntaxCategory::Keyword,
        "param" | "params" => SyntaxCategory::Keyword,

        // Keywords - control flow
        "if" | "else" => SyntaxCategory::Keyword,
        "for" | "range" => SyntaxCategory::Keyword,
        "switch" | "case" | "default" => SyntaxCategory::Keyword,
        "select" => SyntaxCategory::Keyword,
        "return" | "break" | "continue" | "goto" => SyntaxCategory::Keyword,
        "defer" | "go" => SyntaxCategory::Keyword,

        // Types
        "pointer" | "slice" | "map" | "chan" => SyntaxCategory::Type,

        // Functions/calls
        "call" => SyntaxCategory::Function,

        // Operators
        "op" => SyntaxCategory::Operator,
        "binary" | "unary" => SyntaxCategory::Operator,

        // Comments
        "comment" => SyntaxCategory::Comment,

        // Structural elements - no color
        _ => SyntaxCategory::Default,
    }
}
