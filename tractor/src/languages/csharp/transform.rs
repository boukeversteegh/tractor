//! C# transform logic — thin dispatcher driven by `rules::rule`.
//!
//! The per-kind logic is split:
//!   - Pure rename / flatten / shared compositions live as data in
//!     [`super::rules::rule`].
//!   - Language-specific custom logic lives as named functions in
//!     [`super::transformations`].
//!
//! This file's job is just to look up the kind and execute its rule.

use xot::{Xot, Node as XotNode};
use crate::transform::{TransformAction, helpers::*};
use crate::output::syntax_highlight::SyntaxCategory;

use super::input::CsKind;
use super::output::{self, TractorNode};
use TractorNode::{
    Abstract, Async, Const, Extern, Internal, New, Override, Partial, Private, Protected, Public,
    Readonly, Sealed, Static, Unsafe, Virtual,
};

/// Transform a C# AST node.
///
/// Dispatch is split in two:
///   1. If the node carries a `kind` attribute (set by the builder
///      from the original tree-sitter kind), look it up in `CsKind`,
///      fetch its `Rule` from `rules::rule`, and execute via the
///      shared [`crate::languages::rule::dispatch`].
///   2. Otherwise the node is a builder-inserted wrapper (e.g. the
///      `<name>` field wrapper) — handle inline.
pub fn transform(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let kind_str = match get_kind(xot, node) {
        Some(k) => k,
        None => {
            // Builder-inserted wrapper (no `kind` attribute) — dispatch
            // by element name to a transformation by the same name.
            let name = get_element_name(xot, node).unwrap_or_default();
            return match name.as_str() {
                "name" => super::transformations::name_wrapper(xot, node),
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

    crate::languages::rule::dispatch(xot, node, super::rules::rule(kind))
}

/// C# access modifiers in canonical declaration order. Public so that
/// `transformations.rs` and the renderer can share the list.
pub const ACCESS_MODIFIERS: &[TractorNode] = &[Public, Private, Protected, Internal];

/// C# non-access modifiers in canonical declaration order.
pub const OTHER_MODIFIERS: &[TractorNode] = &[
    Static, Abstract, Virtual, Override, Sealed,
    Readonly, Const, Partial, Async, Extern, Unsafe, New,
];

/// Map a transformed element name to a syntax category for highlighting.
pub fn syntax_category(element: &str) -> SyntaxCategory {
    output::spec(element)
        .map(|spec| spec.syntax)
        .unwrap_or(SyntaxCategory::Default)
}
