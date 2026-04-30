//! Per-kind transformations for PHP.
//!
//! Each function is a `Rule::Custom` target — `rule(PhpKind) -> Rule`
//! references these by name. A transformation owns the renaming,
//! child reshaping, and `TransformAction` choice for kinds that don't
//! fit a shared `Rule` variant.
//!
//! Simple flattens / pure renames / `extract op + rename` patterns
//! live as data in `rule()` (see `rules.rs`), not here.

use xot::{Xot, Node as XotNode};

use crate::transform::{TransformAction, helpers::*};

use super::input::PhpKind;
use super::output::TractorNode;
use super::output::TractorNode::{
    Comment as CommentName, Leading, Private, Protected, Public, String as PhpString, Trailing,
};

/// Kinds whose name happens to match our semantic vocabulary already
/// (`name`, `pair`) or grammar supertypes.
pub fn passthrough(_xot: &mut Xot, _node: XotNode) -> Result<TransformAction, xot::Error> {
    Ok(TransformAction::Continue)
}

/// `expression_statement` — drop the wrapper before children are
/// visited so children's parent context becomes the enclosing block
/// rather than the statement wrapper.
pub fn skip(_xot: &mut Xot, _node: XotNode) -> Result<TransformAction, xot::Error> {
    Ok(TransformAction::Skip)
}

/// `<name>` field wrapper inserted by the builder for nodes with a
/// `field=name` attribute. PHP's logic is more involved than other
/// languages because:
///   - `variable_name` (`$foo`) is the bound name in any field slot,
///     so the wrapper should inline it as text.
///   - `qualified_name` / `namespace_name` flatten into multiple
///     segments + `\\` separators; flattening the outer `<name>`
///     hoists those segments to the enclosing namespace / use.
///   - Already-inlined `<name>` / `<variable>` get re-collapsed too.
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
        let ts_kind = get_kind(xot, child).and_then(|kind| kind.parse::<PhpKind>().ok());
        let el_name = get_element_name(xot, child);
        // Single namespace_name / qualified_name child: that child
        // will flatten into segments + separators. Flattening the
        // outer wrapper now hoists segments to the enclosing
        // namespace/use, where each becomes a direct `<name>` sibling.
        if matches!(
            ts_kind,
            Some(PhpKind::NamespaceName | PhpKind::QualifiedName),
        ) {
            return Ok(TransformAction::Flatten);
        }
        let inlineable = matches!(
            ts_kind,
            Some(PhpKind::Name | PhpKind::VariableName),
        ) || matches!(
            el_name.and_then(|name| name.parse::<TractorNode>().ok()),
            Some(TractorNode::Name | TractorNode::Variable),
        );
        if inlineable {
            let text = descendant_text(xot, child);
            let trimmed = text.trim().to_string();
            if !trimmed.is_empty() {
                xot.with_only_text(node, &trimmed)?;
                return Ok(TransformAction::Done);
            }
        }
    } else if element_children.len() > 1 {
        // Multiple element children — qualified name flattened into
        // segments + separators. Flatten the outer `<name>` so each
        // segment becomes a direct child of the enclosing node.
        return Ok(TransformAction::Flatten);
    }
    Ok(TransformAction::Continue)
}

/// `comment` — PHP supports `//`, `#`, and `/* */` comments. Tree-
/// sitter emits all of them as a single `comment` kind; the shared
/// classifier handles trailing / leading / floating + line-comment
/// grouping. Both `//` and `#` count as line-comment prefixes — runs
/// of either family (or a mix on adjacent lines) merge into one
/// `<comment>`.
pub fn comment(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_renamed(node, CommentName);
    static CLASSIFIER: crate::languages::comments::CommentClassifier =
        crate::languages::comments::CommentClassifier { line_prefixes: &["//", "#"] };
    CLASSIFIER.classify_and_group(xot, node, Trailing, Leading)
}

/// `visibility_modifier` / `static_modifier` / `final_modifier` /
/// `abstract_modifier` / `readonly_modifier` — text → marker
/// conversion. The source keyword survives as a dangling sibling so
/// the enclosing declaration's XPath string-value still contains it
/// (Principle #7).
pub fn modifier(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    if let Some(text) = get_text_content(xot, node) {
        let text = text.trim().to_string();
        if !text.is_empty() {
            rename_to_marker(xot, node, &text)?;
            xot.with_inserted_text_after(node, &text)?;
            return Ok(TransformAction::Done);
        }
    }
    Ok(TransformAction::Continue)
}

/// `encapsed_string` — PHP interpolated string `"hello $name"` or
/// `"x {$obj->y}"`. Tree-sitter nests interpolated expressions
/// (`variable_name` / `member_access_expression` / …) directly inside
/// the string; every other language we support wraps these in an
/// `<interpolation>` element so the cross-language shape is uniform.
/// Wrap each real expression in `<interpolation>` so
/// `//string/interpolation/name` works cross-language. Skip
/// text-fragment kinds (`string_content` / `escape_sequence` /
/// `text_interpolation`) — those are literal string text.
pub fn encapsed_string(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let children: Vec<_> = xot.children(node)
        .filter(|&c| xot.element(c).is_some())
        .collect();
    for child in children {
        let ts_kind = get_kind(xot, child).and_then(|kind| kind.parse::<PhpKind>().ok());
        if matches!(
            ts_kind,
            Some(PhpKind::StringContent | PhpKind::EscapeSequence | PhpKind::TextInterpolation) | None,
        ) {
            continue;
        }
        let interp_name = xot.add_name("interpolation");
        let interp = xot.new_element(interp_name);
        xot.with_source_location_from(interp, child)
            .with_wrap_child(child, interp)?;
    }
    xot.with_renamed(node, PhpString);
    Ok(TransformAction::Continue)
}

// ---------------------------------------------------------------------
// Default-access resolver consumed by `Rule::DefaultAccessThenRename`.
// PHP class members default to public (PHP spec); 2 declaration kinds
// (method, property) use the variant. Class-level types don't have
// implicit access modifiers in PHP.
// ---------------------------------------------------------------------

/// Returns `Some(PUBLIC)` when the declaration node lacks any visibility
/// marker child; `None` when one is already present. Walk-order
/// caveat: the `visibility_modifier` child may still be raw (pre-
/// rename) or already converted to a `<public/>` / `<private/>` /
/// `<protected/>` marker, so check both forms.
pub fn default_access_for_declaration(
    xot: &Xot,
    node: XotNode,
) -> Option<TractorNode> {
    if has_visibility_marker(xot, node) {
        None
    } else {
        Some(Public)
    }
}

// ---------------------------------------------------------------------
// Local helper used by `default_access_for_declaration`.
// ---------------------------------------------------------------------

fn has_visibility_marker(xot: &Xot, node: XotNode) -> bool {
    for child in xot.children(node) {
        if xot.element(child).is_none() { continue; }
        if get_kind(xot, child).and_then(|kind| kind.parse::<PhpKind>().ok())
            == Some(PhpKind::VisibilityModifier) {
            return true;
        }
        if let Some(name) = get_element_name(xot, child) {
            if matches!(name.parse::<TractorNode>().ok(), Some(Public | Private | Protected)) {
                return true;
            }
        }
    }
    false
}
