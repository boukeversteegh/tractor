//! Per-kind transformations for Rust.
//!
//! Each function is a `Rule::Custom` target — `rule(RustKind) -> Rule`
//! references these by name. Simple flattens / pure renames /
//! `extract op + rename` patterns live as data in `rule()` (see
//! `rules.rs`), not here.

use xot::{Xot, Node as XotNode};

use crate::transform::{TransformAction, helpers::*};
use crate::transform::generic_type::rewrite_generic_type;

use super::output::*;

/// Kinds whose name happens to match our semantic vocabulary already
/// (`crate`, `label`, `self`, `super`, `attribute`) or grammar
/// supertypes that survive as raw kind names.
pub fn passthrough(_xot: &mut Xot, _node: XotNode) -> Result<TransformAction, xot::Error> {
    Ok(TransformAction::Continue)
}

/// `expression_statement` — drop the wrapper before children are
/// visited so children's parent context becomes the enclosing block.
pub fn skip(_xot: &mut Xot, _node: XotNode) -> Result<TransformAction, xot::Error> {
    Ok(TransformAction::Skip)
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

/// `line_comment` / `block_comment` / `doc_comment` — Tree-sitter Rust
/// emits all three; collapse to the shared `<comment>` vocabulary,
/// then run the shared classifier (trailing/leading/floating +
/// line-comment grouping). Doc comments group naturally because they
/// share the `//` prefix family.
pub fn comment(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    rename(xot, node, COMMENT);
    static CLASSIFIER: crate::languages::comments::CommentClassifier =
        crate::languages::comments::CommentClassifier { line_prefixes: &["//"] };
    CLASSIFIER.classify_and_group(xot, node, TRAILING, LEADING)
}

/// `identifier` / `field_identifier` / `shorthand_field_identifier` —
/// always names. Tree-sitter Rust uses distinct kinds for type
/// positions, so bare identifiers never need a heuristic.
pub fn identifier(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    rename(xot, node, NAME);
    Ok(TransformAction::Continue)
}

/// `type_identifier` / `primitive_type` — type references. Render as
/// `<type><name>i32</name></type>` for the unified vocabulary.
pub fn type_identifier(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    rename(xot, node, TYPE);
    wrap_text_in_name(xot, node)?;
    Ok(TransformAction::Continue)
}

/// `match_pattern` — normalise to `<pattern>` so `//match/arm/pattern`
/// is the uniform shape. The specific pattern form (identifier /
/// literal / tuple / struct / `_`) is exposed via child structure.
pub fn match_pattern(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    rename(xot, node, PATTERN);
    Ok(TransformAction::Continue)
}

/// `generic_type` — rewrite `Vec<T>` as
///   `<type><generic/>Vec<type field="arguments">T</type></type>`
/// matching the cross-language pattern.
pub fn generic_type(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    rewrite_generic_type(xot, node, &["type_identifier", "scoped_type_identifier"])?;
    Ok(TransformAction::Continue)
}

/// `type_parameter` — inline the parameter's name as a `<name>TEXT</name>`
/// child so siblings like trait_bounds remain intact.
pub fn type_parameter(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    replace_identifier_with_name_child(xot, node, &["type_identifier"])?;
    rename(xot, node, GENERIC);
    Ok(TransformAction::Continue)
}

/// `type_parameters` — generic parameter list. Distribute `field=
/// "generics"` to each child, rename to `<generics>`, then flatten
/// (matches the original transform's behavior; the rename before
/// flatten is preserved for parity).
pub fn type_parameters(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    distribute_field_to_children(xot, node, "generics");
    rename(xot, node, GENERICS);
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
        if get_kind(xot, child).as_deref() == Some("attribute") {
            prepend_empty_element(xot, child, INNER)?;
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

    insert_text_after(xot, node, &trimmed)?;
    Ok(TransformAction::Done)
}

/// `raw_string_literal` — rename to `<string>` and prepend `<raw/>`.
pub fn raw_string_literal(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    prepend_empty_element(xot, node, RAW)?;
    rename(xot, node, STRING);
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
    rename(xot, node, LITERAL);
    Ok(TransformAction::Continue)
}

/// `let_declaration` — `let mut x = …`, `let async x = …`, …. Extract
/// modifier keywords as marker children, then rename to `<let>`.
pub fn let_declaration(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    extract_modifiers(xot, node)?;
    rename(xot, node, LET);
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
) -> Option<&'static str> {
    let has_vis = xot.children(node).any(|child| {
        get_kind(xot, child).as_deref() == Some("visibility_modifier")
    });
    if has_vis {
        None
    } else {
        Some(PRIVATE)
    }
}

// ---------------------------------------------------------------------
// Local helpers used by handlers above.
// ---------------------------------------------------------------------

fn extract_modifiers(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    let texts = get_text_children(xot, node);
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
