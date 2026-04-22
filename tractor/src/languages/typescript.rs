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
        // class_heritage is a purely-grouping wrapper around `extends_clause`
        // and `implements_clause`. Drop it so those clauses become direct
        // children of the class, under their renamed forms.
        "class_heritage" => Ok(TransformAction::Flatten),
        // Template string parts: inline the raw text into the enclosing
        // `<template>` so a template literal reads as text with interpolation
        // children, not as a soup of grammar-internal wrappers.
        "string_fragment" | "string_start" | "string_end" => {
            Ok(TransformAction::Flatten)
        }

        // private_property_identifier is handled inside `inline_single_identifier`
        // when the enclosing <name> wrapper is processed: the leading `#` is
        // stripped and a <private/> marker is lifted onto the enclosing
        // field/property node.

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
        // Call/member expressions — field wrapping (function→callee,
        // object→object, property→property) is handled by apply_field_wrappings
        // per TS_FIELD_WRAPPINGS, so we just rename the outer node here.
        // ---------------------------------------------------------------------
        "call_expression" | "member_expression" => {
            if let Some(new_name) = map_element_name(&kind) {
                rename(xot, node, new_name);
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
        // Flat lists (Principle #12): drop the list wrapper; children become
        // direct siblings of the enclosing element and carry field="<plural>"
        // so non-XML serializers can group them back into an array.
        //
        // tree-sitter-javascript emits bare `identifier` for untyped params
        // (tree-sitter-typescript wraps them in required_parameter). Promote
        // those to `<param>` here so the semantic tree is consistent across
        // JS and TS — every parameter is a <param>.
        // ---------------------------------------------------------------------
        "formal_parameters" => {
            wrap_bare_identifier_params(xot, node)?;
            distribute_field_to_children(xot, node, "parameters");
            Ok(TransformAction::Flatten)
        }
        "arguments" if has_kind(xot, node) => {
            distribute_field_to_children(xot, node, "arguments");
            Ok(TransformAction::Flatten)
        }
        "type_arguments" => {
            distribute_field_to_children(xot, node, "arguments");
            Ok(TransformAction::Flatten)
        }

        // Type annotations (`:` prefix) are just a colon-prefixed form of
        // the underlying type. Drop the wrapper; the colon stays as a text
        // sibling for renderability and the actual `<type>` appears as a
        // child directly.
        "type_annotation" => Ok(TransformAction::Flatten),

        // Generic type references: apply the C# pattern.
        //   generic_type(name=Foo, type_arguments=[Bar, Baz])
        //     -> <type><generic/>Foo <type field="arguments">Bar</type>
        //                             <type field="arguments">Baz</type></type>
        "generic_type" => {
            rewrite_generic_type(xot, node, &["type_identifier", "identifier"])?;
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
        // Functions/methods — lift `async` keyword and generator `*` prefix
        // to empty marker children on the complex node (Principle #13).
        // Mirrors Python's function_definition handling.
        // ---------------------------------------------------------------------
        "method_definition" | "function_declaration" | "function_expression"
        | "arrow_function" | "generator_function_declaration"
        | "generator_function" => {
            extract_function_markers(xot, node)?;
            if let Some(new_name) = map_element_name(&kind) {
                rename(xot, node, new_name);
            }
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
        "function_declaration" | "function_expression" => Some("function"),
        "generator_function_declaration" | "generator_function" => Some("function"),
        "method_definition" => Some("method"),
        "arrow_function" => Some("lambda"),
        "interface_declaration" => Some("interface"),
        "type_alias_declaration" => Some("alias"),
        "enum_declaration" => Some("enum"),
        "lexical_declaration" | "variable_declaration" => Some("variable"),

        // Parameters — formal_parameters is flattened; individual params below
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
        "yield_expression" => Some("yield"),
        "as_expression" => Some("as"),

        // Classes / members
        // class_heritage is flattened in the match above; the inner clauses
        // are the semantic nodes (extends_clause → <extends>, etc.).
        "extends_clause" => Some("extends"),
        "implements_clause" => Some("implements"),
        "field_definition" | "public_field_definition" => Some("field"),

        // Template strings
        "template_string" => Some("template"),
        "template_substitution" => Some("interpolation"),

        // Imports/Exports
        "import_statement" => Some("import"),
        "export_statement" => Some("export"),

        // Literals
        "string" => Some("string"),
        "number" => Some("number"),
        "true" | "false" => Some("bool"),
        "null" => Some("null"),

        // Types
        // type_annotation is flattened in the match above.
        "predefined_type" => Some("type"),
        "type_parameters" => Some("generics"),
        "type_parameter" => Some("generic"),

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

/// Lift `async` keyword and generator `*` prefix on functions/methods to
/// empty marker children on the node. Leaves all other children intact.
/// The source keyword/token remains as a text sibling for renderability.
///
/// Text children may concatenate multiple tokens (e.g. `"async function"`
/// or `"function*"`), so we scan token-wise for the keywords.
fn extract_function_markers(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    let texts = get_text_children(xot, node);
    let mut has_async = false;
    let mut has_star = false;
    for t in &texts {
        for tok in t.split_whitespace() {
            if tok == "async" {
                has_async = true;
            }
            // A generator marker may appear as a standalone "*" token or
            // attached to "function" (e.g. "function*"). Either way, if
            // any token contains '*' and is part of a function/method
            // header, treat it as a generator marker.
            if tok == "*" || tok.ends_with('*') || tok.starts_with('*') {
                has_star = true;
            }
        }
    }
    // Prepend in reverse so final order is <async/><generator/>...
    if has_star {
        prepend_empty_element(xot, node, "generator")?;
    }
    if has_async {
        prepend_empty_element(xot, node, "async")?;
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

/// Wrap each bare `identifier` child of a parameter list in a `<param>`
/// element. Harmonises JS (grammar: `formal_parameters → identifier`)
/// with TS (grammar: `formal_parameters → required_parameter → identifier`)
/// so the semantic tree shape is the same.
fn wrap_bare_identifier_params(xot: &mut Xot, list: XotNode) -> Result<(), xot::Error> {
    let children: Vec<XotNode> = xot.children(list)
        .filter(|&c| xot.element(c).is_some())
        .collect();
    for child in children {
        let kind = get_element_name(xot, child);
        if kind.as_deref() != Some("identifier") {
            continue;
        }
        let param_name = xot.add_name("param");
        let param = xot.new_element(param_name);
        copy_source_location(xot, child, param);
        xot.insert_before(child, param)?;
        xot.detach(child)?;
        xot.append(param, child)?;
    }
    Ok(())
}

/// Check if a node has a `kind` attribute (i.e., it's a tree-sitter node, not a wrapper)
fn has_kind(xot: &Xot, node: XotNode) -> bool {
    get_kind(xot, node).is_some()
}

/// If `node` contains a single identifier child, replace the node's children
/// with that identifier's text. Used to flatten builder-created wrappers like
/// `<name><identifier>foo</identifier></name>` to `<name>foo</name>`.
///
/// For `private_property_identifier` the leading `#` is stripped and a
/// `<private/>` marker is prepended to the enclosing field/property — the
/// text "#foo" is purely a sigil, not part of the name, and the marker
/// follows Principle #7 (modifiers as empty elements).
fn inline_single_identifier(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    let children: Vec<_> = xot.children(node).collect();
    for child in children {
        let child_name = match get_element_name(xot, child) {
            Some(n) => n,
            None => continue,
        };
        if !matches!(
            child_name.as_str(),
            "identifier" | "property_identifier" | "private_property_identifier",
        ) {
            continue;
        }
        let text = match get_text_content(xot, child) {
            Some(t) => t,
            None => continue,
        };
        let is_private = child_name == "private_property_identifier";
        let clean_text = if is_private {
            text.trim_start_matches('#').to_string()
        } else {
            text
        };
        let all_children: Vec<_> = xot.children(node).collect();
        for c in all_children {
            xot.detach(c)?;
        }
        let text_node = xot.new_text(&clean_text);
        xot.append(node, text_node)?;
        if is_private {
            if let Some(parent) = get_parent(xot, node) {
                let already = xot.children(parent).any(|c| {
                    get_element_name(xot, c).as_deref() == Some("private")
                });
                if !already {
                    prepend_empty_element(xot, parent, "private")?;
                }
            }
        }
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
        "class" | "interface" | "enum" | "alias" => SyntaxCategory::Keyword,
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
        "typeof" | "generics" | "generic" => SyntaxCategory::Type,

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
