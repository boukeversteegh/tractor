//! Java transform logic
//!
//! This module owns ALL Java-specific transformation rules.
//! No assumptions about other languages - this is self-contained.

use xot::{Xot, Node as XotNode};
use crate::xot_transform::{TransformAction, helpers::*};
use crate::output::syntax_highlight::SyntaxCategory;

/// Transform a Java AST node
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
        "class_body" | "interface_body" | "block" => Ok(TransformAction::Flatten),

        // ---------------------------------------------------------------------
        // Name wrappers - inline identifier text directly
        // ---------------------------------------------------------------------
        "name" => {
            if let Some(parent) = get_parent(xot, node) {
                let parent_kind = get_element_name(xot, parent).unwrap_or_default();
                if matches!(parent_kind.as_str(),
                    "class_declaration" | "interface_declaration" | "enum_declaration"
                    | "method_declaration" | "constructor_declaration"
                    | "class" | "interface" | "enum" | "method" | "ctor"
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

        // ---------------------------------------------------------------------
        // Modifier wrappers - Java wraps modifiers in "modifiers" element
        // Convert <modifiers>public static</modifiers> to <public/><static/>
        // ---------------------------------------------------------------------
        "modifiers" => {
            if let Some(text) = get_text_content(xot, node) {
                let words: Vec<&str> = text.split_whitespace().collect();
                // Insert known modifiers as empty elements before this node
                for modifier in words.iter().rev() {
                    if is_known_modifier(modifier) {
                        insert_empty_before(xot, node, modifier)?;
                    }
                }
            }
            // Remove the wrapper node entirely
            detach(xot, node)?;
            Ok(TransformAction::Done)
        }

        // ---------------------------------------------------------------------
        // Binary/unary expressions - extract operator
        // ---------------------------------------------------------------------
        "binary_expression" | "unary_expression" | "assignment_expression" => {
            extract_operator(xot, node)?;
            if let Some(new_name) = map_element_name(&kind) {
                rename(xot, node, new_name);
            }
            Ok(TransformAction::Continue)
        }

        // ---------------------------------------------------------------------
        // Identifiers - classify as name or type based on context
        // ---------------------------------------------------------------------
        "identifier" => {
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

/// Known Java modifiers
fn is_known_modifier(text: &str) -> bool {
    matches!(text,
        "public" | "private" | "protected" |
        "static" | "final" | "abstract" | "synchronized" |
        "volatile" | "transient" | "native" | "strictfp"
    )
}

/// Map tree-sitter node kinds to semantic element names
fn map_element_name(kind: &str) -> Option<&'static str> {
    match kind {
        "program" => Some("program"),
        "class_declaration" => Some("class"),
        "interface_declaration" => Some("interface"),
        "enum_declaration" => Some("enum"),
        "method_declaration" => Some("method"),
        "constructor_declaration" => Some("ctor"),
        "field_declaration" => Some("field"),
        "formal_parameters" => Some("params"),
        "formal_parameter" => Some("param"),
        "argument_list" => Some("args"),
        "generic_type" => Some("generic"),
        "array_type" => Some("array"),
        "return_statement" => Some("return"),
        "if_statement" => Some("if"),
        "else_clause" => Some("else"),
        "for_statement" => Some("for"),
        "enhanced_for_statement" => Some("foreach"),
        "while_statement" => Some("while"),
        "try_statement" => Some("try"),
        "catch_clause" => Some("catch"),
        "finally_clause" => Some("finally"),
        "throw_statement" => Some("throw"),
        "switch_expression" => Some("switch"),
        "switch_block_statement_group" => Some("case"),
        "method_invocation" => Some("call"),
        "object_creation_expression" => Some("new"),
        "field_access" => Some("member"),
        "array_access" => Some("index"),
        "assignment_expression" => Some("assign"),
        "binary_expression" => Some("binary"),
        "unary_expression" => Some("unary"),
        "ternary_expression" => Some("ternary"),
        "lambda_expression" => Some("lambda"),
        "string_literal" => Some("string"),
        "decimal_integer_literal" => Some("int"),
        "decimal_floating_point_literal" => Some("float"),
        "true" => Some("true"),
        "false" => Some("false"),
        "null_literal" => Some("null"),
        "import_declaration" => Some("import"),
        "package_declaration" => Some("package"),
        _ => None,
    }
}

/// Extract operator from text children and add as `<op>` child element
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

/// Classify an identifier as "name" or "type" based on context
fn classify_identifier(xot: &Xot, node: XotNode) -> &'static str {
    let parent = match get_parent(xot, node) {
        Some(p) => p,
        None => return "type",  // Default for Java
    };
    let parent_kind = get_element_name(xot, parent).unwrap_or_default();

    // Check if followed by parameter list (method/ctor name)
    let siblings = get_following_siblings(xot, node);
    let has_param_sibling = siblings.iter().any(|&s| {
        get_element_name(xot, s)
            .map(|n| matches!(n.as_str(), "formal_parameters" | "parameters"))
            .unwrap_or(false)
    });

    match parent_kind.as_str() {
        // Method/constructor names followed by params
        "method_declaration" | "constructor_declaration" if has_param_sibling => "name",

        // Type declarations - the identifier IS the name
        "class_declaration" | "interface_declaration" | "enum_declaration" => "name",

        // Variable declarator - the identifier is the name
        "variable_declarator" => "name",

        // Parameter - the identifier is the parameter name
        "formal_parameter" => "name",

        // Default to type
        _ => "type",
    }
}

/// Map a transformed element name to a syntax category for highlighting
pub fn syntax_category(element: &str) -> SyntaxCategory {
    match element {
        // Identifiers
        "name" => SyntaxCategory::Identifier,
        "type" => SyntaxCategory::Type,

        // Literals
        "string" => SyntaxCategory::String,
        "int" | "float" => SyntaxCategory::Number,
        "true" | "false" | "null" => SyntaxCategory::Keyword,

        // Keywords - declarations
        "class" | "interface" | "enum" => SyntaxCategory::Keyword,
        "method" | "ctor" | "field" => SyntaxCategory::Keyword,
        "param" | "params" => SyntaxCategory::Keyword,
        "import" | "package" => SyntaxCategory::Keyword,

        // Keywords - control flow
        "if" | "else" => SyntaxCategory::Keyword,
        "for" | "foreach" | "while" | "do" => SyntaxCategory::Keyword,
        "switch" | "case" => SyntaxCategory::Keyword,
        "try" | "catch" | "finally" | "throw" => SyntaxCategory::Keyword,
        "return" | "break" | "continue" => SyntaxCategory::Keyword,

        // Keywords - modifiers
        "public" | "private" | "protected" => SyntaxCategory::Keyword,
        "static" | "final" | "abstract" | "synchronized" => SyntaxCategory::Keyword,
        "volatile" | "transient" | "native" | "strictfp" => SyntaxCategory::Keyword,
        "new" | "this" | "super" => SyntaxCategory::Keyword,

        // Types
        "generic" | "array" => SyntaxCategory::Type,

        // Functions/calls
        "call" => SyntaxCategory::Function,
        "lambda" => SyntaxCategory::Function,

        // Operators
        "op" => SyntaxCategory::Operator,
        "binary" | "unary" | "assign" | "ternary" => SyntaxCategory::Operator,

        // Comments
        "comment" => SyntaxCategory::Comment,

        // Structural elements - no color
        _ => SyntaxCategory::Default,
    }
}
