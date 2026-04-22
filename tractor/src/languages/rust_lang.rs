//! Rust transform logic

use xot::{Xot, Node as XotNode};
use crate::xot_transform::{TransformAction, helpers::*};
use crate::output::syntax_highlight::SyntaxCategory;

/// Transform a Rust AST node
pub fn transform(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let kind = match get_element_name(xot, node) {
        Some(k) => k,
        None => return Ok(TransformAction::Continue),
    };

    match kind.as_str() {
        "expression_statement" => Ok(TransformAction::Skip),
        "block" | "declaration_list" => Ok(TransformAction::Flatten),

        // Pure grouping wrappers around a list of homogeneous children
        // — drop the wrapper, the children become direct siblings of the
        // enclosing struct/expression (Principle #12).
        "field_declaration_list" | "field_initializer_list" => {
            Ok(TransformAction::Flatten)
        }

        // Flat lists (Principle #12)
        "parameters" if has_kind(xot, node) => {
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

        // Generic type references: apply the C# pattern.
        //   generic_type(<type_identifier>Vec</type_identifier>, type_arguments)
        //     -> <type><generic/>Vec <type field="arguments">i32</type>...</type>
        "generic_type" => {
            rewrite_generic_type(xot, node, &["type_identifier", "scoped_type_identifier"])?;
            Ok(TransformAction::Continue)
        }

        // Name wrappers created by the builder for field="name".
        // Inline the single identifier/type_identifier/field_identifier child as text:
        //   <name><identifier>foo</identifier></name> -> <name>foo</name>
        "name" => {
            inline_single_identifier(xot, node)?;
            Ok(TransformAction::Continue)
        }

        // Visibility modifier (pub, pub(crate), etc.)
        "visibility_modifier" => {
            if let Some(text) = get_text_content(xot, node) {
                let text = text.trim().to_string();
                rename_to_marker(xot, node, "pub")?;
                // Add restriction detail as child element
                if let Some(start) = text.find('(') {
                    if let Some(end) = text.find(')') {
                        let inner = text[start+1..end].trim();
                        match inner {
                            "crate" => { prepend_empty_element(xot, node, "crate")?; }
                            "super" => { prepend_empty_element(xot, node, "super")?; }
                            _ if inner.starts_with("in ") => {
                                let path = inner[3..].trim();
                                prepend_element_with_text(xot, node, "in", path)?;
                            }
                            _ => {}
                        }
                    }
                }
                return Ok(TransformAction::Done);
            }
            Ok(TransformAction::Continue)
        }

        // Declarations — prepend <private/> if no visibility_modifier child
        "function_item" | "struct_item" | "enum_item" | "trait_item"
        | "const_item" | "static_item" | "type_item" | "mod_item" => {
            let has_vis = xot.children(node).any(|child| {
                get_element_name(xot, child).as_deref() == Some("visibility_modifier")
            });
            if !has_vis {
                prepend_empty_element(xot, node, "private")?;
            }
            if let Some(new_name) = map_element_name(&kind) {
                rename(xot, node, new_name);
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

        // Raw string literal — rename to <string> and prepend <raw/> marker
        "raw_string_literal" => {
            prepend_empty_element(xot, node, "raw")?;
            rename(xot, node, "string");
            Ok(TransformAction::Continue)
        }

        // let declarations - extract mut modifier
        "let_declaration" => {
            extract_modifiers(xot, node)?;
            rename(xot, node, "let");
            Ok(TransformAction::Continue)
        }

        // Identifiers are always names (definitions or references).
        // Tree-sitter uses distinct node kinds for type positions
        // (type_identifier, primitive_type, etc.), so bare identifiers
        // never need a heuristic — they are never types.
        "identifier" | "field_identifier" | "shorthand_field_identifier" => {
            rename(xot, node, "name");
            Ok(TransformAction::Continue)
        }
        "type_identifier" | "primitive_type" => {
            rename(xot, node, "type");
            wrap_text_in_name(xot, node)?;
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

/// True when the node has a `kind` attribute (i.e., it came from tree-sitter,
/// not a builder-inserted wrapper). Used to distinguish the tree-sitter kind
/// `parameters` (which we want to flatten) from any semantic `parameters`
/// element we might create elsewhere.
fn has_kind(xot: &Xot, node: XotNode) -> bool {
    get_kind(xot, node).is_some()
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
        "type_item" => Some("alias"),
        // parameters is flattened via Principle #12 above
        "parameter" => Some("param"),
        "self_parameter" => Some("self"),
        "reference_type" => Some("ref"),
        "generic_type" => Some("generic"),
        "scoped_type_identifier" | "scoped_identifier" => Some("path"),
        "return_expression" => Some("return"),
        "if_expression" => Some("if"),
        "else_clause" => Some("else"),
        "for_expression" => Some("for"),
        "while_expression" => Some("while"),
        "loop_expression" => Some("loop"),
        "match_expression" => Some("match"),
        "match_arm" => Some("arm"),
        "field_declaration" => Some("field"),
        "field_initializer" => Some("field"),
        "trait_bounds" => Some("bounds"),
        "call_expression" => Some("call"),
        "method_call_expression" => Some("call"),
        "field_expression" => Some("field"),
        "index_expression" => Some("index"),
        "binary_expression" => Some("binary"),
        "unary_expression" => Some("unary"),
        "closure_expression" => Some("closure"),
        "await_expression" => Some("await"),
        "try_expression" => Some("try"),
        "macro_invocation" => Some("macro"),
        "string_literal" => Some("string"),
        // raw_string_literal is handled in the match above (rename + prepend <raw/>)
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
        prepend_op_element(xot, node, op)?;
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

/// If `node` contains a single identifier-kind child, replace the node's
/// children with that identifier's text. Used to flatten builder-created
/// wrappers like `<name><identifier>foo</identifier></name>` to `<name>foo</name>`.
fn inline_single_identifier(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    let children: Vec<_> = xot.children(node).collect();
    for child in children {
        let child_name = match get_element_name(xot, child) {
            Some(n) => n,
            None => continue,
        };
        if !matches!(child_name.as_str(),
            "identifier" | "type_identifier" | "field_identifier" | "shorthand_field_identifier")
        {
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
        "bool" => SyntaxCategory::Keyword,

        // Keywords - declarations
        "function" | "impl" => SyntaxCategory::Keyword,
        "struct" | "enum" | "trait" => SyntaxCategory::Keyword,
        "mod" | "use" => SyntaxCategory::Keyword,
        "const" | "static" | "alias" => SyntaxCategory::Keyword,
        "let" | "param" | "params" | "self" => SyntaxCategory::Keyword,

        // Keywords - control flow
        "if" | "else" => SyntaxCategory::Keyword,
        "for" | "while" | "loop" => SyntaxCategory::Keyword,
        "match" | "arm" => SyntaxCategory::Keyword,
        "return" | "break" | "continue" => SyntaxCategory::Keyword,

        // Keywords - modifiers
        "pub" | "private" | "mut" | "async" | "await" | "unsafe" => SyntaxCategory::Keyword,

        // Types
        "ref" | "generic" | "path" => SyntaxCategory::Type,

        // Functions/calls
        "call" => SyntaxCategory::Function,
        "closure" => SyntaxCategory::Function,
        "macro" => SyntaxCategory::Function,

        // Operators
        "op" => SyntaxCategory::Operator,
        _ if is_operator_marker(element) => SyntaxCategory::Operator,
        "binary" | "unary" | "try" => SyntaxCategory::Operator,

        // Comments
        "comment" => SyntaxCategory::Comment,

        // Structural elements - no color
        _ => SyntaxCategory::Default,
    }
}
