//! C# transform logic — thin dispatcher driven by `semantic::rule`.
//!
//! The per-kind logic is split:
//!   - Pure rename / flatten / shared compositions live as data in
//!     [`super::semantic::rule`].
//!   - Language-specific custom logic lives as named functions in
//!     [`super::transformations`].
//!
//! This file's job is just to look up the kind and execute its rule.
//! The only code that stays inline is the wrapper branch — handling
//! builder-inserted elements that have no `kind=` attribute.

use xot::{Xot, Node as XotNode};
use crate::transform::{TransformAction, helpers::*};
use crate::output::syntax_highlight::SyntaxCategory;

use super::kind::CsKind;
use super::semantic::*;

/// Transform a C# AST node.
///
/// Dispatch is split in two:
///   1. If the node carries a `kind` attribute (set by the builder
///      from the original tree-sitter kind), look it up in `CsKind`,
///      fetch its `Rule` from `semantic::rule`, and execute via the
///      shared [`crate::languages::rule::dispatch`].
///   2. Otherwise the node is a builder-inserted wrapper (e.g. the
///      `<name>` field wrapper) — handle inline.
pub fn transform(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let kind_str = match get_kind(xot, node) {
        Some(k) => k,
        None => {
            // Builder-inserted wrapper (no `kind` attribute).
            let name = get_element_name(xot, node).unwrap_or_default();
            return match name.as_str() {
                // Name wrappers — inline the single identifier child as text.
                //   <name><identifier>Foo</identifier></name>    →  <name>Foo</name>
                //   <name><type_identifier>Foo</type_identifier> →  <name>Foo</name>
                //   <name><name>Foo</name></name>                →  <name>Foo</name>
                //
                // For qualified / scoped names (`System.Text`, etc.) concat
                // descendant text so the outer <name> holds the full dotted
                // path as a single text leaf — Principle #14.
                "name" => inline_name_wrapper(xot, node),
                _ => Ok(TransformAction::Continue),
            };
        }
    };

    // Unknown kinds (synthetic nodes, parse errors) keep their kind
    // name unchanged — same behavior as the old `_` arm fallback when
    // `apply_rename` returned `None`.
    let kind = match CsKind::from_str(&kind_str) {
        Some(k) => k,
        None => return Ok(TransformAction::Continue),
    };

    crate::languages::rule::dispatch(xot, node, super::semantic::rule(kind))
}

fn inline_name_wrapper(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
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

/// C# access modifiers in canonical declaration order. Public so that
/// `transformations.rs` and the renderer can share the list.
pub const ACCESS_MODIFIERS: &[&str] = &[PUBLIC, PRIVATE, PROTECTED, INTERNAL];

/// C# non-access modifiers in canonical declaration order.
pub const OTHER_MODIFIERS: &[&str] = &[
    STATIC, ABSTRACT, VIRTUAL, OVERRIDE, SEALED,
    READONLY, CONST, PARTIAL, ASYNC, EXTERN, UNSAFE, NEW,
];

/// Map a transformed element name to a syntax category for highlighting.
pub fn syntax_category(element: &str) -> SyntaxCategory {
    super::semantic::spec(element)
        .map(|spec| spec.syntax)
        .unwrap_or(SyntaxCategory::Default)
}
