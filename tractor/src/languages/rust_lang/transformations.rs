//! Per-kind transformations for Rust.
//!
//! Each function is a `Rule::Custom` target — `rule(RustKind) -> Rule`
//! references these by name. Simple flattens / pure renames /
//! `extract op + rename` patterns live as data in `rule()` (see
//! `rules.rs`), not here.

use xot::{Xot, Node as XotNode};

use crate::transform::{TransformAction, helpers::*};
use crate::transform::generic_type::rewrite_generic_type;

use super::input::RustKind;
use super::output::TractorNode::{
    self, Async, Await, Borrowed, Comment as CommentName, Const, Crate, Expression, Extern,
    Generic, Generics, In as InName, Inner, Leading, Let, Literal, Mut, Name, Parameter, Pattern,
    Private, Pub, Raw, Static, String as RustString, Super, Trailing, Try, Type, Unsafe,
    Use as UseName,
};

/// `trait_bounds` — `: Clone + Send + 'static` after a generic
/// parameter, type parameter, or trait header. Per Principle #12 (no
/// list containers) + #18 (name relationships after the operator):
/// each constraint becomes a flat `<extends list="extends">`
/// sibling, matching Java's `<T extends A & B>` shape and the
/// cross-language relationship-naming rule. Drops the `<bounds>`
/// wrapper.
///
/// Special bound forms lift their marker onto the `<extends>` itself
/// rather than producing an inner `<bound[marker]>` wrapper:
///   * `removed_trait_bound` (`?Sized`)  → `<extends[optional]>...`
///   * `higher_ranked_trait_bound` (`for<'a> Fn(...)`) → `<extends[higher]>...`
/// Pre-iter-126 these rendered as `<extends>/<bound[optional]>/...`,
/// awkward for `//extends/type[...]` queries.
pub fn trait_bounds(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    use super::output::TractorNode::{Extends, Higher, Optional};
    let elem_children: Vec<XotNode> = xot.children(node)
        .filter(|&c| xot.element(c).is_some())
        .collect();
    for child in elem_children {
        let ext_elt = xot.add_name(Extends.as_str());
        let ext_node = xot.new_element(ext_elt);
        xot.insert_before(child, ext_node)?;
        let child_kind = get_kind(xot, child);
        let marker = match child_kind.as_deref() {
            Some("removed_trait_bound") => Some(Optional),
            Some("higher_ranked_trait_bound") => Some(Higher),
            _ => None,
        };
        if marker.is_some() {
            // Lift the bound's element children directly into the
            // `<extends>` wrapper, then drop the bound element. The
            // text leaves (`?` for removed, `for`/`<`/`>` for higher)
            // are pure source markers; the lifted marker captures the
            // semantic.
            let inner_elements: Vec<XotNode> = xot.children(child)
                .filter(|&c| xot.element(c).is_some())
                .collect();
            for inner in inner_elements {
                xot.detach(inner)?;
                xot.append(ext_node, inner)?;
            }
            xot.detach(child)?;
        } else {
            xot.detach(child)?;
            xot.append(ext_node, child)?;
        }
        xot.with_attr(ext_node, "list", "extends");
        if let Some(m) = marker {
            xot.with_appended_marker(ext_node, m)?;
        }
    }
    Ok(TransformAction::Flatten)
}

/// `expression_statement` — wrap value-producing statements in an
/// `<expression>` host (Principle #15). Control-flow constructs used
/// as statements (`if`, `for`, `while`, `loop`, `match`, `return`,
/// `break`, `continue`, `block`, `unsafe_block`, `async_block`,
/// `try_block`, `const_block`) drop the wrapper — they're structural,
/// not annotations on a value, so they sit directly in the body.
pub fn expression_statement(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let inner_kind = xot.children(node)
        .find(|&c| xot.element(c).is_some())
        .and_then(|c| get_kind(xot, c));
    let is_control_flow = matches!(
        inner_kind.as_deref(),
        Some(
            "if_expression" | "for_expression" | "while_expression" | "loop_expression"
            | "match_expression" | "return_expression" | "break_expression"
            | "continue_expression" | "block" | "unsafe_block" | "async_block"
            | "try_block" | "const_block"
        )
    );
    if is_control_flow {
        Ok(TransformAction::Skip)
    } else {
        xot.with_renamed(node, Expression);
        Ok(TransformAction::Continue)
    }
}

/// Legacy skip used by other kinds; kept for compatibility while the
/// migration proceeds. Drops the wrapper, promotes children to parent.
pub fn skip(_xot: &mut Xot, _node: XotNode) -> Result<TransformAction, xot::Error> {
    Ok(TransformAction::Skip)
}

/// `try_expression` — `foo()?`. Promote to `<expression>` host with a
/// trailing `<try/>` marker (postfix in source, marker order matches).
/// See [Principle #15: Stable Expression Hosts].
pub fn try_expression(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_renamed(node, Expression)
        .with_appended_marker(node, Try)?;
    Ok(TransformAction::Continue)
}

/// `await_expression` — `foo().await`. Rust's `.await` is postfix, so
/// the marker trails the operand. Promote to `<expression>` host with
/// a trailing `<await/>` marker. See [Principle #15: Stable Expression
/// Hosts].
pub fn await_expression(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_renamed(node, Expression)
        .with_appended_marker(node, Await)?;
    Ok(TransformAction::Continue)
}

/// `<name>` field wrapper inserted by the builder. Rust-specific:
/// when the single child is a `lifetime` (e.g. for type parameters
/// or named loops), inline the lifetime's descendant text directly.
/// Otherwise inline the standard identifier-family children.
pub fn name_wrapper(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let element_children: Vec<_> = xot
        .children(node)
        .filter(|&c| xot.element(c).is_some())
        .collect();
    if element_children.len() == 1 {
        let child = element_children[0];
        if get_kind(xot, child).and_then(|kind| kind.parse::<RustKind>().ok())
            == Some(RustKind::Lifetime) {
            let text = descendant_text(xot, child);
            let trimmed = text.trim().to_string();
            if !trimmed.is_empty() {
                xot.with_only_text(node, &trimmed)?;
                return Ok(TransformAction::Done);
            }
        }
    }
    inline_single_identifier(xot, node)?;
    Ok(TransformAction::Continue)
}

/// `line_comment` / `block_comment` / `doc_comment` — Tree-sitter Rust
/// emits all three; collapse to the shared `<comment>` vocabulary,
/// then run the shared classifier (trailing/leading/floating +
/// line-comment grouping). Doc comments group naturally because they
/// share the `//` prefix family.
pub fn comment(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_renamed(node, CommentName);
    static CLASSIFIER: crate::languages::comments::CommentClassifier =
        crate::languages::comments::CommentClassifier { line_prefixes: &["//"] };
    CLASSIFIER.classify_and_group(xot, node, Trailing, Leading)
}

/// `identifier` / `field_identifier` / `shorthand_field_identifier` —
/// always names. Tree-sitter Rust uses distinct kinds for type
/// positions, so bare identifiers never need a heuristic.
pub fn identifier(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_renamed(node, Name);
    Ok(TransformAction::Continue)
}

/// `type_identifier` / `primitive_type` — type references. Render as
/// `<type><name>i32</name></type>` for the unified vocabulary.
pub fn type_identifier(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_renamed(node, Type);
    wrap_text_in_name(xot, node)?;
    Ok(TransformAction::Continue)
}

/// `match_pattern` — normalise to `<pattern>` so `//match/arm/pattern`
/// is the uniform shape. The specific pattern form (identifier /
/// literal / tuple / struct / `_`) is exposed via child structure.
pub fn match_pattern(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_renamed(node, Pattern);
    Ok(TransformAction::Continue)
}

/// `generic_type` — rewrite `Vec<T>` as
///   `<type><generic/>Vec<type list="arguments">T</type></type>`
/// matching the cross-language pattern.
pub fn generic_type(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    rewrite_generic_type(xot, node, &["type_identifier", "scoped_type_identifier"])?;
    Ok(TransformAction::Continue)
}

/// `type_parameter` — inline the parameter's name as a `<name>TEXT</name>`
/// child so siblings like trait_bounds remain intact.
pub fn type_parameter(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    replace_identifier_with_name_child(xot, node, &["type_identifier"])?;
    xot.with_renamed(node, Generic);
    Ok(TransformAction::Continue)
}

/// `type_parameters` — generic parameter list. Distribute `field=
/// "generics"` to each child, rename to `<generics>`, then flatten
/// (matches the original transform's behavior; the rename before
/// flatten is preserved for parity).
pub fn type_parameters(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    distribute_list_to_children(xot, node, "generics");
    xot.with_renamed(node, Generics);
    Ok(TransformAction::Flatten)
}

/// `inner_attribute_item` — `#![attr]`. Mark the inner attribute with
/// `<inner/>` so queries can distinguish inner (scope-level) from
/// outer (item-level) attributes, then flatten the wrapper.
pub fn inner_attribute_item(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    let children: Vec<_> = xot.children(node).collect();
    for child in children {
        if get_kind(xot, child).and_then(|kind| kind.parse::<RustKind>().ok())
            == Some(RustKind::Attribute) {
            xot.with_prepended_marker(child, Inner)?;
            break;
        }
    }
    Ok(TransformAction::Flatten)
}

/// `visibility_modifier` — `pub`, `pub(crate)`, `pub(super)`,
/// `pub(in path)`. Collapse the subtree into a single `<pub>` element
/// with a restriction marker child; dangle the original source token
/// as a sibling so string-value stays source-accurate.
pub fn visibility_modifier(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    let source = descendant_text(xot, node);
    let trimmed = source.trim().to_string();

    xot.with_detached_children(node)?
        .with_renamed(node, Pub);

    if let (Some(lp), Some(rp)) = (trimmed.find('('), trimmed.find(')')) {
        let inner = trimmed[lp + 1..rp].trim();
        match inner {
            "crate" => { xot.with_prepended_marker(node, Crate)?; }
            "super" => { xot.with_prepended_marker(node, Super)?; }
            _ if inner.starts_with("in ") => {
                let path = inner[3..].trim();
                xot.with_prepended_element_with_text(node, InName, path)?;
            }
            _ => {}
        }
    }

    xot.with_inserted_text_after(node, &trimmed)?;
    Ok(TransformAction::Done)
}

/// `raw_string_literal` — rename to `<string>` and prepend `<raw/>`.
pub fn raw_string_literal(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    xot.with_prepended_marker(node, Raw)?
        .with_renamed(node, RustString);
    Ok(TransformAction::Continue)
}

/// `reference_type` — `&T`, `&mut T`, `&'a T`. Render as a single
/// `<type>` with a `<borrowed/>` marker (Principle #14 + #13). The
/// inner referenced type is a nested `<type>` child, so
/// `//type[borrowed]` finds every reference and `//type[borrowed][mut]`
/// finds every mutable borrow.
pub fn reference_type(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    let children: Vec<_> = xot.children(node).collect();
    let mut has_mut = false;
    for child in &children {
        if get_kind(xot, *child).and_then(|kind| kind.parse::<RustKind>().ok())
            == Some(RustKind::MutableSpecifier) {
            has_mut = true;
            let text = get_text_content(xot, *child).unwrap_or_default();
            let text_node = xot.new_text(&text);
            xot.insert_before(*child, text_node)?;
            xot.detach(*child)?;
        }
    }
    if has_mut {
        xot.with_prepended_marker(node, Mut)?;
    }
    xot.with_prepended_marker(node, Borrowed)?
        .with_renamed(node, Type);
    Ok(TransformAction::Continue)
}

/// `struct_expression` — `Point { x: 1, y: 2 }`. Render as
///   `<literal><name>Point</name><field>…</field></literal>`
/// — semantically a struct literal. The type-being-constructed is a
/// `<name>`, not a `<type>`, since this is reference-by-name.
pub fn struct_expression(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    replace_identifier_with_name_child(
        xot,
        node,
        &["type_identifier", "scoped_type_identifier"],
    )?;
    xot.with_renamed(node, Literal);
    Ok(TransformAction::Continue)
}

/// `let_declaration` — `let mut x = …`, `let async x = …`, …. Extract
/// modifier keywords as marker children, then rename to `<let>`.
pub fn let_declaration(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    extract_modifiers(xot, node)?;
    xot.with_renamed(node, Let);
    Ok(TransformAction::Continue)
}

/// `closure_parameters` — `|x|`, `|a, b|`, `|x: i32|`. Tree-sitter
/// emits the bare-name forms as `identifier` children; typed forms
/// emit `parameter` (with name+type fields). Wrap each bare
/// identifier in `<parameter>` so the shape is uniform across both
/// forms (Principle #5 within-language).
pub fn closure_parameters(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    let candidates: Vec<XotNode> = xot.children(node)
        .filter(|&c| {
            matches!(
                get_kind(xot, c).and_then(|k| k.parse::<RustKind>().ok()),
                Some(RustKind::Identifier | RustKind::MutPattern | RustKind::SelfParameter)
            )
        })
        .collect();
    for child in candidates {
        let param_elt = xot.add_name(Parameter.as_str());
        let param_node = xot.new_element(param_elt);
        xot.insert_before(child, param_node)?;
        xot.detach(child)?;
        xot.append(param_node, child)?;
    }
    Ok(TransformAction::Flatten)
}

/// `static_item` — `static FOO: T = …` and `static mut FOO: T = …`.
/// Combines default-access resolution with `mut` modifier extraction
/// (Principle #7). Without this the `mutable_specifier` text leaks
/// inside `<static>` as bare `mut` keyword text.
pub fn static_item(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    if let Some(marker) = default_access_for_declaration(xot, node) {
        xot.with_prepended_marker(node, marker)?;
    }
    extract_modifiers(xot, node)?;
    xot.with_renamed(node, Static);
    Ok(TransformAction::Continue)
}

/// `function_modifiers` — tree-sitter wraps function-level keywords
/// (`async`, `const`, `unsafe`) inside a sub-element. Prepend a
/// corresponding empty marker for each keyword, leaving the source
/// text in place (so the parent function's XPath string-value still
/// contains the keyword per Principle #10), then flatten so the
/// markers lift onto the parent `<function>` (matching
/// `function[pub and async]` — same as C# `method[public and async]`,
/// Java `method[public and synchronized]`).
pub fn function_modifiers(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    extract_modifiers(xot, node)?;
    Ok(TransformAction::Flatten)
}

/// `foreign_mod_item` — `extern "C" { … }`. Rename to `<mod>` with
/// `<foreign/>` and `<extern/>` markers.
pub fn foreign_mod_item(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    use super::output::TractorNode::{Foreign, Mod};
    xot.with_renamed(node, Mod)
        .with_prepended_marker(node, Foreign)?
        .with_prepended_marker(node, Extern)?;
    Ok(TransformAction::Continue)
}

/// `extern_crate_declaration` — `extern crate alloc;`. Drop the literal
/// `extern` / `crate` keyword children (Tree-sitter exposes them as
/// `crate` elements that would otherwise carry text and violate the
/// marker-empty invariant), then rename to `<use>` with an `<extern/>`
/// marker so `//use[extern]` finds the legacy import form.
pub fn extern_crate_declaration(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    let to_remove: Vec<_> = xot.children(node)
        .filter(|&c| matches!(
            get_kind(xot, c).and_then(|k| k.parse::<RustKind>().ok()),
            Some(RustKind::Crate)
        ))
        .collect();
    for child in to_remove {
        xot.detach(child)?;
    }
    xot.with_prepended_marker(node, Extern)?
        .with_renamed(node, UseName);
    Ok(TransformAction::Continue)
}

// ---------------------------------------------------------------------
// Default-access resolver consumed by `Rule::DefaultAccessThenRename`.
// 8 Rust declaration kinds (function, struct, enum, trait, const,
// static, type, mod) use this directly via the shared rule variant.
// Default is always `private` — Rust's convention for "no `pub`
// modifier means item-private".
// ---------------------------------------------------------------------

/// Returns `Some(PRIVATE)` when the declaration node has no
/// `visibility_modifier` child; `None` when one is present.
pub fn default_access_for_declaration(
    xot: &Xot,
    node: XotNode,
) -> Option<TractorNode> {
    let has_vis = xot.children(node).any(|child| {
        get_kind(xot, child).and_then(|kind| kind.parse::<RustKind>().ok())
            == Some(RustKind::VisibilityModifier)
    });
    if has_vis {
        None
    } else {
        Some(Private)
    }
}

// ---------------------------------------------------------------------
// Local helpers used by handlers above.
// ---------------------------------------------------------------------

fn extract_modifiers(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    // Modifiers can appear as either:
    //  - anonymous text children (`async unsafe` inside function_modifiers)
    //  - element children with their own kind (`mutable_specifier` for `mut`)
    // Cover both. Element children are processed BEFORE they're flattened
    // by their own rule (top-down walk).
    let texts = get_text_children(xot, node);
    let mut found: Vec<TractorNode> = texts.iter()
        .filter_map(|t| t.parse().ok())
        .filter(|name| matches!(name, Mut | Async | Unsafe | Const))
        .collect();
    // Element children: any whose text content parses to a modifier name.
    for child in xot.children(node).collect::<Vec<_>>() {
        if xot.element(child).is_none() {
            continue;
        }
        let text = match get_text_content(xot, child) {
            Some(t) => t.trim().to_string(),
            None => continue,
        };
        if let Ok(name) = text.parse::<TractorNode>() {
            if matches!(name, Mut | Async | Unsafe | Const) && !found.contains(&name) {
                found.push(name);
            }
        }
    }

    // Source-location source: the keyword token is anonymous text inside
    // `node` (a let_declaration or function_modifiers wrapper); copy
    // `node`'s range onto each marker so `<async/>` carries the keyword's
    // line/column (Principle #10). When multiple modifiers share the
    // wrapper (`async unsafe fn`), they share the wrapper's range —
    // the best fidelity available without per-token source info.
    for modifier in found.into_iter().rev() {
        xot.with_prepended_marker(node, modifier)?;
    }
    Ok(())
}

fn inline_single_identifier(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    let children: Vec<_> = xot.children(node).collect();
    for child in children {
        if !matches!(
            get_element_name(xot, child).and_then(|name| name.parse::<RustKind>().ok()),
            Some(
                RustKind::Identifier
                    | RustKind::TypeIdentifier
                    | RustKind::FieldIdentifier
                    | RustKind::ShorthandFieldIdentifier
                    | RustKind::Metavariable
            )
        )
        {
            continue;
        }
        let text = match get_text_content(xot, child) {
            Some(t) => t,
            None => continue,
        };
        xot.with_only_text(node, &text)?;
        return Ok(());
    }
    Ok(())
}
