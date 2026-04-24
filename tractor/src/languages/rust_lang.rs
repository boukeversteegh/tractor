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
        "parenthesized_expression" => Ok(TransformAction::Flatten),
        "block" | "declaration_list" => Ok(TransformAction::Flatten),

        // Pure grouping wrappers around a list of homogeneous children
        // — drop the wrapper, the children become direct siblings of the
        // enclosing struct/expression (Principle #12).
        "field_declaration_list" | "field_initializer_list"
        | "match_block" => {
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
        // Type parameter list: flatten with field="generics".
        "type_parameters" => {
            distribute_field_to_children(xot, node, "generics");
            rename(xot, node, "generics");
            Ok(TransformAction::Flatten)
        }
        "type_parameter" => {
            // Inline the parameter's name as a `<name>TEXT</name>` child
            // so siblings like trait_bounds remain intact.
            replace_identifier_with_name_child(xot, node, &["type_identifier"])?;
            rename(xot, node, "generic");
            Ok(TransformAction::Continue)
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
        //
        // If the single child is a `lifetime` (tree-sitter kind), inline the
        // lifetime's descendant text so `<name><lifetime>'a</lifetime></name>`
        // becomes `<name>'a</name>` — preserves the text-leaf invariant and
        // avoids the `<lifetime><name><lifetime>…` triple-wrap that happens
        // when lifetime_parameter also renames to `<lifetime>`.
        "name" => {
            let element_children: Vec<_> = xot
                .children(node)
                .filter(|&c| xot.element(c).is_some())
                .collect();
            if element_children.len() == 1 {
                let child = element_children[0];
                if get_kind(xot, child).as_deref() == Some("lifetime") {
                    let text = descendant_text(xot, child);
                    let trimmed = text.trim().to_string();
                    if !trimmed.is_empty() {
                        let all_children: Vec<_> = xot.children(node).collect();
                        for c in all_children {
                            xot.detach(c)?;
                        }
                        let text_node = xot.new_text(&trimmed);
                        xot.append(node, text_node)?;
                        return Ok(TransformAction::Done);
                    }
                }
            }
            inline_single_identifier(xot, node)?;
            Ok(TransformAction::Continue)
        }

        // Visibility modifier (pub, pub(crate), etc.)
        //
        // Tree-sitter gives us a mix of text tokens and nested
        // elements (e.g. `pub(<crate>crate</crate>)`); we collapse
        // the whole subtree into a single empty `<pub/>` marker
        // with the restriction as a child marker, and dangle the
        // source text in the parent so string-value is preserved
        // character-for-character.
        "visibility_modifier" => {
            let source = descendant_text(xot, node);
            let trimmed = source.trim().to_string();

            // Drop all existing children (text tokens + tree-sitter's
            // `<crate>` / `<super>` nested elements) so we can rebuild
            // cleanly without duplicating the restriction marker.
            let existing: Vec<_> = xot.children(node).collect();
            for child in existing {
                xot.detach(child)?;
            }

            rename(xot, node, "pub");

            if let (Some(lp), Some(rp)) = (trimmed.find('('), trimmed.find(')')) {
                let inner = trimmed[lp + 1..rp].trim();
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

            // Dangle the original source token (`pub` /
            // `pub(crate)` / `pub(in path)` / …) as a sibling so
            // the enclosing declaration's string-value stays
            // source-accurate.
            insert_text_after(xot, node, &trimmed)?;
            return Ok(TransformAction::Done);
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
            apply_rename(xot, node, &kind)?;
            Ok(TransformAction::Continue)
        }

        "binary_expression" | "unary_expression" => {
            extract_operator(xot, node)?;
            apply_rename(xot, node, &kind)?;
            Ok(TransformAction::Continue)
        }

        // Raw string literal — rename to <string> and prepend <raw/> marker
        "raw_string_literal" => {
            prepend_empty_element(xot, node, "raw")?;
            rename(xot, node, "string");
            Ok(TransformAction::Continue)
        }

        // Reference types (`&T`, `&mut T`, `&'a T`) render as a single
        // <type> with a <borrowed/> marker (Principle #14: every type
        // reference wraps in <type>; Principle #13: empty marker for the
        // "this is a borrow" annotation). The inner referenced type is a
        // nested <type> child, so //type[borrowed] finds every reference
        // and //type[borrowed][mut] finds every mutable borrow.
        "reference_type" => {
            // Hoist `mut` from mutable_specifier to an empty marker.
            // The source "mut" text stays where it was — we replace
            // the `<mutable_specifier>mut</mutable_specifier>` wrapper
            // with a text node containing its own text, so the
            // enclosing string-value still sees "& mut T".
            let children: Vec<_> = xot.children(node).collect();
            let mut has_mut = false;
            for child in &children {
                if get_kind(xot, *child).as_deref() == Some("mutable_specifier") {
                    has_mut = true;
                    let text = get_text_content(xot, *child).unwrap_or_default();
                    let text_node = xot.new_text(&text);
                    xot.insert_before(*child, text_node)?;
                    xot.detach(*child)?;
                }
            }
            if has_mut {
                prepend_empty_element(xot, node, "mut")?;
            }
            prepend_empty_element(xot, node, "borrowed")?;
            rename(xot, node, "type");
            Ok(TransformAction::Continue)
        }

        // Struct construction expression: `Point { x: 1, y: 2 }`.
        // Renders as <literal><name>Point</name><field>…</field></literal>
        // — semantically a struct literal value (Principle #5: the name of
        // the type being constructed is a <name>, not a <type>, because
        // this is a reference-by-name to the struct being instantiated).
        "struct_expression" => {
            replace_identifier_with_name_child(
                xot,
                node,
                &["type_identifier", "scoped_type_identifier"],
            )?;
            rename(xot, node, "literal");
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
        // Tree-sitter Rust emits `line_comment` and `block_comment`;
        // normalise to the shared `<comment>` vocabulary.
        "line_comment" | "block_comment" => {
            rename(xot, node, "comment");
            Ok(TransformAction::Continue)
        }

        // String internals — grammar wrappers with no semantic
        // beyond their text value (Principle #12).
        "string_content" | "escape_sequence" | "raw_string_literal_content" => {
            Ok(TransformAction::Flatten)
        }

        // `doc_comment` is tree-sitter rust's `///` / `//!` kind —
        // semantically still a comment.
        "doc_comment" => {
            rename(xot, node, "comment");
            Ok(TransformAction::Continue)
        }

        // Qualified types, enum variants, tuple_struct patterns — all
        // grammar wrappers with no semantic beyond their subtree.
        "qualified_type"
        | "tuple_struct_pattern"
        | "enum_variant_list"
        | "use_list"
        | "use_as_clause"
        | "scoped_use_list"
        | "ordered_field_declaration_list"
        | "closure_parameters"
        | "type_binding"
        | "mutable_specifier"
        | "let_condition"
        | "use_wildcard"
        | "spread_element"
        | "outer_doc_comment_marker"
        | "inner_doc_comment_marker" => Ok(TransformAction::Flatten),

        // Token trees are the opaque body of a macro invocation.
        // Flatten so the macro call reads as a continuous run of
        // tokens; a dedicated structural model of macro args is
        // deferred.
        "token_tree" => Ok(TransformAction::Flatten),

        // Pattern kinds in match arms — normalise to `<pattern>`
        // so `//match/arm/pattern` is the uniform shape. The
        // specific pattern form (identifier / literal / tuple /
        // struct / `_`) is exposed via child structure rather
        // than element name.
        "match_pattern" => {
            rename(xot, node, "pattern");
            Ok(TransformAction::Continue)
        }
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
            apply_rename(xot, node, &kind)?;
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

/// Map tree-sitter node kinds to semantic element names.
///
/// The second tuple element is an optional disambiguation marker:
/// when multiple tree-sitter kinds collapse to the same semantic
/// element (e.g. all `*_type` → `<type>`, `or_pattern`/`struct_pattern`
/// → `<pattern>`), the empty marker child preserves the original
/// shape so queries like `//type[function]` remain expressible.
fn map_element_name(kind: &str) -> Option<(&'static str, Option<&'static str>)> {
    match kind {
        "source_file" => Some(("file", None)),
        "function_item" => Some(("function", None)),
        "impl_item" => Some(("impl", None)),
        "struct_item" => Some(("struct", None)),
        "enum_item" => Some(("enum", None)),
        "trait_item" => Some(("trait", None)),
        "mod_item" => Some(("mod", None)),
        "use_declaration" => Some(("use", None)),
        "const_item" => Some(("const", None)),
        "static_item" => Some(("static", None)),
        "type_item" => Some(("alias", None)),
        // parameters is flattened via Principle #12 above
        "parameter" => Some(("parameter", None)),
        "self_parameter" => Some(("self", None)),
        // reference_type is handled above: <type> with <borrowed/> marker
        "generic_type" => Some(("generic", None)),
        "scoped_type_identifier" | "scoped_identifier" => Some(("path", None)),
        "return_expression" => Some(("return", None)),
        "if_expression" => Some(("if", None)),
        "else_clause" => Some(("else", None)),
        "for_expression" => Some(("for", None)),
        "while_expression" => Some(("while", None)),
        "loop_expression" => Some(("loop", None)),
        "match_expression" => Some(("match", None)),
        "enum_variant" => Some(("variant", None)),
        "lifetime_parameter" | "lifetime" => Some(("lifetime", None)),
        "function_signature_item" => Some(("signature", None)),
        "type_cast_expression" => Some(("cast", None)),
        "function_modifiers" => Some(("modifiers", None)),
        "break_expression" | "break_statement" => Some(("break", None)),
        "continue_expression" | "continue_statement" => Some(("continue", None)),
        "range_expression" => Some(("range", None)),
        "send_statement" => Some(("send", None)),
        "shorthand_field_initializer" => Some(("field", None)),
        "where_clause" => Some(("where", None)),
        "where_predicate" => Some(("bound", None)),
        "reference_expression" => Some(("ref", None)),
        "range_pattern" => Some(("range", None)),
        // Types — shape markers distinguish each flavor.
        "pointer_type" => Some(("type", Some("pointer"))),
        "function_type" => Some(("type", Some("function"))),
        "tuple_type" => Some(("type", Some("tuple"))),
        "never_type" => Some(("type", Some("never"))),
        "unit_type" => Some(("type", Some("unit"))),
        "dynamic_type" => Some(("type", Some("dynamic"))),
        "trait_type" => Some(("type", Some("trait"))),
        "abstract_type" => Some(("type", Some("abstract"))),
        "associated_type" => Some(("type", Some("associated"))),
        "bounded_type" => Some(("type", Some("bounded"))),
        "array_type" => Some(("type", Some("array"))),
        "slice_type" => Some(("type", Some("slice"))),
        // Patterns — shape markers distinguish or/field/struct/tuple.
        "or_pattern" => Some(("pattern", Some("or"))),
        "field_pattern" => Some(("pattern", Some("field"))),
        "struct_pattern" => Some(("pattern", Some("struct"))),
        "attribute_item" | "inner_attribute_item" => Some(("attribute", None)),
        "compound_assignment_expr" => Some(("assign", None)),
        "tuple_expression" => Some(("tuple", None)),
        "unsafe_block" => Some(("unsafe", None)),
        "match_arm" => Some(("arm", None)),
        "field_declaration" => Some(("field", None)),
        "field_initializer" => Some(("field", None)),
        "trait_bounds" => Some(("bounds", None)),
        // Tree-sitter-rust emits `call_expression` for every call; method
        // calls like `obj.m()` appear with a `field_expression` as the
        // function child, so `//call/field` finds them without needing
        // a marker. `method_call_expression` is kept for forward-compat.
        "call_expression" => Some(("call", None)),
        "method_call_expression" => Some(("call", Some("method"))),
        "field_expression" => Some(("field", None)),
        "index_expression" => Some(("index", None)),
        "binary_expression" => Some(("binary", None)),
        "unary_expression" => Some(("unary", None)),
        "closure_expression" => Some(("closure", None)),
        "await_expression" => Some(("await", None)),
        "try_expression" => Some(("try", None)),
        "macro_invocation" => Some(("macro", None)),
        "string_literal" => Some(("string", None)),
        // raw_string_literal is handled in the match above (rename + prepend <raw/>)
        "integer_literal" => Some(("int", None)),
        "float_literal" => Some(("float", None)),
        "boolean_literal" => Some(("bool", None)),
        _ => None,
    }
}

/// Apply `map_element_name` to a node: rename + prepend marker (if any).
fn apply_rename(xot: &mut Xot, node: XotNode, kind: &str) -> Result<(), xot::Error> {
    if let Some((new_name, marker)) = map_element_name(kind) {
        rename(xot, node, new_name);
        if let Some(m) = marker {
            prepend_empty_element(xot, node, m)?;
        }
    }
    Ok(())
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
        "let" | "parameter" | "parameters" | "self" => SyntaxCategory::Keyword,

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
