//! Rust transform logic

use xot::{Xot, Node as XotNode};
use crate::xot_transform::{TransformAction, helpers::*};
use crate::output::syntax_highlight::SyntaxCategory;

use semantic::*;

/// Semantic element names — tractor's Rust XML vocabulary after transform.
/// Tree-sitter kind strings (left side of `match` arms) stay as bare
/// literals — they are external vocabulary.
pub mod semantic {
    // Structural — containers

    // Top-level / declarations
    pub const FILE: &str = "file";
    pub const FUNCTION: &str = "function";
    pub const IMPL: &str = "impl";
    pub const STRUCT: &str = "struct";
    pub const ENUM: &str = "enum";
    pub const TRAIT: &str = "trait";
    pub const MOD: &str = "mod";
    pub const USE: &str = "use";
    pub const CONST: &str = "const";
    pub const STATIC: &str = "static";
    pub const ALIAS: &str = "alias";
    pub const SIGNATURE: &str = "signature";
    pub const MODIFIERS: &str = "modifiers";

    // Members
    pub const PARAMETER: &str = "parameter";
    pub const SELF: &str = "self";
    pub const FIELD: &str = "field";
    pub const VARIANT: &str = "variant";
    pub const LIFETIME: &str = "lifetime";
    pub const ATTRIBUTE: &str = "attribute";

    // Types / generics
    pub const TYPE: &str = "type";
    pub const GENERIC: &str = "generic";
    pub const GENERICS: &str = "generics";
    pub const PATH: &str = "path";
    pub const BOUNDS: &str = "bounds";
    pub const BOUND: &str = "bound";
    pub const WHERE: &str = "where";

    // Statements / control flow
    pub const LET: &str = "let";
    pub const RETURN: &str = "return";
    pub const IF: &str = "if";
    pub const ELSE: &str = "else";
    pub const FOR: &str = "for";
    pub const WHILE: &str = "while";
    pub const LOOP: &str = "loop";
    pub const MATCH: &str = "match";
    pub const ARM: &str = "arm";
    pub const PATTERN: &str = "pattern";
    pub const BREAK: &str = "break";
    pub const CONTINUE: &str = "continue";
    pub const RANGE: &str = "range";
    pub const SEND: &str = "send";

    // Expressions
    pub const CALL: &str = "call";
    pub const INDEX: &str = "index";
    pub const BINARY: &str = "binary";
    pub const UNARY: &str = "unary";
    pub const ASSIGN: &str = "assign";
    pub const CLOSURE: &str = "closure";
    pub const AWAIT: &str = "await";
    pub const TRY: &str = "try";
    pub const MACRO: &str = "macro";
    pub const CAST: &str = "cast";
    pub const REF: &str = "ref";
    pub const TUPLE: &str = "tuple";
    pub const UNSAFE: &str = "unsafe";
    pub const LITERAL: &str = "literal";
    pub const BLOCK: &str = "block";

    // Visibility
    pub const PUB: &str = "pub";
    pub const IN: &str = "in";

    // Literals / atoms
    pub const STRING: &str = "string";
    pub const INT: &str = "int";
    pub const FLOAT: &str = "float";
    pub const BOOL: &str = "bool";
    pub const CHAR: &str = "char";

    // Identifiers
    pub const NAME: &str = "name";

    // Comments
    pub const COMMENT: &str = "comment";

    // Operator child
    pub const OP: &str = "op";

    // Markers — always-empty when emitted. These names MUST NOT also
    // be emitted as structural containers elsewhere in this file.
    pub const RAW: &str = "raw";
    pub const INNER: &str = "inner";
    pub const BORROWED: &str = "borrowed";
    pub const PRIVATE: &str = "private";
    pub const CRATE: &str = "crate";
    pub const SUPER: &str = "super";
    pub const MUT: &str = "mut";
    pub const ASYNC: &str = "async";
    pub const POINTER: &str = "pointer";
    pub const NEVER: &str = "never";
    pub const UNIT: &str = "unit";
    pub const DYNAMIC: &str = "dynamic";
    pub const ABSTRACT: &str = "abstract";
    pub const ASSOCIATED: &str = "associated";
    pub const BOUNDED: &str = "bounded";
    pub const ARRAY: &str = "array";
    pub const OR: &str = "or";
    pub const METHOD: &str = "method";
    pub const BASE: &str = "base";

    // These names double as marker AND structural container. Kept as
    // constants so the transform code is still type-safe, but NOT in
    // MARKER_ONLY — the invariant can't distinguish the two contexts.
    //   - FUNCTION: function_item (container) vs function_type (marker)
    //   - TUPLE: tuple_expression (container) vs tuple_type (marker)
    //   - TRAIT: trait_item (container) vs trait_type (marker)
    //   - SLICE: slice_type (marker only in emitted code, but kept
    //     as a distinct constant for syntax category alignment)
    //   - REF: reference_expression (container) vs ref_pattern (marker)
    //   - FIELD: field_expression / field_declaration (container) vs
    //     field_pattern / base_field_initializer (markers)
    //   - STRUCT: struct_item (container) vs struct_pattern (marker)
    //   - GENERIC: generic_type (container) vs generic_function (marker)
    //   - CONST: const_item (container) vs const_block (marker)
    //   - TRY: try_expression (container) vs try_block (marker)
    pub const SLICE: &str = "slice";

    /// Names that, when emitted, are always empty. Excludes ambiguous
    /// names that also appear as structural containers.
    pub const MARKER_ONLY: &[&str] = &[
        RAW,
        INNER,
        BORROWED,
        PRIVATE,
        CRATE,
        SUPER,
        MUT,
        ASYNC,
        POINTER,
        NEVER,
        UNIT,
        DYNAMIC,
        ABSTRACT,
        ASSOCIATED,
        BOUNDED,
        ARRAY,
        OR,
        METHOD,
        BASE,
    ];

    /// Every semantic name this language's transform can emit.
    pub const ALL_NAMES: &[&str] = &[
        FILE,
        FUNCTION, IMPL, STRUCT, ENUM, TRAIT, MOD, USE, CONST, STATIC, ALIAS,
        SIGNATURE, MODIFIERS,
        PARAMETER, SELF, FIELD, VARIANT, LIFETIME, ATTRIBUTE,
        TYPE, GENERIC, GENERICS, PATH, BOUNDS, BOUND, WHERE,
        LET, RETURN, IF, ELSE, FOR, WHILE, LOOP, MATCH, ARM, PATTERN,
        BREAK, CONTINUE, RANGE, SEND,
        CALL, INDEX, BINARY, UNARY, ASSIGN, CLOSURE, AWAIT, TRY, MACRO, CAST,
        REF, TUPLE, UNSAFE, LITERAL, BLOCK,
        PUB, IN,
        STRING, INT, FLOAT, BOOL, CHAR,
        NAME, COMMENT, OP,
        RAW, INNER, BORROWED, PRIVATE, CRATE, SUPER, MUT, ASYNC,
        POINTER, NEVER, UNIT, DYNAMIC, ABSTRACT, ASSOCIATED, BOUNDED, ARRAY,
        OR, METHOD, BASE, SLICE,
    ];
}

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

        // Declarations — prepend <private/> if no visibility_modifier child
        "function_item" | "struct_item" | "enum_item" | "trait_item"
        | "const_item" | "static_item" | "type_item" | "mod_item" => {
            let has_vis = xot.children(node).any(|child| {
                get_element_name(xot, child).as_deref() == Some("visibility_modifier")
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
        // Tree-sitter Rust emits `line_comment` and `block_comment`;
        // normalise to the shared `<comment>` vocabulary.
        "line_comment" | "block_comment" => {
            rename(xot, node, COMMENT);
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
            rename(xot, node, COMMENT);
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
        "source_file" => Some((FILE, None)),
        "function_item" => Some((FUNCTION, None)),
        "impl_item" => Some((IMPL, None)),
        "struct_item" => Some((STRUCT, None)),
        "enum_item" => Some((ENUM, None)),
        "trait_item" => Some((TRAIT, None)),
        "mod_item" => Some((MOD, None)),
        "use_declaration" => Some((USE, None)),
        "const_item" => Some((CONST, None)),
        "static_item" => Some((STATIC, None)),
        "type_item" => Some((ALIAS, None)),
        // parameters is flattened via Principle #12 above
        "parameter" => Some((PARAMETER, None)),
        "self_parameter" => Some((SELF, None)),
        // reference_type is handled above: <type> with <borrowed/> marker
        "generic_type" => Some((GENERIC, None)),
        "scoped_type_identifier" | "scoped_identifier" => Some((PATH, None)),
        "return_expression" => Some((RETURN, None)),
        "if_expression" => Some((IF, None)),
        "else_clause" => Some((ELSE, None)),
        "for_expression" => Some((FOR, None)),
        "while_expression" => Some((WHILE, None)),
        "loop_expression" => Some((LOOP, None)),
        "match_expression" => Some((MATCH, None)),
        "enum_variant" => Some((VARIANT, None)),
        "lifetime_parameter" | "lifetime" => Some((LIFETIME, None)),
        "function_signature_item" => Some((SIGNATURE, None)),
        "type_cast_expression" => Some((CAST, None)),
        "function_modifiers" => Some((MODIFIERS, None)),
        "break_expression" | "break_statement" => Some((BREAK, None)),
        "continue_expression" | "continue_statement" => Some((CONTINUE, None)),
        "range_expression" => Some((RANGE, None)),
        "send_statement" => Some((SEND, None)),
        "shorthand_field_initializer" => Some((FIELD, None)),
        "where_clause" => Some((WHERE, None)),
        "where_predicate" => Some((BOUND, None)),
        "reference_expression" => Some((REF, None)),
        "range_pattern" => Some((RANGE, None)),
        // Types — shape markers distinguish each flavor.
        "pointer_type" => Some((TYPE, Some(POINTER))),
        "function_type" => Some((TYPE, Some(FUNCTION))),
        "tuple_type" => Some((TYPE, Some(TUPLE))),
        "never_type" => Some((TYPE, Some(NEVER))),
        "unit_type" => Some((TYPE, Some(UNIT))),
        "dynamic_type" => Some((TYPE, Some(DYNAMIC))),
        "trait_type" => Some((TYPE, Some(TRAIT))),
        "abstract_type" => Some((TYPE, Some(ABSTRACT))),
        "associated_type" => Some((TYPE, Some(ASSOCIATED))),
        "bounded_type" => Some((TYPE, Some(BOUNDED))),
        "array_type" => Some((TYPE, Some(ARRAY))),
        "slice_type" => Some((TYPE, Some(SLICE))),
        // Patterns — shape markers distinguish or/field/struct/tuple.
        "or_pattern" => Some((PATTERN, Some(OR))),
        "field_pattern" => Some((PATTERN, Some(FIELD))),
        "struct_pattern" => Some((PATTERN, Some(STRUCT))),
        // attribute_item / inner_attribute_item are handled above
        // (flattened; inner form gets an <inner/> marker).
        "compound_assignment_expr" => Some((ASSIGN, None)),
        "tuple_expression" => Some((TUPLE, None)),
        "unsafe_block" => Some((UNSAFE, None)),
        "match_arm" => Some((ARM, None)),
        "field_declaration" => Some((FIELD, None)),
        "field_initializer" => Some((FIELD, None)),
        "trait_bounds" => Some((BOUNDS, None)),
        // Tree-sitter-rust emits `call_expression` for every call; method
        // calls like `obj.m()` appear with a `field_expression` as the
        // function child, so `//call/field` finds them without needing
        // a marker. `method_call_expression` is kept for forward-compat.
        "call_expression" => Some((CALL, None)),
        "method_call_expression" => Some((CALL, Some(METHOD))),
        "field_expression" => Some((FIELD, None)),
        "index_expression" => Some((INDEX, None)),
        "binary_expression" => Some((BINARY, None)),
        "unary_expression" => Some((UNARY, None)),
        "closure_expression" => Some((CLOSURE, None)),
        "await_expression" => Some((AWAIT, None)),
        "try_expression" => Some((TRY, None)),
        "macro_invocation" => Some((MACRO, None)),
        "string_literal" => Some((STRING, None)),
        // raw_string_literal is handled in the match above (rename + prepend <raw/>)
        "integer_literal" => Some((INT, None)),
        "float_literal" => Some((FLOAT, None)),
        "boolean_literal" => Some((BOOL, None)),
        // Char literal — collapse to <char>.
        "char_literal" => Some((CHAR, None)),
        // `async { … }` — collapse to <block> with <async/> marker so
        // `//block[async]` finds all async blocks.
        "async_block" => Some((BLOCK, Some(ASYNC))),
        // `try { … }` — same pattern.
        "try_block" => Some((BLOCK, Some(TRY))),
        // `const { … }` — const context block (Rust edition).
        "const_block" => Some((BLOCK, Some(CONST))),
        // `..base` in struct literals — shape marker lets queries find it.
        "base_field_initializer" => Some((FIELD, Some(BASE))),
        // `foo::<T>` — turbofish-style generic call. Collapse to <call>
        // with a <generic/> marker so it joins the existing call shape.
        "generic_function" => Some((CALL, Some(GENERIC))),
        // `ref x` / `ref mut x` pattern — shape marker on <pattern>.
        "ref_pattern" => Some((PATTERN, Some(REF))),
        "mut_pattern" => Some((PATTERN, Some(MUT))),
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

#[cfg(test)]
mod tests {
    use super::semantic::*;

    #[test]
    fn marker_only_names_are_in_all_names() {
        for m in MARKER_ONLY {
            assert!(
                ALL_NAMES.contains(m),
                "MARKER_ONLY entry {:?} missing from ALL_NAMES",
                m,
            );
        }
    }

    #[test]
    fn all_names_has_no_duplicates() {
        let mut seen = std::collections::HashSet::new();
        for name in ALL_NAMES {
            assert!(seen.insert(*name), "duplicate name in ALL_NAMES: {:?}", name);
        }
    }
}
