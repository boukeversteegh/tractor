//! PHP transform logic.
//!
//! Applies the shared design principles:
//!   - Renames tree-sitter kinds to short, developer-friendly names.
//!   - Lifts visibility / static / final / abstract keywords to
//!     empty markers while preserving the source keyword as a
//!     dangling text sibling.
//!   - Flattens grammar wrappers (Principle #12) — parameter_list,
//!     arguments, declaration_list, property_element, ...
//!
//! Still rough — focuses on the most-visible constructs so queries
//! work uniformly. Refine as blueprint snapshots surface specifics.

use xot::{Xot, Node as XotNode};
use crate::xot_transform::{TransformAction, helpers::*};
use crate::output::syntax_highlight::SyntaxCategory;

/// Transform a PHP AST node
pub fn transform(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let kind = match get_element_name(xot, node) {
        Some(k) => k,
        None => return Ok(TransformAction::Continue),
    };

    match kind.as_str() {
        // Purely-grouping wrappers — Principle #12. Drop the
        // container so children become direct siblings of the
        // enclosing class / method / …
        "declaration_list"
        | "compound_statement"
        | "property_element"
        | "match_block"
        | "match_condition_list"
        | "namespace_name"
        | "namespace_use_clause"
        | "string_content"
        | "escape_sequence"
        | "array_element_initializer"
        => Ok(TransformAction::Flatten),

        // Expression statement is a grammar wrapper with no
        // semantic payload. Children become direct siblings of the
        // parent (Principle #12).
        //
        // NOTE: parenthesized_expression is deliberately NOT skipped
        // here — in PHP it interacts with nested ternaries in a
        // way that trips the xot walker (freed-node access). Left
        // intact for now; todo to revisit once the walker is
        // strengthened for Skip + consolidation interactions.
        "expression_statement" => Ok(TransformAction::Skip),

        // Qualified names (`App\Hello\Greeter`) collapse to a single
        // text leaf inside their enclosing <name> — same design as
        // C# qualified_name. The outer <name> field wrapper handles
        // the collapse; here we just flatten the inner wrapper so
        // its segments become siblings of the enclosing <name>,
        // which then consolidates.
        "qualified_name" => Ok(TransformAction::Flatten),

        // Comments — normalise tree-sitter's distinction between
        // line and block into the shared `<comment>` name.
        "comment" => Ok(TransformAction::Continue),

        // Flat lists (Principle #12) — parameters and arguments
        // become direct siblings with field="parameters" / "arguments".
        "formal_parameters" => {
            distribute_field_to_children(xot, node, "parameters");
            Ok(TransformAction::Flatten)
        }
        "arguments" if has_kind(xot, node) => {
            distribute_field_to_children(xot, node, "arguments");
            Ok(TransformAction::Flatten)
        }

        // Modifier wrappers. PHP's grammar gives us
        // `visibility_modifier`, `static_modifier`, `final_modifier`,
        // `abstract_modifier`, `readonly_modifier` — each a text
        // token like "public" / "static". Convert to empty markers
        // with the source keyword preserved as a dangling sibling.
        "visibility_modifier"
        | "static_modifier"
        | "final_modifier"
        | "abstract_modifier"
        | "readonly_modifier"
        | "class_modifier" => {
            if let Some(text) = get_text_content(xot, node) {
                let text = text.trim().to_string();
                if !text.is_empty() {
                    rename_to_marker(xot, node, &text)?;
                    insert_text_after(xot, node, &text)?;
                    return Ok(TransformAction::Done);
                }
            }
            Ok(TransformAction::Continue)
        }

        // Base class / implements — wrap the type reference in <type>
        // (Principle #14).
        "base_clause" => {
            rename(xot, node, "extends");
            Ok(TransformAction::Continue)
        }
        "class_interface_clause" => {
            rename(xot, node, "implements");
            Ok(TransformAction::Continue)
        }

        // PHP emits `name` directly on identifiers — our field
        // wrappings already produce <name>foo</name>, so nothing to
        // rewrite here except collapsing the occasional <name><name>…</name></name>
        // that field+identifier double-wrapping creates.
        "name" => {
            let children: Vec<_> = xot.children(node).collect();
            let element_children: Vec<_> = children
                .iter()
                .copied()
                .filter(|&c| xot.element(c).is_some())
                .collect();
            if element_children.len() == 1 {
                let child = element_children[0];
                if get_element_name(xot, child).as_deref() == Some("name") {
                    if let Some(text) = get_text_content(xot, child) {
                        for c in children {
                            xot.detach(c)?;
                        }
                        let text_node = xot.new_text(&text);
                        xot.append(node, text_node)?;
                        return Ok(TransformAction::Done);
                    }
                }
            }
            Ok(TransformAction::Continue)
        }

        // Binary / assignment / unary expressions — lift the operator.
        "binary_expression" | "assignment_expression" | "unary_op_expression" => {
            extract_operator(xot, node)?;
            if let Some(new_name) = map_element_name(&kind) {
                rename(xot, node, new_name);
            }
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

fn has_kind(xot: &Xot, node: XotNode) -> bool {
    get_kind(xot, node).is_some()
}

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

fn map_element_name(kind: &str) -> Option<&'static str> {
    match kind {
        "program" => Some("program"),
        "namespace_definition" => Some("namespace"),
        "namespace_use_declaration" => Some("use"),
        "class_declaration" => Some("class"),
        "interface_declaration" => Some("interface"),
        "trait_declaration" => Some("trait"),
        "enum_declaration" => Some("enum"),
        "method_declaration" => Some("method"),
        "function_definition" => Some("function"),
        "property_declaration" => Some("field"),
        "const_declaration" => Some("const"),
        "enum_case" => Some("constant"),
        "formal_parameter" | "simple_parameter" | "variadic_parameter" => Some("parameter"),
        // property_element / formal_parameters flattened above
        "argument" => Some("argument"),
        // arguments flattened above when has kind
        "return_statement" => Some("return"),
        "if_statement" => Some("if"),
        "else_clause" => Some("else"),
        "else_if_clause" | "elseif_clause" => Some("else_if"),
        "for_statement" => Some("for"),
        "foreach_statement" => Some("foreach"),
        "while_statement" => Some("while"),
        "do_statement" => Some("do"),
        "switch_statement" => Some("switch"),
        "case_statement" => Some("case"),
        "default_statement" => Some("default"),
        "try_statement" => Some("try"),
        "catch_clause" => Some("catch"),
        "finally_clause" => Some("finally"),
        "throw_expression" => Some("throw"),
        "echo_statement" => Some("echo"),
        "continue_statement" => Some("continue"),
        "break_statement" => Some("break"),
        "match_expression" => Some("match"),
        "match_conditional_expression" => Some("arm"),
        "match_default_expression" => Some("arm"),
        "class_constant_access_expression" => Some("member"),
        "subscript_expression" => Some("index"),
        "yield_expression" => Some("yield"),
        "require_expression" | "require_once_expression" | "include_expression" | "include_once_expression" => Some("require"),
        "type_cast_expression" => Some("cast"),
        "print_intrinsic" => Some("print"),
        "exit_intrinsic" | "exit_statement" => Some("exit"),
        "function_call_expression" => Some("call"),
        "member_call_expression" => Some("call"),
        "scoped_call_expression" => Some("call"),
        "member_access_expression" => Some("member"),
        "scoped_property_access_expression" => Some("member"),
        "object_creation_expression" => Some("new"),
        "cast_expression" => Some("cast"),
        "assignment_expression" => Some("assign"),
        "binary_expression" => Some("binary"),
        "unary_op_expression" => Some("unary"),
        "conditional_expression" => Some("ternary"),
        "array_creation_expression" => Some("array"),
        "string" | "encapsed_string" => Some("string"),
        "integer" => Some("int"),
        "float" => Some("float"),
        "boolean" => Some("bool"),
        "null" => Some("null"),
        "variable_name" => Some("variable"),
        "primitive_type" | "named_type" | "union_type" | "optional_type" => Some("type"),
        _ => None,
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
        "bool" | "null" => SyntaxCategory::Keyword,

        // Keywords
        "namespace" | "use" | "class" | "interface" | "trait" | "enum" => SyntaxCategory::Keyword,
        "function" | "method" | "field" | "const" | "constant" => SyntaxCategory::Keyword,
        "parameter" | "parameters" | "argument" | "arguments" => SyntaxCategory::Keyword,
        "if" | "else" | "else_if" | "switch" | "case" | "default" => SyntaxCategory::Keyword,
        "for" | "foreach" | "while" | "do" => SyntaxCategory::Keyword,
        "try" | "catch" | "finally" | "throw" => SyntaxCategory::Keyword,
        "return" | "break" | "continue" => SyntaxCategory::Keyword,
        "extends" | "implements" => SyntaxCategory::Keyword,
        "public" | "private" | "protected" | "static" | "final" | "abstract"
        | "readonly" => SyntaxCategory::Keyword,

        // Functions/calls
        "call" | "new" => SyntaxCategory::Function,

        // Operators
        "op" => SyntaxCategory::Operator,
        _ if is_operator_marker(element) => SyntaxCategory::Operator,
        "binary" | "unary" | "assign" | "ternary" | "cast" => SyntaxCategory::Operator,

        // Comments
        "comment" => SyntaxCategory::Comment,

        // Structural elements - no color
        _ => SyntaxCategory::Default,
    }
}
