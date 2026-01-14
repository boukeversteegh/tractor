//! TypeScript/JavaScript transform logic
//!
//! This module owns ALL TypeScript-specific transformation rules.
//! No assumptions about other languages - this is self-contained.

use xot::{Xot, Node as XotNode};
use crate::xot_transform::{TransformAction, helpers::*};

/// Transform a TypeScript AST node
///
/// This is the main entry point - receives each node during tree walk
/// and decides what transformations to apply.
pub fn transform(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let kind = match get_element_name(xot, node) {
        Some(k) => k,
        None => return Ok(TransformAction::Continue),
    };

    match kind.as_str() {
        // ---------------------------------------------------------------------
        // Skip nodes - remove entirely, promote children
        // ---------------------------------------------------------------------
        "expression_statement" => Ok(TransformAction::Skip),

        // ---------------------------------------------------------------------
        // Flatten nodes - transform children, then remove wrapper
        // ---------------------------------------------------------------------
        "variable_declarator" => Ok(TransformAction::Flatten),
        "class_body" => Ok(TransformAction::Flatten),

        // ---------------------------------------------------------------------
        // Name wrappers - inline identifier text directly
        // ---------------------------------------------------------------------
        "name" => {
            if let Some(parent) = get_parent(xot, node) {
                let parent_kind = get_element_name(xot, parent).unwrap_or_default();
                if matches!(parent_kind.as_str(),
                    "function_declaration" | "class_declaration" | "method_definition"
                    | "function" | "class" | "method"
                ) {
                    let children: Vec<_> = xot.children(node).collect();
                    for child in children {
                        if let Some(child_name) = get_element_name(xot, child) {
                            if child_name == "identifier" || child_name == "property_identifier" {
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

        // ---------------------------------------------------------------------
        // Binary/unary expressions - extract operator
        // ---------------------------------------------------------------------
        "binary_expression" | "unary_expression" | "assignment_expression"
        | "augmented_assignment_expression" | "update_expression" => {
            extract_operator(xot, node)?;
            if let Some(new_name) = map_element_name(&kind) {
                rename(xot, node, new_name);
            }
            Ok(TransformAction::Continue)
        }

        // ---------------------------------------------------------------------
        // Variable declarations - extract let/const/var modifier
        // ---------------------------------------------------------------------
        "lexical_declaration" | "variable_declaration" => {
            extract_keyword_modifiers(xot, node)?;
            rename(xot, node, "variable");
            Ok(TransformAction::Continue)
        }

        // ---------------------------------------------------------------------
        // Identifiers - classify as name or type based on context
        // ---------------------------------------------------------------------
        "identifier" | "property_identifier" => {
            let classification = classify_identifier(xot, node);
            rename(xot, node, classification);
            Ok(TransformAction::Continue)
        }
        "type_identifier" => {
            rename(xot, node, "type");
            Ok(TransformAction::Continue)
        }

        // ---------------------------------------------------------------------
        // Other nodes - just rename if needed
        // ---------------------------------------------------------------------
        _ => {
            if let Some(new_name) = map_element_name(&kind) {
                rename(xot, node, new_name);
            }
            Ok(TransformAction::Continue)
        }
    }
}

/// Map tree-sitter node kinds to semantic element names
fn map_element_name(kind: &str) -> Option<&'static str> {
    match kind {
        // Declarations
        "program" => Some("program"),
        "class_declaration" => Some("class"),
        "function_declaration" => Some("function"),
        "method_definition" => Some("method"),
        "arrow_function" => Some("lambda"),
        "interface_declaration" => Some("interface"),
        "type_alias_declaration" => Some("typealias"),
        "enum_declaration" => Some("enum"),
        "lexical_declaration" | "variable_declaration" => Some("variable"),

        // Parameters
        "formal_parameters" => Some("params"),
        "required_parameter" | "optional_parameter" => Some("param"),

        // Blocks
        "statement_block" => Some("block"),

        // Statements
        "return_statement" => Some("return"),
        "if_statement" => Some("if"),
        "else_clause" => Some("else"),
        "for_statement" => Some("for"),
        "while_statement" => Some("while"),
        "try_statement" => Some("try"),
        "catch_clause" => Some("catch"),
        "throw_statement" => Some("throw"),

        // Expressions
        "call_expression" => Some("call"),
        "new_expression" => Some("new"),
        "member_expression" => Some("member"),
        "assignment_expression" => Some("assign"),
        "binary_expression" => Some("binary"),
        "unary_expression" => Some("unary"),
        "ternary_expression" => Some("ternary"),
        "await_expression" => Some("await"),

        // Imports/Exports
        "import_statement" => Some("import"),
        "export_statement" => Some("export"),

        // Literals
        "string" => Some("string"),
        "number" => Some("number"),
        "true" => Some("true"),
        "false" => Some("false"),
        "null" => Some("null"),

        // Types
        "type_annotation" => Some("typeof"),
        "type_parameters" => Some("typeparams"),
        "type_parameter" => Some("typeparam"),

        // Default - no mapping
        _ => None,
    }
}

/// Extract operator from text children and add as `op` attribute
fn extract_operator(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    let texts = get_text_children(xot, node);

    // Find operator (skip punctuation)
    let operator = texts.iter().find(|t| {
        !t.chars().all(|c| matches!(c, '(' | ')' | ',' | ';' | '{' | '}' | '[' | ']'))
    });

    if let Some(op) = operator {
        set_attr(xot, node, "op", op);
    }

    Ok(())
}

/// Extract let/const/var keywords and add as empty modifier elements
fn extract_keyword_modifiers(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    let texts = get_text_children(xot, node);

    // Known keyword modifiers for TypeScript
    const MODIFIERS: &[&str] = &["let", "const", "var", "async", "export", "default"];

    // Find modifiers and prepend as empty elements (in reverse to maintain order)
    let found: Vec<&str> = texts.iter()
        .filter_map(|t| MODIFIERS.iter().find(|&&m| m == t).copied())
        .collect();

    for modifier in found.into_iter().rev() {
        prepend_empty_element(xot, node, modifier)?;
    }

    Ok(())
}

/// Classify an identifier as "name" or "type" based on context
fn classify_identifier(xot: &Xot, node: XotNode) -> &'static str {
    let parent = match get_parent(xot, node) {
        Some(p) => p,
        None => return "name",  // Default
    };

    let parent_kind = get_element_name(xot, parent).unwrap_or_default();

    // Check if followed by parameter list (function/method name)
    let siblings = get_following_siblings(xot, node);
    let has_param_sibling = siblings.iter().any(|&s| {
        get_element_name(xot, s)
            .map(|n| matches!(n.as_str(), "formal_parameters" | "parameters"))
            .unwrap_or(false)
    });

    match parent_kind.as_str() {
        // Method/function names followed by params
        "method_definition" | "function_declaration" | "arrow_function" if has_param_sibling => "name",

        // Type declarations - the identifier IS the name
        "class_declaration" | "interface_declaration" | "type_alias_declaration"
        | "enum_declaration" => "name",

        // Variable declarator - the identifier is the name
        "variable_declarator" => "name",

        // Parameter - the identifier is the parameter name
        "required_parameter" | "optional_parameter" => "name",

        // Property assignment - the key is a name
        "pair" => "name",

        // Default to type
        _ => "type",
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::parse_string_to_xot;
    use crate::output::{render_document, RenderOptions};

    #[test]
    fn test_typescript_transform() {
        let source = "let x = 1 + 2;";
        let result = parse_string_to_xot(source, "typescript", "<test>".to_string(), false).unwrap();

        let options = RenderOptions::default();
        let xml = render_document(&result.xot, result.root, &options);

        // Check transforms applied
        assert!(xml.contains("<binary"), "binary_expression should be renamed");
        assert!(xml.contains(r#"op="+""#), "operator should be extracted");
        assert!(xml.contains("<let"), "let should be extracted as modifier");
    }
}
