//! Rust transform logic

use xot::{Xot, Node as XotNode};
use crate::transform::{TransformAction, helpers::*};
use crate::transform::operators::{prepend_op_element, is_operator_marker};
use crate::output::syntax_highlight::SyntaxCategory;

use super::semantic::*;


/// Transform a Rust AST node.
///
/// Dispatch is split in two:
///   1. If the node carries a `kind` attribute (set by the builder from
///      the original tree-sitter kind), match on that — it never changes
///      mid-walk, so an arm like `"identifier"` always wins.
///   2. Otherwise the node is a builder-inserted wrapper (e.g. the
///      `<name>` field wrapper) — match on the element name for the
///      few wrappers we need to handle.
pub fn transform(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let kind = match get_kind(xot, node) {
        Some(k) => k,
        None => {
            // Builder-inserted wrapper (no `kind` attribute).
            let name = get_element_name(xot, node).unwrap_or_default();
            return match name.as_str() {
                // Name wrappers created by the builder for field="name".
                // Inline the single identifier/type_identifier/field_identifier
                // child as text:
                //   <name><identifier>foo</identifier></name> -> <name>foo</name>
                //
                // If the single child is a `lifetime` (tree-sitter kind),
                // inline the lifetime's descendant text so
                // `<name><lifetime>'a</lifetime></name>` becomes
                // `<name>'a</name>` — preserves the text-leaf invariant
                // and avoids the `<lifetime><name><lifetime>…` triple-wrap
                // that happens when lifetime_parameter also renames to
                // `<lifetime>`.
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
                _ => Ok(TransformAction::Continue),
            };
        }
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

        // Flat lists (Principle #12). The kind branch already excludes
        // builder-inserted wrappers, so the legacy `if has_kind(...)`
        // guards are no longer needed.
        "parameters" => {
            distribute_field_to_children(xot, node, "parameters");
            Ok(TransformAction::Flatten)
        }
        "arguments" => {
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
            rename(xot, node, GENERICS);
            Ok(TransformAction::Flatten)
        }
        "type_parameter" => {
            // Inline the parameter's name as a `<name>TEXT</name>` child
            // so siblings like trait_bounds remain intact.
            replace_identifier_with_name_child(xot, node, &["type_identifier"])?;
            rename(xot, node, GENERIC);
            Ok(TransformAction::Continue)
        }

        // Generic type references: apply the C# pattern.
        //   generic_type(<type_identifier>Vec</type_identifier>, type_arguments)
        //     -> <type><generic/>Vec <type field="arguments">i32</type>...</type>
        "generic_type" => {
            rewrite_generic_type(xot, node, &["type_identifier", "scoped_type_identifier"])?;
            Ok(TransformAction::Continue)
        }

        // Attribute items — `#[derive(…)]` and `#![allow(…)]`.
        //
        // Tree-sitter's shape is:
        //     attribute_item(#[ attribute(name,…) ])
        // — an outer `_item` wrapper containing `#[` / `]` tokens and the
        // real `<attribute>` meta-item inside. Rendering both as
        // `<attribute>` produced nested `<attribute><attribute>…</attribute></attribute>`,
        // which diverges from Java/Python/C#/PHP's flat shape.
        //
        // Fix: flatten the outer `_item` wrapper so the inner `<attribute>`
        // becomes a direct child of the enclosing declaration. The bracket
        // tokens (`#[` / `]`) survive as text siblings.
        //
        // For `inner_attribute_item` (`#![…]`), prepend an `<inner/>`
        // marker on the inner attribute BEFORE flattening so queries can
        // distinguish inner (scope-level) from outer (item-level) attrs.
        "inner_attribute_item" => {
            let children: Vec<_> = xot.children(node).collect();
            for child in children {
                if get_kind(xot, child).as_deref() == Some("attribute") {
                    prepend_empty_element(xot, child, INNER)?;
                    break;
                }
            }
            Ok(TransformAction::Flatten)
        }
        "attribute_item" => Ok(TransformAction::Flatten),

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

            rename(xot, node, PUB);

            if let (Some(lp), Some(rp)) = (trimmed.find('('), trimmed.find(')')) {
                let inner = trimmed[lp + 1..rp].trim();
                match inner {
                    "crate" => { prepend_empty_element(xot, node, CRATE)?; }
                    "super" => { prepend_empty_element(xot, node, SUPER)?; }
                    _ if inner.starts_with("in ") => {
                        let path = inner[3..].trim();
                        prepend_element_with_text(xot, node, IN, path)?;
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

        // Declarations — prepend <private/> if no visibility_modifier child.
        // Dispatch on the stable tree-sitter `kind` attribute so we
        // detect the modifier even if a previous pass has already
        // renamed the child's element.
        "function_item" | "struct_item" | "enum_item" | "trait_item"
        | "const_item" | "static_item" | "type_item" | "mod_item" => {
            let has_vis = xot.children(node).any(|child| {
                get_kind(xot, child).as_deref() == Some("visibility_modifier")
            });
            if !has_vis {
                prepend_empty_element(xot, node, PRIVATE)?;
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
            prepend_empty_element(xot, node, RAW)?;
            rename(xot, node, STRING);
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
                prepend_empty_element(xot, node, MUT)?;
            }
            prepend_empty_element(xot, node, BORROWED)?;
            rename(xot, node, TYPE);
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
            rename(xot, node, LITERAL);
            Ok(TransformAction::Continue)
        }

        // let declarations - extract mut modifier
        "let_declaration" => {
            extract_modifiers(xot, node)?;
            rename(xot, node, LET);
            Ok(TransformAction::Continue)
        }

        // Identifiers are always names (definitions or references).
        // Tree-sitter uses distinct node kinds for type positions
        // (type_identifier, primitive_type, etc.), so bare identifiers
        // never need a heuristic — they are never types.
        // Tree-sitter Rust emits `line_comment`, `block_comment`, and
        // `doc_comment` (the `///` / `//!` kinds). All collapse to the
        // shared `<comment>` vocabulary, then go through the shared
        // classifier (trailing / leading / floating + line-comment
        // grouping). Doc comments group naturally because they share
        // the `//` prefix family.
        "line_comment" | "block_comment" | "doc_comment" => {
            rename(xot, node, COMMENT);
            static CLASSIFIER: crate::languages::comments::CommentClassifier =
                crate::languages::comments::CommentClassifier { line_prefixes: &["//"] };
            CLASSIFIER.classify_and_group(xot, node, TRAILING, LEADING)
        }

        // String internals — grammar wrappers with no semantic
        // beyond their text value (Principle #12).
        "string_content" | "escape_sequence" | "raw_string_literal_content" => {
            Ok(TransformAction::Flatten)
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
            rename(xot, node, PATTERN);
            Ok(TransformAction::Continue)
        }
        "identifier" | "field_identifier" | "shorthand_field_identifier" => {
            rename(xot, node, NAME);
            Ok(TransformAction::Continue)
        }
        "type_identifier" | "primitive_type" => {
            rename(xot, node, TYPE);
            wrap_text_in_name(xot, node)?;
            Ok(TransformAction::Continue)
        }

        _ => {
            apply_rename(xot, node, &kind)?;
            Ok(TransformAction::Continue)
        }
    }
}

/// Map tree-sitter node kinds to semantic element names.
///
/// Derived from `semantic::KINDS` — the catalogue is the single source
/// of truth, this is just the rename projection.
fn map_element_name(kind: &str) -> Option<(&'static str, Option<&'static str>)> {
    super::semantic::rename_target(kind)
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
    // Each entry pairs a source-text keyword with the semantic marker
    // name to emit (so typos can't drift between the two).
    const MODIFIERS: &[(&str, &str)] = &[
        ("mut", MUT),
        ("async", ASYNC),
        ("unsafe", UNSAFE),
        ("const", CONST),
    ];

    let found: Vec<&str> = texts.iter()
        .filter_map(|t| MODIFIERS.iter().find(|(src, _)| *src == t).map(|(_, marker)| *marker))
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

/// Map a transformed element name to a syntax category for highlighting.
///
/// Consults the per-name NODES table first (one source of truth);
/// falls back to cross-cutting rules for names not in NODES.
pub fn syntax_category(element: &str) -> SyntaxCategory {
    if let Some(spec) = super::semantic::spec(element) {
        return spec.syntax;
    }
    match element {
        // Raw tree-sitter kinds / builder wrappers not in NODES:
        "parameters" => SyntaxCategory::Keyword,
        _ if is_operator_marker(element) => SyntaxCategory::Operator,
        _ => SyntaxCategory::Default,
    }
}

#[cfg(test)]
mod tests {
    use crate::languages::rust_lang::semantic::NODES;

    #[test]
    fn no_duplicate_node_names() {
        let mut names: Vec<&str> = NODES.iter().map(|n| n.name).collect();
        names.sort();
        let total = names.len();
        names.dedup();
        assert_eq!(names.len(), total, "duplicate NODES entry");
    }

    #[test]
    fn no_unused_role() {
        for n in NODES {
            assert!(
                n.marker || n.container,
                "<{}> is neither marker nor container — dead entry?",
                n.name,
            );
        }
    }
}
