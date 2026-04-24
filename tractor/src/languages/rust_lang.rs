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
        RAW, BORROWED, PRIVATE, CRATE, SUPER, MUT, ASYNC,
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
        // Char literal — collapse to <char>.
        "char_literal" => Some(("char", None)),
        // `async { … }` — collapse to <block> with <async/> marker so
        // `//block[async]` finds all async blocks.
        "async_block" => Some(("block", Some("async"))),
        // `try { … }` — same pattern.
        "try_block" => Some(("block", Some("try"))),
        // `const { … }` — const context block (Rust edition).
        "const_block" => Some(("block", Some("const"))),
        // `..base` in struct literals — shape marker lets queries find it.
        "base_field_initializer" => Some(("field", Some("base"))),
        // `foo::<T>` — turbofish-style generic call. Collapse to <call>
        // with a <generic/> marker so it joins the existing call shape.
        "generic_function" => Some(("call", Some("generic"))),
        // `ref x` / `ref mut x` pattern — shape marker on <pattern>.
        "ref_pattern" => Some(("pattern", Some("ref"))),
        "mut_pattern" => Some(("pattern", Some("mut"))),
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
