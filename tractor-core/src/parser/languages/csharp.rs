//! C# transform logic
//!
//! This module owns ALL C#-specific transformation rules.
//! No assumptions about other languages - this is self-contained.

use xot::{Xot, Node as XotNode};
use crate::xot_transform::{TransformAction, helpers::*};

/// Transform a C# AST node
pub fn transform(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let kind = match get_element_name(xot, node) {
        Some(k) => k,
        None => return Ok(TransformAction::Continue),
    };

    match kind.as_str() {
        // ---------------------------------------------------------------------
        // Flatten nodes - transform children, then remove wrapper
        // ---------------------------------------------------------------------
        "declaration_list" => Ok(TransformAction::Flatten),

        // ---------------------------------------------------------------------
        // Modifier wrappers - C# wraps modifiers in "modifier" elements
        // Convert <modifier>public</modifier> to <public/>
        // ---------------------------------------------------------------------
        "modifier" => {
            if let Some(text) = get_text_content(xot, node) {
                let text = text.trim();
                if is_known_modifier(text) {
                    rename(xot, node, text);
                    remove_text_children(xot, node)?;
                    // Remove location attributes for cleaner output
                    remove_attr(xot, node, "start");
                    remove_attr(xot, node, "end");
                    remove_attr(xot, node, "field");
                    return Ok(TransformAction::Done);
                }
            }
            Ok(TransformAction::Continue)
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
        "type_identifier" | "predefined_type" => {
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

/// Known C# modifiers
fn is_known_modifier(text: &str) -> bool {
    matches!(text,
        "public" | "private" | "protected" | "internal" |
        "static" | "async" | "abstract" | "virtual" | "override" |
        "sealed" | "readonly" | "const" | "partial"
    )
}

/// Map tree-sitter node kinds to semantic element names
fn map_element_name(kind: &str) -> Option<&'static str> {
    match kind {
        "compilation_unit" => Some("unit"),
        "class_declaration" => Some("class"),
        "struct_declaration" => Some("struct"),
        "interface_declaration" => Some("interface"),
        "enum_declaration" => Some("enum"),
        "record_declaration" => Some("record"),
        "method_declaration" => Some("method"),
        "constructor_declaration" => Some("ctor"),
        "property_declaration" => Some("prop"),
        "field_declaration" => Some("field"),
        "namespace_declaration" => Some("namespace"),
        "parameter_list" => Some("params"),
        "parameter" => Some("param"),
        "argument_list" => Some("args"),
        "argument" => Some("arg"),
        "generic_name" => Some("generic"),
        "nullable_type" => Some("nullable"),
        "array_type" => Some("array"),
        "block" => Some("block"),
        "return_statement" => Some("return"),
        "if_statement" => Some("if"),
        "else_clause" => Some("else"),
        "for_statement" => Some("for"),
        "foreach_statement" => Some("foreach"),
        "while_statement" => Some("while"),
        "try_statement" => Some("try"),
        "catch_clause" => Some("catch"),
        "throw_statement" => Some("throw"),
        "using_statement" => Some("using"),
        "invocation_expression" => Some("call"),
        "member_access_expression" => Some("member"),
        "object_creation_expression" => Some("new"),
        "assignment_expression" => Some("assign"),
        "binary_expression" => Some("binary"),
        "unary_expression" => Some("unary"),
        "conditional_expression" => Some("ternary"),
        "lambda_expression" => Some("lambda"),
        "await_expression" => Some("await"),
        "variable_declaration" => Some("var"),
        "variable_declarator" => Some("decl"),
        "local_declaration_statement" => Some("local"),
        "string_literal" => Some("string"),
        "integer_literal" => Some("int"),
        "real_literal" => Some("float"),
        "boolean_literal" => Some("bool"),
        "null_literal" => Some("null"),
        "attribute_list" => Some("attrs"),
        "attribute" => Some("attr"),
        "using_directive" => Some("import"),
        _ => None,
    }
}

/// Extract operator from text children and add as `op` attribute
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

/// Classify an identifier as "name" or "type" based on context
fn classify_identifier(xot: &Xot, node: XotNode) -> &'static str {
    let parent = match get_parent(xot, node) {
        Some(p) => p,
        None => return "type",  // Default for C#
    };

    let parent_kind = get_element_name(xot, parent).unwrap_or_default();

    // Check if in namespace declaration path
    let in_namespace = is_in_namespace_context(xot, node);
    if parent_kind == "qualified_name" && in_namespace {
        return "name";
    }

    // Check if followed by parameter list (method/ctor name)
    let siblings = get_following_siblings(xot, node);
    let has_param_sibling = siblings.iter().any(|&s| {
        get_element_name(xot, s)
            .map(|n| matches!(n.as_str(), "parameter_list" | "parameters"))
            .unwrap_or(false)
    });

    match parent_kind.as_str() {
        // Method/constructor names followed by params
        "method_declaration" | "constructor_declaration" if has_param_sibling => "name",

        // Type declarations - the identifier IS the name
        "class_declaration" | "struct_declaration" | "interface_declaration"
        | "enum_declaration" | "record_declaration" | "namespace_declaration" => "name",

        // Variable declarator - the identifier is the name
        "variable_declarator" => "name",

        // Parameter - the identifier is the parameter name
        "parameter" => "name",

        // Generic name - the identifier is the generic type name
        "generic_name" => "type",

        // Default to type
        _ => "type",
    }
}

/// Check if node is in a namespace declaration context
fn is_in_namespace_context(xot: &Xot, node: XotNode) -> bool {
    let mut current = get_parent(xot, node);
    while let Some(parent) = current {
        if let Some(kind) = get_element_name(xot, parent) {
            match kind.as_str() {
                "namespace_declaration" => return true,
                // Stop if we hit a type declaration
                "class_declaration" | "struct_declaration" | "interface_declaration"
                | "enum_declaration" | "record_declaration" => return false,
                _ => {}
            }
        }
        current = get_parent(xot, parent);
    }
    false
}

#[cfg(test)]
mod tests {
    use crate::parser::parse_string_to_xot;
    use crate::output::{render_document, RenderOptions};

    #[test]
    fn test_csharp_transform() {
        let source = r#"
public class Foo {
    public void Bar() { }
}
"#;
        let result = parse_string_to_xot(source, "csharp", "<test>".to_string(), false).unwrap();

        let options = RenderOptions::default();
        let xml = render_document(&result.xot, result.root, &options);

        // Check transforms applied
        assert!(xml.contains("<class"), "class_declaration should be renamed");
        assert!(xml.contains("<method"), "method_declaration should be renamed");
        assert!(xml.contains("<public"), "public modifier should be extracted");
    }
}
