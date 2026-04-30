//! Per-kind transformations for C#.
//!
//! Each function is a `Rule::Custom` target — `rule(CsKind) -> Rule`
//! references these by name. A transformation owns the renaming,
//! child reshaping, and `TransformAction` choice for kinds that don't
//! fit a shared `Rule` variant.
//!
//! Simple flattens / pure renames / `extract op + rename` patterns
//! live as data in `rule()` (see `semantic.rs`), not here.

use xot::{Xot, Node as XotNode};

use crate::transform::{TransformAction, helpers::*};
use crate::transform::operators::extract_operator;

use super::output::CsName::{
    self, Accessor, Else, Generic, If, Internal, Leading, Name, Nullable, Private, Public,
    String as CsString, Ternary, This, Trailing, Type, Unary, Variable,
};

/// Kinds whose name happens to match our semantic vocabulary already
/// (`discard`, `subpattern`, `interpolation`, `alias_qualified_name`)
/// — leave them unchanged. Also used for kinds the post-transform
/// pipeline consumes (`type_parameter_constraint`, etc.) and for
/// kinds the grammar emits but the C# transform never rewrites.
pub fn passthrough(_xot: &mut Xot, _node: XotNode) -> Result<TransformAction, xot::Error> {
    Ok(TransformAction::Continue)
}

/// `<name>` field wrapper inserted by the builder for nodes with a
/// `field=name` attribute. Inline the single identifier child as text:
///   `<name><identifier>Foo</identifier></name>`    →  `<name>Foo</name>`
///   `<name><type_identifier>Foo</type_identifier>` →  `<name>Foo</name>`
///   `<name><name>Foo</name></name>`                →  `<name>Foo</name>`
///
/// For qualified / scoped names (`System.Text`, etc.) concat descendant
/// text so the outer `<name>` holds the full dotted path as a single
/// text leaf — Principle #14.
///
/// Called from the dispatcher's wrapper branch, not from the rule
/// table — the node has no `kind=` attribute since it was synthesised
/// by the builder, not emitted by tree-sitter.
pub fn name_wrapper(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let children: Vec<_> = xot.children(node).collect();
    let element_children: Vec<_> = children
        .iter()
        .copied()
        .filter(|&c| xot.element(c).is_some())
        .collect();
    if element_children.len() == 1 {
        let child = element_children[0];
        let child_kind = get_kind(xot, child);
        let is_identifier = matches!(
            child_kind.as_deref(),
            Some("identifier") | Some("type_identifier") | Some("property_identifier")
        );
        let is_inlined_name = get_element_name(xot, child).as_deref() == Some("name");
        let is_qualified = matches!(
            child_kind.as_deref(),
            Some("qualified_name") | Some("generic_name") | Some("alias_qualified_name")
        );
        if is_identifier || is_inlined_name {
            if let Some(text) = get_text_content(xot, child) {
                for c in children {
                    xot.detach(c)?;
                }
                let text_node = xot.new_text(&text);
                xot.append(node, text_node)?;
                return Ok(TransformAction::Done);
            }
        } else if is_qualified {
            let text = descendant_text(xot, child);
            if !text.is_empty() {
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

/// `comment` — normalise to `<comment>` and run the shared
/// trailing/leading/floating classifier with `//` line-comment
/// grouping.
pub fn comment(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    static CLASSIFIER: crate::languages::comments::CommentClassifier =
        crate::languages::comments::CommentClassifier { line_prefixes: &["//"] };
    CLASSIFIER.classify_and_group(xot, node, Trailing.as_str(), Leading.as_str())
}

/// `interpolated_string_expression` — rename to the shared `<string>`
/// so the cross-language shape holds: `<string>` wraps interpolation
/// children matching Python f-strings, TS templates, Ruby double-
/// quotes, and PHP.
pub fn interpolated_string_expression(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    rename(xot, node, CsString);
    Ok(TransformAction::Continue)
}

/// `implicit_type` — C#'s `var` keyword in a type position. Render as
/// `<type><name>var</name></type>` for uniform querying.
pub fn implicit_type(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    rename(xot, node, Type);
    wrap_text_in_name(xot, node)?;
    Ok(TransformAction::Continue)
}

/// `predefined_type` — keywords like `int`, `string`. Same shape as
/// `implicit_type`.
pub fn predefined_type(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    rename(xot, node, Type);
    wrap_text_in_name(xot, node)?;
    Ok(TransformAction::Continue)
}

/// `accessor_declaration` — list of accessor kinds (get/set/init/add/remove).
/// Rename the node to whichever accessor kind it carries; fall back to
/// `<accessor>` if the text doesn't match a known kind. Principle #11:
/// the specific name is the node itself, not `<accessor><get/></accessor>`.
pub fn accessor_declaration(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    const KINDS: &[&str] = &["get", "set", "init", "add", "remove"];
    let children: Vec<_> = xot.children(node).collect();
    for child in children {
        let raw = match xot.text_str(child) {
            Some(t) => t.to_string(),
            None => continue,
        };
        let stripped = raw.trim().trim_end_matches(';').trim();
        if let Some(&accessor_kind) = KINDS.iter().find(|&&k| k == stripped) {
            rename(xot, node, accessor_kind);
            return Ok(TransformAction::Continue);
        }
    }
    rename(xot, node, Accessor);
    Ok(TransformAction::Continue)
}

/// `modifier` — text → marker conversion. Known modifiers (`public`,
/// `static`, `this`, …) become empty marker children; the source
/// keyword is preserved as a dangling sibling so the enclosing
/// declaration's XPath string-value still contains the keyword
/// (Principle #7).
pub fn modifier(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    if let Some(text) = get_text_content(xot, node) {
        let text = text.trim().to_string();
        if is_known_modifier(&text) {
            rename_to_marker(xot, node, &text)?;
            insert_text_after(xot, node, &text)?;
            return Ok(TransformAction::Done);
        }
    }
    Ok(TransformAction::Continue)
}

/// `nullable_type` — convert `<nullable_type><identifier>Guid</identifier>?`
/// to `<type kind="nullable_type">Guid<nullable/></type>`.
pub fn nullable_type(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let children: Vec<_> = xot.children(node).collect();
    for child in children {
        if let Some(child_kind) = get_kind(xot, child) {
            if matches!(
                child_kind.as_str(),
                "identifier" | "predefined_type" | "type_identifier"
            ) {
                if let Some(type_text) = get_text_content(xot, child) {
                    let all_children: Vec<_> = xot.children(node).collect();
                    for c in all_children {
                        xot.detach(c)?;
                    }
                    rename(xot, node, Type);
                    let text_node = xot.new_text(&type_text);
                    xot.append(node, text_node)?;
                    let nullable_name = xot.add_name(Nullable.as_str());
                    let nullable_el = xot.new_element(nullable_name);
                    xot.append(node, nullable_el)?;
                    return Ok(TransformAction::Done);
                }
            }
        }
    }
    rename(xot, node, Type);
    Ok(TransformAction::Continue)
}

/// `identifier` — context-dependent classification. Decide whether
/// the identifier names a binding or a type reference based on parent
/// kind and sibling shape; rename accordingly. If classified as a
/// type reference, wrap its text in a `<name>` so `//type[name='Foo']`
/// matches uniformly across declaration and reference sites.
pub fn identifier(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let classification = classify_identifier(xot, node);
    rename(xot, node, classification);
    if classification == Type {
        wrap_text_in_name(xot, node)?;
    }
    Ok(TransformAction::Continue)
}

/// `generic_name` — `List<T>`. Rewrite as
/// `<type><generic/><name>List</name><arguments>…</arguments></type>`.
pub fn generic_name(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let mut type_name = String::new();
    let children: Vec<_> = xot.children(node).collect();

    for child in &children {
        if let Some(child_kind) = get_kind(xot, *child) {
            if child_kind == "identifier" {
                if let Some(text) = get_text_content(xot, *child) {
                    type_name = text;
                }
                xot.detach(*child)?;
            }
        }
    }

    rename(xot, node, Type);

    let generic_name_id = xot.add_name(Generic.as_str());
    let generic_el = xot.new_element(generic_name_id);
    xot.prepend(node, generic_el)?;

    if !type_name.is_empty() {
        let name_id = xot.add_name(Name.as_str());
        let name_el = xot.new_element(name_id);
        let text_node = xot.new_text(&type_name);
        xot.append(name_el, text_node)?;
        xot.insert_after(generic_el, name_el)?;
    }

    Ok(TransformAction::Continue)
}

/// `conditional_expression` — ternary `a ? b : c`. Wrap the
/// `alternative` field child in `<else>` so the shared conditional-
/// shape post-transform can collapse the chain uniformly. Rename to
/// `<ternary>`.
pub fn conditional_expression(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    wrap_field_child(xot, node, "alternative", Else)?;
    rename(xot, node, Ternary);
    Ok(TransformAction::Continue)
}

/// `if_statement` — C#'s tree-sitter doesn't emit an `else_clause`
/// wrapper; the `alternative` field of an if_statement points
/// directly at the nested if_statement (for `else if`) or a block
/// (for final `else {…}`). Wrap the alternative in `<else>` so the
/// shared conditional-shape post-transform can collapse the chain
/// uniformly.
pub fn if_statement(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    wrap_field_child(xot, node, "alternative", Else)?;
    rename(xot, node, If);
    Ok(TransformAction::Continue)
}

/// `variable_declaration` — flat-promote when the parent already
/// provides the semantic container (a `<field>` declaration); rename
/// to `<variable>` for local declarations where this node IS the
/// declaration.
pub fn variable_declaration(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    let parent_kind = get_parent(xot, node).and_then(|parent| get_kind(xot, parent));
    if parent_kind.as_deref() == Some("field_declaration") {
        Ok(TransformAction::Flatten)
    } else {
        rename(xot, node, Variable);
        Ok(TransformAction::Continue)
    }
}

/// `postfix_unary_expression` — `x!`, `x++`. Same shape as
/// `unary_expression` (extract operator + rename to `<unary>`); kept
/// as a Custom rather than `ExtractOpThenRename` because postfix
/// operators sit *after* the operand and we want a stable arm name
/// in case future C# additions need to differentiate.
pub fn postfix_unary_expression(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    extract_operator(xot, node)?;
    rename(xot, node, Unary);
    Ok(TransformAction::Continue)
}

// ---------------------------------------------------------------------
// Default-access resolver consumed by `Rule::DefaultAccessThenRename`.
// All 9 C# declaration kinds (class / struct / interface / enum /
// record / method / constructor / property / field) use this shape via
// the shared rule variant; the language-specific bit lives here.
// ---------------------------------------------------------------------

/// Returns `Some(marker_name)` when the declaration node has no
/// explicit access modifier and should receive a default marker;
/// `None` when an access modifier is already present.
///
/// The default depends on enclosing scope (interface members → public;
/// class/struct/record members → private; top-level types → internal),
/// see `default_access_modifier`.
pub fn default_access_for_declaration(
    xot: &Xot,
    node: XotNode,
) -> Option<&'static str> {
    if has_access_modifier_child(xot, node) {
        None
    } else {
        Some(default_access_modifier(xot, node))
    }
}

// ---------------------------------------------------------------------
// Local helpers used by `default_access_for_declaration`.
// ---------------------------------------------------------------------

fn is_named_declaration(kind: &str) -> bool {
    matches!(
        kind,
        "class_declaration"
            | "struct_declaration"
            | "interface_declaration"
            | "enum_declaration"
            | "record_declaration"
            | "namespace_declaration"
            | "method_declaration"
            | "constructor_declaration"
            | "property_declaration"
            | "enum_member_declaration"
            | "parameter"
            | "variable_declarator"
            | "type_parameter"
            | "attribute"
    )
}

fn classify_identifier(xot: &Xot, node: XotNode) -> CsName {
    if let Some(field) = get_attr(xot, node, "field") {
        if field == "type" {
            return Type;
        }
    }

    let parent = match get_parent(xot, node) {
        Some(p) => p,
        None => return Type,
    };

    let parent_kind = get_kind(xot, parent).unwrap_or_default();

    if parent_kind == "name" {
        if let Some(grandparent) = get_parent(xot, parent) {
            let grandparent_kind = get_kind(xot, grandparent).unwrap_or_default();
            if is_named_declaration(&grandparent_kind) {
                return Name;
            }
        }
    }

    let in_namespace = is_in_namespace_context(xot, node);
    if parent_kind == "qualified_name" && in_namespace {
        return Name;
    }

    let siblings = get_following_siblings(xot, node);
    let has_param_sibling = siblings.iter().any(|&s| {
        get_kind(xot, s)
            .map(|n| matches!(n.as_str(), "parameter_list" | "parameters"))
            .unwrap_or(false)
    });

    match parent_kind.as_str() {
        "method_declaration" | "constructor_declaration" if has_param_sibling => Name,
        "class_declaration"
        | "struct_declaration"
        | "interface_declaration"
        | "enum_declaration"
        | "record_declaration"
        | "namespace_declaration" => Name,
        "variable_declarator" => Name,
        "parameter" => Name,
        "generic_name" => Type,
        "type_argument_list" | "type_parameter" => Type,
        "base_list" => Type,
        _ => Name,
    }
}

fn is_in_namespace_context(xot: &Xot, node: XotNode) -> bool {
    let mut current = get_parent(xot, node);
    while let Some(parent) = current {
        if let Some(kind) = get_kind(xot, parent) {
            match kind.as_str() {
                "namespace_declaration" => return true,
                "class_declaration"
                | "struct_declaration"
                | "interface_declaration"
                | "enum_declaration"
                | "record_declaration" => return false,
                _ => {}
            }
        }
        current = get_parent(xot, parent);
    }
    false
}

fn has_access_modifier_child(xot: &Xot, node: XotNode) -> bool {
    for child in xot.children(node) {
        if let Some(kind) = get_kind(xot, child) {
            if kind == "modifier" {
                if let Some(text) = get_text_content(xot, child) {
                    if is_access_modifier(text.trim()) {
                        return true;
                    }
                }
            }
        }
        if let Some(name) = get_element_name(xot, child) {
            if is_access_modifier(&name) {
                return true;
            }
        }
    }
    false
}

fn is_access_modifier(text: &str) -> bool {
    super::transform::ACCESS_MODIFIERS.iter().any(|m| m.as_str() == text)
}

fn is_known_modifier(text: &str) -> bool {
    super::transform::ACCESS_MODIFIERS.iter().any(|m| m.as_str() == text)
        || super::transform::OTHER_MODIFIERS.iter().any(|m| m.as_str() == text)
        || text == This.as_str()
}

fn default_access_modifier(xot: &Xot, node: XotNode) -> &'static str {
    let mut current = get_parent(xot, node);
    while let Some(parent) = current {
        if let Some(parent_kind) = get_kind(xot, parent).as_deref().map(str::to_owned) {
            match parent_kind.as_str() {
                "interface_declaration" => return Public.as_str(),
                "class_declaration" | "struct_declaration" | "record_declaration" => return Private.as_str(),
                "declaration_list" => {}
                _ => break,
            }
        }
        current = get_parent(xot, parent);
    }
    Internal.as_str()
}
