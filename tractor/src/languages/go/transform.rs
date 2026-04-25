//! Go transform logic

use xot::{Xot, Node as XotNode};
use crate::xot_transform::{TransformAction, helpers::*};
use crate::output::syntax_highlight::SyntaxCategory;

use super::semantic::*;


/// Transform a Go AST node.
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
                // Inline the single identifier/type_identifier child as text:
                //   <name><identifier>foo</identifier></name> -> <name>foo</name>
                "name" => {
                    inline_single_identifier(xot, node)?;
                    Ok(TransformAction::Continue)
                }
                _ => Ok(TransformAction::Continue),
            };
        }
    };

    match kind.as_str() {
        "expression_statement" => Ok(TransformAction::Skip),
        "block" => Ok(TransformAction::Flatten),
        // Struct/interface field lists and miscellaneous groupings —
        // drop the wrappers so each field/spec is a direct child of
        // the enclosing struct/interface/declaration (Principle #12).
        "field_declaration_list" | "expression_list" => {
            Ok(TransformAction::Flatten)
        }
        // Import declarations wrap one or more `import_spec` nodes in
        // parens. Collapse the spec wrapper so each import path is a
        // direct child of the <import>; rename the outer declaration
        // to `import` below via map_element_name.
        //
        // `const_spec` / `var_spec` are the same pattern — a grammar
        // wrapper around `name = value`. Flatten so the declaration
        // reads as `<const>const<name>x</name>=<value>1</value></const>`
        // rather than nesting the assignment inside an opaque spec
        // element.
        "import_spec" | "const_spec" | "var_spec" => Ok(TransformAction::Flatten),

        // Composite literal elements — `literal_element` /
        // `keyed_element` / `literal_value` are grammar wrappers
        // around individual list/map values. Flatten so they become
        // direct siblings inside the enclosing `<literal>`
        // (Principle #12).
        "literal_element" | "keyed_element" | "literal_value"
        | "var_spec_list"
        // `import (…)` wraps its `import_spec`s in an `import_spec_list`
        // — grouping wrapper, flatten so imports become siblings.
        | "import_spec_list"
        // `for init; cond; post { … }` puts init/cond/post inside a
        // `for_clause` wrapper — grouping only, flatten so the three
        // pieces become direct children of the `<for>`.
        | "for_clause" => Ok(TransformAction::Flatten),

        // More Go generic / constraint wrappers — all grammar-only.
        "type_parameter_list" | "type_parameter_declaration"
        | "type_elem" | "type_constraint" | "qualified_type"
        | "type_case" => Ok(TransformAction::Flatten),
        // The content-inside-quotes node on "interpreted" strings —
        // inline as raw text into the enclosing <string>.
        "interpreted_string_literal_content" | "raw_string_literal_content" | "escape_sequence" => Ok(TransformAction::Flatten),

        // Type declarations: move the leading `type` keyword into the
        // inner `type_spec` so it renders as part of the <type> element,
        // then flatten the outer wrapper.
        "type_declaration" => {
            move_type_keyword_into_spec(xot, node)?;
            Ok(TransformAction::Flatten)
        }

        // Flat lists (Principle #12)
        "parameter_list" => {
            // Go reuses `parameter_list` for both formal parameters AND
            // multi-value return specs. The builder has already wrapped
            // the returns case in a <returns> element (via the
            // `result`→`returns` field normalisation), so we can tell
            // which we're in by looking at the parent.
            let in_returns = get_parent(xot, node)
                .and_then(|p| get_element_name(xot, p))
                .as_deref() == Some("returns");
            if in_returns {
                // Each parameter_declaration here holds a return type.
                // Collapse `<param><type>X</type></param>` to just `<type>X</type>`
                // so the returns list reads as a sequence of types, not params.
                collapse_return_param_list(xot, node)?;
                Ok(TransformAction::Flatten)
            } else {
                distribute_field_to_children(xot, node, "parameters");
                Ok(TransformAction::Flatten)
            }
        }
        "argument_list" => {
            distribute_field_to_children(xot, node, "arguments");
            Ok(TransformAction::Flatten)
        }

        // Raw string literal — rename to <string> and prepend <raw/> marker
        "raw_string_literal" => {
            prepend_empty_element(xot, node, RAW)?;
            rename(xot, node, STRING);
            Ok(TransformAction::Continue)
        }

        // Short variable declarations (`x := 42`) — render as <variable>
        // with a <short/> marker to distinguish from `var x = 42`.
        "short_var_declaration" => {
            prepend_empty_element(xot, node, SHORT)?;
            rename(xot, node, VARIABLE);
            Ok(TransformAction::Continue)
        }

        // Declarations — add <exported/> or <unexported/> based on name capitalization
        "function_declaration" | "method_declaration" => {
            let marker = get_export_marker(xot, node);
            prepend_empty_element(xot, node, marker)?;
            apply_rename(xot, node, &kind)?;
            Ok(TransformAction::Continue)
        }

        // Struct field — Go export rule is name-capitalization, same as
        // functions/methods/types. Emit <exported/>/<unexported/> markers.
        "field_declaration" => {
            let marker = get_export_marker(xot, node);
            prepend_empty_element(xot, node, marker)?;
            apply_rename(xot, node, &kind)?;
            Ok(TransformAction::Continue)
        }

        // Type declarations split three ways:
        //
        //   type Hello struct { … }    -> <struct><name>Hello</name>…</struct>
        //   type Greeter interface {…} -> <interface><name>Greeter</name>…</interface>
        //   type MyInt int             -> <type><name>MyInt</name><type>int</type></type>
        //
        // For struct/interface, hoist the inner shape up so a dev reads
        // "I'm declaring a struct named Hello" (Goal #5). The `<type>`
        // wrapper in the tree-sitter grammar is Go-spec terminology, not
        // developer mental model. For defined types over a plain type
        // reference, keep `<type>` — Go's own spec term — with the
        // underlying type as a nested `<type>` child.
        "type_spec" => {
            let marker = get_export_marker(xot, node);
            prepend_empty_element(xot, node, marker)?;

            let inner = xot.children(node)
                .filter(|&c| xot.element(c).is_some())
                .find(|&c| matches!(
                    get_kind(xot, c).as_deref(),
                    Some("struct_type") | Some("interface_type"),
                ));

            if let Some(inner) = inner {
                let inner_kind = get_kind(xot, inner).unwrap();
                let new_name = if inner_kind == "struct_type" {
                    STRUCT
                } else {
                    INTERFACE
                };
                rename(xot, node, new_name);
                // Hoist inner's children before the inner wrapper itself,
                // then drop the wrapper so the outer element owns them.
                let inner_children: Vec<_> = xot.children(inner).collect();
                for c in inner_children {
                    xot.detach(c)?;
                    xot.insert_before(inner, c)?;
                }
                xot.detach(inner)?;
            } else {
                rename(xot, node, TYPE);
            }
            Ok(TransformAction::Continue)
        }

        // Type alias declarations — `type Color = int`. Distinct from
        // `type MyInt int` (defined type), which creates a new distinct
        // type. Rename to <alias> — parallel with Rust / TS / C# / Java.
        "type_alias" => {
            let marker = get_export_marker(xot, node);
            prepend_empty_element(xot, node, marker)?;
            rename(xot, node, ALIAS);
            Ok(TransformAction::Continue)
        }

        // Go's tree-sitter doesn't emit an `else_clause` wrapper: the
        // `alternative` field of an if_statement points directly at
        // the nested if_statement (for `else if`) or a block (for
        // final `else {…}`). Wrap the alternative in `<else>`
        // surgically so the shared conditional-shape post-transform
        // can collapse the chain uniformly (same fix as Java / C#).
        "if_statement" => {
            wrap_field_child(xot, node, "alternative", ELSE)?;
            rename(xot, node, IF);
            Ok(TransformAction::Continue)
        }

        "binary_expression" | "unary_expression" => {
            extract_operator(xot, node)?;
            apply_rename(xot, node, &kind)?;
            Ok(TransformAction::Continue)
        }

        // Identifiers are always names (definitions or references).
        // Tree-sitter uses `type_identifier` for type positions, so bare
        // identifiers never need a heuristic — they are never types.
        "identifier" => {
            rename(xot, node, NAME);
            Ok(TransformAction::Continue)
        }
        "type_identifier" => {
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

/// Move the literal `type` keyword text from a `type_declaration` into
/// its inner `type_spec` / `type_alias` child so the keyword stays
/// attached when the outer wrapper is flattened. Without this,
/// `type Foo struct { … }` becomes a free-floating `"type"` text node
/// sitting next to the bare spec/alias element at the file level.
fn move_type_keyword_into_spec(xot: &mut Xot, decl: XotNode) -> Result<(), xot::Error> {
    let keyword = match xot.children(decl)
        .find(|&c| xot.text_str(c).map(|t| t.trim() == "type").unwrap_or(false))
    {
        Some(k) => k,
        None => return Ok(()),
    };
    let spec = xot.children(decl)
        .filter(|&c| xot.element(c).is_some())
        .find(|&c| matches!(
            get_kind(xot, c).as_deref(),
            Some("type_spec") | Some("type_alias"),
        ));
    let spec = match spec {
        Some(s) => s,
        None => return Ok(()),
    };
    xot.detach(keyword)?;
    xot.prepend(spec, keyword)?;
    Ok(())
}

/// Rewrite each `parameter_declaration` in a return-type list to just its
/// inner type node, dropping the `<param>` wrapper so a returns list reads
/// as a sequence of types:
///
/// `<returns><param><type>int</type></param><param><type>error</type></param></returns>`
///   → `<returns><type>int</type><type>error</type></returns>`
fn collapse_return_param_list(xot: &mut Xot, list: XotNode) -> Result<(), xot::Error> {
    let children: Vec<XotNode> = xot.children(list)
        .filter(|&c| xot.element(c).is_some())
        .collect();
    for child in children {
        if get_kind(xot, child).as_deref() != Some("parameter_declaration") {
            continue;
        }
        let type_child = xot.children(child).find(|&c| {
            get_element_name(xot, c).as_deref() == Some("type")
                || matches!(
                    get_kind(xot, c).as_deref(),
                    Some("type_identifier" | "pointer_type" | "slice_type" | "array_type"
                        | "map_type" | "channel_type" | "interface_type" | "struct_type"
                        | "generic_type")
                )
        });
        if let Some(type_node) = type_child {
            xot.detach(type_node)?;
            xot.insert_before(child, type_node)?;
            xot.detach(child)?;
        }
    }
    Ok(())
}

/// Determine exported/unexported based on name child's first character
fn get_export_marker(xot: &Xot, node: XotNode) -> &'static str {
    for child in xot.children(node) {
        if let Some(name) = get_element_name(xot, child) {
            if name == NAME {
                // Look for identifier text inside the name wrapper
                for grandchild in xot.children(child) {
                    if let Some(text) = get_text_content(xot, grandchild) {
                        if text.starts_with(|c: char| c.is_uppercase()) {
                            return EXPORTED;
                        }
                        return UNEXPORTED;
                    }
                }
                // Name wrapper might have text directly
                if let Some(text) = get_text_content(xot, child) {
                    if text.starts_with(|c: char| c.is_uppercase()) {
                        return EXPORTED;
                    }
                    return UNEXPORTED;
                }
            }
            // Also check identifier/type_identifier directly (before name wrapping)
            if name == "identifier" || name == "type_identifier" {
                if let Some(field) = get_attr(xot, child, "field") {
                    if field == "name" {
                        if let Some(text) = get_text_content(xot, child) {
                            if text.starts_with(|c: char| c.is_uppercase()) {
                                return EXPORTED;
                            }
                            return UNEXPORTED;
                        }
                    }
                }
            }
        }
    }
    UNEXPORTED // default
}

/// Map tree-sitter node kinds to semantic element names.
///
/// The second tuple element is an optional disambiguation marker
/// for kinds that otherwise collapse (e.g. `type_switch_statement`
/// and `switch_statement` both → `<switch>`, distinguished by the
/// `<type/>` marker child on the former).
fn map_element_name(kind: &str) -> Option<(&'static str, Option<&'static str>)> {
    match kind {
        "source_file" => Some((FILE, None)),
        "package_clause" => Some((PACKAGE, None)),
        "function_declaration" => Some((FUNCTION, None)),
        "method_declaration" => Some((METHOD, None)),
        // type_declaration is flattened in the match above.
        "type_spec" => Some((TYPE, None)),
        "struct_type" => Some((STRUCT, None)),
        "interface_type" => Some((INTERFACE, None)),
        "const_declaration" => Some((CONST, None)),
        "var_declaration" => Some((VAR, None)),
        "import_declaration" => Some((IMPORT, None)),
        // parameter_list is flattened via Principle #12 above
        "parameter_declaration" => Some((PARAMETER, None)),
        "method_elem" => Some((METHOD, None)),
        "field_declaration" => Some((FIELD, None)),
        "pointer_type" => Some((POINTER, None)),
        "slice_type" => Some((SLICE, None)),
        "map_type" => Some((MAP, None)),
        "channel_type" => Some((CHAN, None)),
        "return_statement" => Some((RETURN, None)),
        "if_statement" => Some((IF, None)),
        "else_clause" => Some((ELSE, None)),
        "for_statement" => Some((FOR, None)),
        "range_clause" => Some((RANGE, None)),
        // Tree-sitter-go emits `expression_switch_statement` for a
        // plain switch; `switch_statement` appears in older grammars.
        "switch_statement" => Some((SWITCH, None)),
        "expression_switch_statement" => Some((SWITCH, None)),
        "case_clause" => Some((CASE, None)),
        "default_case" => Some((DEFAULT, None)),
        "defer_statement" => Some((DEFER, None)),
        "go_statement" => Some((GO, None)),
        "select_statement" => Some((SELECT, None)),
        "call_expression" => Some((CALL, None)),
        "selector_expression" => Some((MEMBER, None)),
        "index_expression" => Some((INDEX, None)),
        "composite_literal" => Some((LITERAL, None)),
        "binary_expression" => Some((BINARY, None)),
        "unary_expression" => Some((UNARY, None)),
        "interpreted_string_literal" => Some((STRING, None)),
        // raw_string_literal is handled in the match above (rename + prepend <raw/>)
        "int_literal" => Some((INT, None)),
        "float_literal" => Some((FLOAT, None)),
        "assignment_statement" => Some((ASSIGN, None)),
        "inc_statement" => Some((UNARY, None)),
        "dec_statement" => Some((UNARY, None)),
        "labeled_statement" => Some((LABELED, None)),
        "label_name" => Some((LABEL, None)),
        "send_statement" => Some((SEND, None)),
        "communication_case" => Some((CASE, None)),
        "receive_statement" => Some((RECEIVE, None)),
        // Function types get a <function/> marker, negated types get
        // <negated/> (interface constraints: `~int`). Keeps the tree
        // reads as a single <type> with the shape annotated via marker.
        "function_type" => Some((TYPE, Some(FUNCTION))),
        "negated_type" => Some((TYPE, Some(NEGATED))),
        "func_literal" => Some((CLOSURE, None)),
        "continue_statement" => Some((CONTINUE, None)),
        "variadic_parameter_declaration" => Some((PARAMETER, None)),
        // `switch x.(type) { … }` — distinguished from a regular switch
        // by a <type/> marker so `//switch[type]` finds every type switch.
        "type_switch_statement" => Some((SWITCH, Some(TYPE))),
        "type_assertion_expression" => Some((ASSERT, None)),
        "type_arguments" => Some((ARGUMENTS, None)),
        "break_statement" => Some((BREAK, None)),
        "true" => Some((TRUE, None)),
        "false" => Some((FALSE, None)),
        "nil" => Some((NIL, None)),
        // `field_identifier` is a leaf — either the name of a struct field
        // or the method/field being accessed in a selector. Treat it as
        // `<name>` in both contexts (role inferred from tree position).
        "field_identifier" => Some((NAME, None)),
        "package_identifier" => Some((NAME, None)),
        // `_` — Go's discard identifier. Still a name slot semantically.
        "blank_identifier" => Some((NAME, None)),
        // `'a'` — Go rune literal; collapse to <char> (uniform with Rust).
        "rune_literal" => Some((CHAR, None)),
        // `goto LABEL` — rename.
        "goto_statement" => Some((GOTO, None)),
        // `generic_type` — `Foo[int]` generic type reference. Rename to
        // <type><generic/> so it joins the collapsed type vocabulary.
        "generic_type" => Some((TYPE, Some(GENERIC))),
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
        // Also accept:
        //   - `package_identifier` — Go's import alias name (`myio "io"`);
        //     raw tree-sitter kind, pre-rename.
        //   - `name` — walk order may have already renamed an inner
        //     identifier (package_identifier / field_identifier), so
        //     the outer field wrapper sees `<name><name>…</name></name>`
        //   - `dot` — Go's `import . "pkg"` uses a `.` token; tree-sitter
        //     tags it as `dot`. It's the "name" for import purposes.
        //   - `blank_identifier` — Go's `_` discard; still fills a name slot.
        if !matches!(
            child_name.as_str(),
            "identifier" | "type_identifier" | "field_identifier"
                | "package_identifier"
                | "name" | "dot" | "blank_identifier",
        ) {
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
    use crate::languages::go::semantic::NODES;

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
