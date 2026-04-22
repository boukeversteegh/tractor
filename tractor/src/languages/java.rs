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
        "class_body" | "interface_body" | "block"
        | "enum_body" | "field_declaration_list" | "type_list"
        | "constructor_body" => Ok(TransformAction::Flatten),

        // ---------------------------------------------------------------------
        // Flat lists (Principle #12)
        // ---------------------------------------------------------------------
        "formal_parameters" => {
            distribute_field_to_children(xot, node, "parameters");
            Ok(TransformAction::Flatten)
        }
        "argument_list" => {
            distribute_field_to_children(xot, node, "arguments");
            Ok(TransformAction::Flatten)
        }
        "type_arguments" => {
            distribute_field_to_children(xot, node, "arguments");
            Ok(TransformAction::Flatten)
        }
        "type_parameters" => {
            distribute_field_to_children(xot, node, "generics");
            rename(xot, node, "generics");
            Ok(TransformAction::Flatten)
        }

        // ---------------------------------------------------------------------
        // Generic type references: apply the C# pattern.
        //   generic_type(<type_identifier>Foo</type_identifier>, type_arguments)
        //     -> <type><generic/>Foo <type field="arguments">Bar</type>...</type>
        // ---------------------------------------------------------------------
        "generic_type" => {
            rewrite_generic_type(xot, node, &["type_identifier", "scoped_type_identifier"])?;
            Ok(TransformAction::Continue)
        }

        // ---------------------------------------------------------------------
        // Name wrappers created by the builder for field="name".
        // Inline the single identifier child as text:
        //   <name><identifier>foo</identifier></name> -> <name>foo</name>
        // ---------------------------------------------------------------------
        "name" => {
            inline_single_identifier(xot, node)?;
            Ok(TransformAction::Continue)
        }

        // ---------------------------------------------------------------------
        // Modifier wrappers - Java wraps modifiers in "modifiers" element
        // Convert <modifiers>public static</modifiers> to <public/><static/>
        // Also inserts <package/> if no access modifier found (Principle #9)
        // ---------------------------------------------------------------------
        "modifiers" => {
            let mut has_access = false;
            if let Some(text) = get_text_content(xot, node) {
                let words: Vec<&str> = text.split_whitespace().collect();
                for word in &words {
                    if is_access_modifier(word) {
                        has_access = true;
                    }
                }
                // Insert known modifiers as empty elements before this node
                for modifier in words.iter().rev() {
                    if is_known_modifier(modifier) {
                        insert_empty_before(xot, node, modifier)?;
                    }
                }
            }
            if !has_access {
                insert_empty_before(xot, node, "package")?;
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

        // Ternary expression — surgically wrap `alternative` in `<else>`.
        // See TS transform for rationale.
        "ternary_expression" => {
            wrap_field_child(xot, node, "alternative", "else")?;
            rename(xot, node, "ternary");
            Ok(TransformAction::Continue)
        }

        // ---------------------------------------------------------------------
        // Identifiers are always names (definitions or references).
        // Tree-sitter uses `type_identifier` for type positions, so bare
        // identifiers never need a heuristic — they are never types.
        // ---------------------------------------------------------------------
        "identifier" => {
            rename(xot, node, "name");
            Ok(TransformAction::Continue)
        }
        "type_identifier" | "integral_type" | "floating_point_type"
        | "boolean_type" | "void_type" => {
            rename(xot, node, "type");
            wrap_text_in_name(xot, node)?;
            Ok(TransformAction::Continue)
        }

        // ---------------------------------------------------------------------
        // Declarations — prepend <package/> if no modifiers child
        // ---------------------------------------------------------------------
        "class_declaration" | "interface_declaration" | "enum_declaration"
        | "method_declaration" | "constructor_declaration" | "field_declaration" => {
            if !has_modifiers_child(xot, node) {
                // Members declared inside an interface are implicitly public
                // (Java spec §9.4). Top-level types and class members default
                // to package access.
                let default = if is_inside_interface(xot, node) {
                    "public"
                } else {
                    "package"
                };
                prepend_empty_element(xot, node, default)?;
            }
            // Java's grammar tags the method return type as field="type"
            // (the same field name used on parameters), so the builder
            // can't wrap it by name. Do it here for methods only.
            if kind == "method_declaration" {
                wrap_method_return_type(xot, node)?;
            }
            if let Some(new_name) = map_element_name(&kind) {
                rename(xot, node, new_name);
            }
            Ok(TransformAction::Continue)
        }

        // ---------------------------------------------------------------------
        // Other nodes - just rename if needed
        // ---------------------------------------------------------------------
        _ => {
            if let Some(new_name) = map_element_name(&kind) {
                rename(xot, node, new_name);
                if new_name == "type" {
                    wrap_text_in_name(xot, node)?;
                }
            }
            Ok(TransformAction::Continue)
        }
    }
}

/// Walk up from `node` looking for an enclosing `interface_declaration`.
/// Stops at the first class/enum/record (which would override the default).
fn is_inside_interface(xot: &Xot, node: XotNode) -> bool {
    let mut current = get_parent(xot, node);
    while let Some(parent) = current {
        if let Some(kind) = get_kind(xot, parent) {
            match kind.as_str() {
                "interface_declaration" => return true,
                "class_declaration" | "enum_declaration" | "record_declaration" => return false,
                // interface_body and class_body are transparent wrappers
                _ => {}
            }
        }
        current = get_parent(xot, parent);
    }
    false
}

/// Wrap a method's return type (the child with field="type") in a `<returns>`
/// element so it's symmetric with C#/Rust/TS. Java's tree-sitter grammar
/// uses the ambiguous field name `type` for both return types and parameter
/// types, so this can't be done generically by the builder.
fn wrap_method_return_type(xot: &mut Xot, method: XotNode) -> Result<(), xot::Error> {
    let children: Vec<XotNode> = xot.children(method).collect();
    for child in children {
        if xot.element(child).is_none() {
            continue;
        }
        if get_attr(xot, child, "field").as_deref() != Some("type") {
            continue;
        }
        let returns_name = xot.add_name("returns");
        let wrapper = xot.new_element(returns_name);
        copy_source_location(xot, child, wrapper);
        set_attr(xot, wrapper, "field", "returns");
        xot.insert_before(child, wrapper)?;
        xot.detach(child)?;
        xot.append(wrapper, child)?;
        // Drop field="type" on the inner type — it's now "returns" at the wrapper level
        remove_attr(xot, child, "field");
        break;
    }
    Ok(())
}

/// Check if text is an access modifier keyword
fn is_access_modifier(text: &str) -> bool {
    matches!(text, "public" | "private" | "protected")
}

/// Check if a declaration node has a `modifiers` child element
fn has_modifiers_child(xot: &Xot, node: XotNode) -> bool {
    for child in xot.children(node) {
        if let Some(name) = get_element_name(xot, child) {
            if name == "modifiers" {
                return true;
            }
        }
    }
    false
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
        "constructor_declaration" => Some("constructor"),
        // constructor_body is flattened (handled above) — the `body` wrapper
        // already comes from the field-wrapping pass.
        "field_declaration" => Some("field"),
        "enum_constant" => Some("constant"),
        // formal_parameters and argument_list are flattened via Principle #12 above
        "formal_parameter" => Some("param"),
        "generic_type" => Some("generic"),
        "array_type" => Some("array"),
        "scoped_identifier" | "scoped_type_identifier" => Some("path"),
        "super_interfaces" => Some("implements"),
        "type_bound" => Some("bound"),
        "type_parameter" => Some("generic"),
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
        // ternary_expression handled above
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
        prepend_op_element(xot, node, op)?;
    }
    Ok(())
}

/// If `node` contains a single identifier child, replace the node's children
/// with that identifier's text. Used to flatten builder-created wrappers like
/// `<name><identifier>foo</identifier></name>` to `<name>foo</name>`.
fn inline_single_identifier(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    let children: Vec<_> = xot.children(node).collect();
    for child in children {
        if get_element_name(xot, child).as_deref() != Some("identifier") {
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

        // Literals
        "string" => SyntaxCategory::String,
        "int" | "float" => SyntaxCategory::Number,
        "true" | "false" | "null" => SyntaxCategory::Keyword,

        // Keywords - declarations
        "class" | "interface" | "enum" => SyntaxCategory::Keyword,
        "method" | "constructor" | "field" => SyntaxCategory::Keyword,
        "param" | "params" => SyntaxCategory::Keyword,
        "import" | "package" => SyntaxCategory::Keyword,

        // Keywords - control flow
        "if" | "else" => SyntaxCategory::Keyword,
        "for" | "foreach" | "while" | "do" => SyntaxCategory::Keyword,
        "switch" | "case" => SyntaxCategory::Keyword,
        "try" | "catch" | "finally" | "throw" => SyntaxCategory::Keyword,
        "return" | "break" | "continue" => SyntaxCategory::Keyword,

        // Keywords - modifiers (note: "package" is covered earlier)
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
        _ if is_operator_marker(element) => SyntaxCategory::Operator,
        "binary" | "unary" | "assign" | "ternary" => SyntaxCategory::Operator,

        // Comments
        "comment" => SyntaxCategory::Comment,

        // Structural elements - no color
        _ => SyntaxCategory::Default,
    }
}
