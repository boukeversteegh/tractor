//! Per-kind transformations for Go.
//!
//! Each function is a `Rule::Custom` target — `rule(GoKind) -> Rule`
//! references these by name. A transformation owns the renaming,
//! child reshaping, and `TransformAction` choice for kinds that don't
//! fit a shared `Rule` variant.
//!
//! Simple flattens / pure renames / `extract op + rename` patterns
//! live as data in `rule()` (see `semantic.rs`), not here.

use xot::{Xot, Node as XotNode};

use crate::transform::{TransformAction, helpers::*};

use super::output::GoName::{
    Alias, Comment as CommentName, Else, Exported, Field, Function, If, Interface, Leading,
    Method, Name, Raw, Short, String as GoString, Struct, Trailing, Type, Unexported, Variable,
};

/// Kinds whose name happens to match our semantic vocabulary already
/// (`iota`, `dot`, `array_type`, …) — leave them unchanged.
pub fn passthrough(_xot: &mut Xot, _node: XotNode) -> Result<TransformAction, xot::Error> {
    Ok(TransformAction::Continue)
}

/// `expression_statement` is a pure grammar wrapper around a single
/// expression. Skip its subtree so the inner expression's transform
/// drives the output (matches the previous behavior of returning
/// `TransformAction::Skip` directly).
pub fn expression_statement(_xot: &mut Xot, _node: XotNode) -> Result<TransformAction, xot::Error> {
    Ok(TransformAction::Skip)
}

/// `parameter_list` does double duty in Go: formal parameters AND
/// multi-value return specs. The builder has already wrapped the
/// returns case in a `<returns>` element; check the parent to decide.
pub fn parameter_list(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let in_returns = get_parent(xot, node)
        .and_then(|p| get_element_name(xot, p))
        .as_deref()
        == Some("returns");
    if in_returns {
        collapse_return_param_list(xot, node)?;
    } else {
        distribute_field_to_children(xot, node, "parameters");
    }
    Ok(TransformAction::Flatten)
}

/// `type_declaration` — move the leading `type` keyword text into
/// the inner `type_spec` / `type_alias` so the keyword stays attached
/// when the outer wrapper is flattened.
pub fn type_declaration(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    move_type_keyword_into_spec(xot, node)?;
    Ok(TransformAction::Flatten)
}

/// `raw_string_literal` — render as `<string>` with a `<raw/>` marker.
pub fn raw_string_literal(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    prepend_empty_element(xot, node, Raw)?;
    rename(xot, node, GoString);
    Ok(TransformAction::Continue)
}

/// `short_var_declaration` (`x := 42`) — render as `<variable>` with
/// a `<short/>` marker to distinguish from `var x = 42`.
pub fn short_var_declaration(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    prepend_empty_element(xot, node, Short)?;
    rename(xot, node, Variable);
    Ok(TransformAction::Continue)
}

/// `function_declaration` — prepend `<exported/>` / `<unexported/>`
/// based on the function name's capitalisation, then rename to
/// `<function>`.
pub fn function_declaration(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let marker = get_export_marker(xot, node);
    prepend_empty_element(xot, node, marker)?;
    rename(xot, node, Function);
    Ok(TransformAction::Continue)
}

/// `method_declaration` — same export-marker pattern, rename to
/// `<method>`.
pub fn method_declaration(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let marker = get_export_marker(xot, node);
    prepend_empty_element(xot, node, marker)?;
    rename(xot, node, Method);
    Ok(TransformAction::Continue)
}

/// `field_declaration` — same export-marker pattern (Go capitalisation
/// rule applies to struct fields too), rename to `<field>`.
pub fn field_declaration(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let marker = get_export_marker(xot, node);
    prepend_empty_element(xot, node, marker)?;
    rename(xot, node, Field);
    Ok(TransformAction::Continue)
}

/// `type_spec` — three shapes:
///   - `type Hello struct {…}`   → `<struct><name>Hello</name>…</struct>`
///   - `type Greeter interface…`  → `<interface><name>Greeter</name>…</interface>`
///   - `type MyInt int`           → `<type><name>MyInt</name><type>int</type></type>`
///
/// For the first two, hoist the inner shape up so the declaration
/// reads "I'm declaring a struct named Hello" (Goal #5).
pub fn type_spec(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let marker = get_export_marker(xot, node);
    prepend_empty_element(xot, node, marker)?;

    let inner = xot
        .children(node)
        .filter(|&c| xot.element(c).is_some())
        .find(|&c| matches!(get_kind(xot, c).as_deref(), Some("struct_type") | Some("interface_type")));

    if let Some(inner) = inner {
        let inner_kind = get_kind(xot, inner).unwrap();
        let new_name = if inner_kind == "struct_type" { Struct } else { Interface };
        rename(xot, node, new_name);
        let inner_children: Vec<_> = xot.children(inner).collect();
        for c in inner_children {
            xot.detach(c)?;
            xot.insert_before(inner, c)?;
        }
        xot.detach(inner)?;
    } else {
        rename(xot, node, Type);
    }
    Ok(TransformAction::Continue)
}

/// `type_alias` (`type Color = int`) — distinct from `type MyInt int`.
/// Rename to `<alias>` with the export marker.
pub fn type_alias(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let marker = get_export_marker(xot, node);
    prepend_empty_element(xot, node, marker)?;
    rename(xot, node, Alias);
    Ok(TransformAction::Continue)
}

/// `if_statement` — Go's tree-sitter doesn't emit an `else_clause`
/// wrapper; the `alternative` field points directly at a nested
/// `if_statement` (for `else if`) or a block. Wrap the alternative in
/// `<else>` so the shared conditional-shape post-transform can
/// collapse the chain uniformly.
pub fn if_statement(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    wrap_field_child(xot, node, "alternative", Else)?;
    rename(xot, node, If);
    Ok(TransformAction::Continue)
}

/// `type_identifier` — rename to `<type>` and wrap the text in
/// `<name>` so `//type[name='Foo']` matches uniformly across
/// declaration and reference sites.
pub fn type_identifier(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    rename(xot, node, Type);
    wrap_text_in_name(xot, node)?;
    Ok(TransformAction::Continue)
}

/// `<name>` field wrapper inserted by the builder for nodes with a
/// `field=name` attribute. Inline the single identifier-like child
/// as text:
///   `<name><identifier>foo</identifier></name>` → `<name>foo</name>`
///
/// Also accepts:
///   - `package_identifier` (Go import alias `myio "io"`),
///   - already-renamed `<name>` (walk-order race),
///   - `dot` (Go's `import . "pkg"`),
///   - `blank_identifier` (Go's `_`).
///
/// Called from the dispatcher's wrapper branch, not from the rule
/// table — the node has no `kind=` attribute since it was synthesised
/// by the builder, not emitted by tree-sitter.
pub fn name_wrapper(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let children: Vec<_> = xot.children(node).collect();
    for child in children {
        let child_name = match get_element_name(xot, child) {
            Some(n) => n,
            None => continue,
        };
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
        break;
    }
    Ok(TransformAction::Continue)
}

/// `comment` — normalise to `<comment>` and run the shared
/// trailing/leading/floating classifier with `//` line-comment
/// grouping.
pub fn comment(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    rename(xot, node, CommentName);
    static CLASSIFIER: crate::languages::comments::CommentClassifier =
        crate::languages::comments::CommentClassifier { line_prefixes: &["//"] };
    CLASSIFIER.classify_and_group(xot, node, Trailing.as_str(), Leading.as_str())
}

// ---------------------------------------------------------------------
// Local helpers — used by the handlers above. Mirror the same
// helpers in `transform.rs`; once the dispatcher swap lands and the
// match-based path is gone, the originals there can be deleted.
// ---------------------------------------------------------------------

/// Move the literal `type` keyword text from a `type_declaration` into
/// its inner `type_spec` / `type_alias` child.
fn move_type_keyword_into_spec(xot: &mut Xot, decl: XotNode) -> Result<(), xot::Error> {
    let keyword = match xot.children(decl).find(|&c| {
        xot.text_str(c).map(|t| t.trim() == "type").unwrap_or(false)
    }) {
        Some(k) => k,
        None => return Ok(()),
    };
    let spec = xot
        .children(decl)
        .filter(|&c| xot.element(c).is_some())
        .find(|&c| matches!(get_kind(xot, c).as_deref(), Some("type_spec") | Some("type_alias")));
    let spec = match spec {
        Some(s) => s,
        None => return Ok(()),
    };
    xot.detach(keyword)?;
    xot.prepend(spec, keyword)?;
    Ok(())
}

/// Strip the `<param>` wrapper from each return-type entry so a
/// returns list reads as a sequence of types, not parameters.
fn collapse_return_param_list(xot: &mut Xot, list: XotNode) -> Result<(), xot::Error> {
    let children: Vec<XotNode> = xot.children(list).filter(|&c| xot.element(c).is_some()).collect();
    for child in children {
        if get_kind(xot, child).as_deref() != Some("parameter_declaration") {
            continue;
        }
        let type_child = xot.children(child).find(|&c| {
            get_element_name(xot, c).as_deref() == Some("type")
                || matches!(
                    get_kind(xot, c).as_deref(),
                    Some(
                        "type_identifier"
                            | "pointer_type"
                            | "slice_type"
                            | "array_type"
                            | "map_type"
                            | "channel_type"
                            | "interface_type"
                            | "struct_type"
                            | "generic_type"
                    )
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

/// Determine `<exported/>` vs `<unexported/>` from the name child's
/// first-character capitalisation.
fn get_export_marker(xot: &Xot, node: XotNode) -> super::output::GoName {
    for child in xot.children(node) {
        if let Some(name) = get_element_name(xot, child) {
            if name == Name.as_str() {
                for grandchild in xot.children(child) {
                    if let Some(text) = get_text_content(xot, grandchild) {
                        if text.starts_with(|c: char| c.is_uppercase()) {
                            return Exported;
                        }
                        return Unexported;
                    }
                }
                if let Some(text) = get_text_content(xot, child) {
                    if text.starts_with(|c: char| c.is_uppercase()) {
                        return Exported;
                    }
                    return Unexported;
                }
            }
            if name == "identifier" || name == "type_identifier" {
                if let Some(field) = get_attr(xot, child, "field") {
                    if field == "name" {
                        if let Some(text) = get_text_content(xot, child) {
                            if text.starts_with(|c: char| c.is_uppercase()) {
                                return Exported;
                            }
                            return Unexported;
                        }
                    }
                }
            }
        }
    }
    Unexported
}
