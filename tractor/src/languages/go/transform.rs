//! Go transform logic — thin dispatcher driven by `semantic::rule`.
//!
//! The per-kind logic is split:
//!   - Pure rename / flatten / shared compositions live as data in
//!     [`super::semantic::rule`].
//!   - Language-specific custom logic lives as named functions in
//!     [`super::handlers`].
//!
//! This file's job is just to look up the kind and execute its rule.

use xot::{Xot, Node as XotNode};
use crate::transform::{TransformAction, helpers::*};
use crate::transform::operators::is_operator_marker;
use crate::output::syntax_highlight::SyntaxCategory;

use super::kind::GoKind;

/// Transform a Go AST node.
///
/// Dispatch is split in two:
///   1. If the node carries a `kind` attribute (set by the builder
///      from the original tree-sitter kind), look it up in `GoKind`,
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

    // Unknown kinds (parse errors, synthetic nodes) keep their kind
    // name unchanged — same behavior as the old `_` arm fallback when
    // `apply_rename` returned `None`.
    let kind = match GoKind::from_str(&kind_str) {
        Some(k) => k,
        None => return Ok(TransformAction::Continue),
    };

    crate::languages::rule::dispatch(xot, node, super::semantic::rule(kind))
}

/// If `node` contains a single identifier child, replace the node's
/// children with that identifier's text. Used to flatten builder-
/// created wrappers like `<name><identifier>foo</identifier></name>`
/// → `<name>foo</name>`.
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
