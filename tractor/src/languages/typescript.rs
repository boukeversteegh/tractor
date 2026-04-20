//! TypeScript/JavaScript transform logic
//!
//! This module owns ALL TypeScript-specific transformation rules.
//! No assumptions about other languages - this is self-contained.

use xot::{Xot, Node as XotNode};
use crate::xot_transform::{TransformAction, helpers::*};
use crate::output::syntax_highlight::SyntaxCategory;

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
        "class_body" | "interface_body" | "enum_body" => Ok(TransformAction::Flatten),

        // ---------------------------------------------------------------------
        // Name wrappers created by the builder for field="name".
        // Inline the single identifier/property_identifier child as text:
        //   <name><identifier>foo</identifier></name> -> <name>foo</name>
        // ---------------------------------------------------------------------
        "name" => {
            inline_single_identifier(xot, node)?;
            Ok(TransformAction::Continue)
        }

        // ---------------------------------------------------------------------
        // Call/member expressions - promote field attributes to role wrappers
        // ---------------------------------------------------------------------
        "call_expression" | "member_expression" => {
            promote_children_fields(xot, node)?;
            if let Some(new_name) = map_element_name(&kind) {
                rename(xot, node, new_name);
            }
            Ok(TransformAction::Continue)
        }

        // ---------------------------------------------------------------------
        // Role wrappers (created by field promotion above)
        // Inline identifiers with <ref/> marker
        // ---------------------------------------------------------------------
        "function" | "object" | "property" if !has_kind(xot, node) => {
            inline_identifier_with_ref(xot, node)?;
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
        // Identifiers are always names (definitions or references).
        // Tree-sitter uses `type_identifier` for type positions, so bare
        // identifiers never need a heuristic — they are never types.
        // ---------------------------------------------------------------------
        "identifier" | "property_identifier" => {
            rename(xot, node, "name");
            Ok(TransformAction::Continue)
        }
        "type_identifier" => {
            rename(xot, node, "type");
            Ok(TransformAction::Continue)
        }

        // ---------------------------------------------------------------------
        // Optional parameters - add <optional/> marker to distinguish from required
        // ---------------------------------------------------------------------
        "optional_parameter" => {
            prepend_empty_element(xot, node, "optional")?;
            rename(xot, node, "param");
            Ok(TransformAction::Continue)
        }

        // ---------------------------------------------------------------------
        // Required parameters - add <required/> marker (exhaustive with optional)
        // ---------------------------------------------------------------------
        "required_parameter" => {
            prepend_empty_element(xot, node, "required")?;
            rename(xot, node, "param");
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
        // Note: call_expression and member_expression are also handled explicitly
        // in the transform match for field promotion, then renamed via map_element_name.
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
        "true" | "false" => Some("bool"),
        "null" => Some("null"),

        // Types
        "type_annotation" => Some("typeof"),
        "type_parameters" => Some("typeparams"),
        "type_parameter" => Some("typeparam"),

        // Default - no mapping
        _ => None,
    }
}

/// Extract operator from text children and add as `<op>` child element
fn extract_operator(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    let texts = get_text_children(xot, node);

    // Find operator (skip punctuation)
    let operator = texts.iter().find(|t| {
        !t.chars().all(|c| matches!(c, '(' | ')' | ',' | ';' | '{' | '}' | '[' | ']'))
    });

    if let Some(op) = operator {
        prepend_op_element(xot, node, op)?;
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

/// Fields to promote to wrapper elements for call/member expressions
const PROMOTED_FIELDS: &[&str] = &["function", "object", "property"];

/// Check if a node has a `kind` attribute (i.e., it's a tree-sitter node, not a wrapper)
fn has_kind(xot: &Xot, node: XotNode) -> bool {
    get_kind(xot, node).is_some()
}

/// Promote field attributes on children to wrapper elements
fn promote_children_fields(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    let children: Vec<XotNode> = xot.children(node)
        .filter(|&c| xot.element(c).is_some())
        .collect();
    for child in children {
        promote_field_to_wrapper(xot, child, PROMOTED_FIELDS)?;
    }
    Ok(())
}

/// Inline an identifier child into the role wrapper with a `<ref/>` marker.
///
/// `<function><identifier>require</identifier></function>`
/// becomes `<function><ref/>require</function>`
///
/// If the child is not an identifier (e.g., member_expression), does nothing.
fn inline_identifier_with_ref(xot: &mut Xot, wrapper: XotNode) -> Result<(), xot::Error> {
    let children: Vec<_> = xot.children(wrapper).collect();
    for child in children {
        if let Some(child_name) = get_element_name(xot, child) {
            if child_name == "identifier" || child_name == "property_identifier" {
                if let Some(text) = get_text_content(xot, child) {
                    // Remove all children from wrapper
                    let all_children: Vec<_> = xot.children(wrapper).collect();
                    for c in all_children {
                        xot.detach(c)?;
                    }
                    // Add <ref/> marker and text
                    prepend_empty_element(xot, wrapper, "ref")?;
                    let text_node = xot.new_text(&text);
                    xot.append(wrapper, text_node)?;
                    return Ok(());
                }
            }
        }
    }
    Ok(())
}

/// If `node` contains a single identifier child, replace the node's children
/// with that identifier's text. Used to flatten builder-created wrappers like
/// `<name><identifier>foo</identifier></name>` to `<name>foo</name>`.
fn inline_single_identifier(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    let children: Vec<_> = xot.children(node).collect();
    for child in children {
        let child_name = match get_element_name(xot, child) {
            Some(n) => n,
            None => continue,
        };
        if !matches!(child_name.as_str(), "identifier" | "property_identifier") {
            continue;
        }
        let text = match get_text_content(xot, child) {
            Some(t) => t,
            None => continue,
        };
        let all_children: Vec<_> = xot.children(node).collect();
        for c in all_children {
            xot.detach(c)?;
        }
        let text_node = xot.new_text(&text);
        xot.append(node, text_node)?;
        return Ok(());
    }
    Ok(())
}

/// Map a transformed element name to a syntax category for highlighting
pub fn syntax_category(element: &str) -> SyntaxCategory {
    match element {
        // Identifiers
        "name" => SyntaxCategory::Identifier,
        "type" => SyntaxCategory::Type,
        "ref" => SyntaxCategory::Identifier,

        // Literals
        "string" => SyntaxCategory::String,
        "number" => SyntaxCategory::Number,
        "true" | "false" | "null" => SyntaxCategory::Keyword,

        // Keywords - declarations
        "class" | "interface" | "enum" | "typealias" => SyntaxCategory::Keyword,
        "function" | "method" => SyntaxCategory::Keyword,
        "variable" | "param" | "params" | "optional" | "required" => SyntaxCategory::Keyword,
        "import" | "export" => SyntaxCategory::Keyword,

        // Keywords - control flow
        "if" | "else" | "for" | "while" | "do" => SyntaxCategory::Keyword,
        "switch" | "case" | "default" => SyntaxCategory::Keyword,
        "try" | "catch" | "finally" | "throw" => SyntaxCategory::Keyword,
        "return" | "break" | "continue" | "yield" => SyntaxCategory::Keyword,

        // Keywords - modifiers
        "let" | "const" | "var" => SyntaxCategory::Keyword,
        "async" | "await" => SyntaxCategory::Keyword,
        "new" | "this" | "super" => SyntaxCategory::Keyword,

        // Functions/calls
        "call" => SyntaxCategory::Function,
        "lambda" => SyntaxCategory::Function,

        // Operators
        "op" => SyntaxCategory::Operator,
        _ if is_operator_marker(element) => SyntaxCategory::Operator,
        "binary" | "unary" | "assign" | "ternary" => SyntaxCategory::Operator,

        // Comments
        "comment" => SyntaxCategory::Comment,

        // Types
        "typeof" | "typeparams" | "typeparam" => SyntaxCategory::Type,

        // Structural elements - no color
        _ => SyntaxCategory::Default,
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::parse_string_to_xot;
    use crate::output::{render_document, RenderOptions};

    #[test]
    fn test_typescript_transform() {
        let source = "let x = 1 + 2;";
        let result = parse_string_to_xot(source, "typescript", "<test>".to_string(), None).unwrap();

        let options = RenderOptions::default();
        let xml = render_document(&result.xot, result.root, &options);

        // Check transforms applied
        assert!(xml.contains("<binary"), "binary_expression should be renamed");
        assert!(xml.contains("<op><plus/>+</op>"), "operator should be extracted with semantic marker");
        assert!(xml.contains("<let"), "let should be extracted as modifier");
    }

    #[test]
    fn test_optional_parameter_marker() {
        let source = "function f(a: string, b?: number) {}";
        let result = parse_string_to_xot(source, "typescript", "<test>".to_string(), None).unwrap();

        let options = RenderOptions::default();
        let xml = render_document(&result.xot, result.root, &options);

        // Count occurrences of <param> (not <params> or <parameters>) - should have 2
        let param_count = xml.matches("<param>").count() + xml.matches("<param ").count();
        assert_eq!(param_count, 2, "should have 2 params, got: {xml}");

        // Only the optional parameter should have <optional/>
        assert!(xml.contains("<optional/>"), "optional parameter should have <optional/> marker, got: {xml}");

        // The <optional/> should appear exactly once (only for b?)
        let optional_count = xml.matches("<optional/>").count();
        assert_eq!(optional_count, 1, "should have exactly 1 <optional/> marker, got: {xml}");
    }
}
