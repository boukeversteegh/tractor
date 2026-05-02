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
use crate::transform::operators::{extract_operator, is_prefix_form};

use super::input::PhpKind;
use super::output::TractorNode;
use super::output::TractorNode::{
    Comment as CommentName, Global, Leading, Prefix, Primitive, Private, Protected, Public,
    String as PhpString, Trailing, Type, Unary, Variable,
};

/// Pure-grammar wrappers (parenthesized expressions, etc.) — drop
/// the wrapper, promote children to parent.
pub fn skip(_xot: &mut Xot, _node: XotNode) -> Result<TransformAction, xot::Error> {
    Ok(TransformAction::Skip)
}

/// `enum_declaration` — `enum Status: string { ... }` (backed enum).
/// Tree-sitter emits the storage type as a `primitive_type` child of
/// the enum_declaration. Tag it with `[underlying]` + `field="underlying"`
/// so cross-language `//enum/type[underlying]` works uniformly with C#
/// (iter 125). Plain (non-backed) enums have no primitive_type child
/// and are unaffected.
pub fn enum_declaration(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    use super::output::TractorNode::Underlying;
    let elem_children: Vec<XotNode> = xot.children(node)
        .filter(|&c| xot.element(c).is_some())
        .collect();
    for child in elem_children {
        if get_kind(xot, child).as_deref() == Some("primitive_type") {
            xot.with_appended_marker(child, Underlying)?;
            break;
        }
    }
    xot.with_renamed(node, super::output::TractorNode::Enum);
    Ok(TransformAction::Continue)
}

/// `primitive_type` — render as `<type[primitive]><name>string</name></type>`
/// matching the cross-language type shape (`type/name` is the broad
/// query path; `[primitive]` distinguishes built-in from user types).
pub fn primitive_type(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    xot.with_renamed(node, Type);
    wrap_text_in_name(xot, node)?;
    xot.with_appended_marker(node, Primitive)?;
    Ok(TransformAction::Continue)
}

/// `expression_statement` — wrap value-producing statements in an
/// `<expression>` host (Principle #15). PHP's `expression_statement`
/// only wraps value-producing expressions in statement context, so
/// no inner-kind dispatch needed.
pub fn expression_statement(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_renamed(node, super::output::TractorNode::Expression);
    Ok(TransformAction::Continue)
}

/// `update_expression` — `++$x`, `$x++`, `--$x`, `$x--`. Tree-sitter
/// uses one kind for both prefix and postfix forms, distinguished only
/// by child order. Extract the operator into `<op>`, rename to
/// `<unary>`, and prepend `<prefix/>` when the source form was prefix
/// so `//unary[prefix][op[increment]]` matches `++$x` cross-language
/// (parallels C#'s explicit `prefix_unary_expression` shape).
pub fn update_expression(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let was_prefix = is_prefix_form(xot, node);
    extract_operator(xot, node)?;
    xot.with_renamed(node, Unary);
    if was_prefix {
        xot.with_prepended_marker(node, Prefix)?;
    }
    Ok(TransformAction::Continue)
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
/// `abstract_modifier` / `readonly_modifier` / `var_modifier` —
/// text → marker conversion. The source keyword survives as a dangling
/// sibling so the enclosing declaration's XPath string-value still
/// contains it (Principle #7).
///
/// Deprecated PHP 4 `var $x` is equivalent to `public $x`; rewrite
/// the marker name so cross-language `//field[public]` queries find
/// both forms.
pub fn modifier(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    if let Some(text) = get_text_content(xot, node) {
        let text = text.trim().to_string();
        if !text.is_empty() {
            let marker_name = if text == "var" { "public" } else { text.as_str() };
            rename_to_marker(xot, node, marker_name)?;
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

/// `cast_type` — `(int)`/`(string)` cast prefix. Wrap the bare type
/// text in `<name>` so the shape matches other PHP type contexts
/// (Principle #14 — identifiers in `<name>`).
pub fn cast_type(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_renamed(node, Type);
    wrap_text_in_name(xot, node)?;
    Ok(TransformAction::Continue)
}

/// `base_clause` — `class Foo extends Bar` (PHP allows only one).
/// Renames to `<extends>` and adds `field="extends"` so JSON output
/// is consistently an array regardless of count.
pub fn base_clause(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    use super::output::TractorNode::Extends;
    xot.with_renamed(node, Extends)
        .with_attr(node, "list", "extends");
    Ok(TransformAction::Continue)
}

/// `class_interface_clause` — `implements A, B, C`. Wrap each
/// interface name in its own `<implements>` sibling with
/// `field="implements"` so JSON serializers reconstruct as an
/// `implements` array (Principle #12 — flat siblings + field attr).
pub fn class_interface_clause(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    use super::output::TractorNode::Implements;
    let elem_children: Vec<XotNode> = xot.children(node)
        .filter(|&c| xot.element(c).is_some())
        .collect();
    for child in elem_children {
        let impl_elt = xot.add_name(Implements.as_str());
        let impl_node = xot.new_element(impl_elt);
        xot.insert_before(child, impl_node)?;
        xot.detach(child)?;
        xot.append(impl_node, child)?;
        xot.with_attr(impl_node, "list", "implements");
    }
    Ok(TransformAction::Flatten)
}

/// `global_declaration` — `global $x;`. Strip the bare `global`
/// keyword text. Within each variable_name child, lift the `<name>`
/// out so the shape is `<variable[global]><name>x</name>...</variable>`
/// rather than `<variable[global]><variable>{$, name=x}</variable></variable>`.
pub fn global_declaration(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    // Strip the bare `global` keyword text.
    for child in xot.children(node).collect::<Vec<_>>() {
        if let Some(text) = xot.text_str(child) {
            if text.trim() == "global" || text.trim() == ";" {
                xot.detach(child)?;
            }
        }
    }
    // For each variable_name child, lift its inner content (the
    // `$` text + `<name>` element) up as siblings, then detach the
    // variable_name wrapper.
    for child in xot.children(node).collect::<Vec<_>>() {
        if !matches!(
            get_kind(xot, child).and_then(|k| k.parse::<PhpKind>().ok()),
            Some(PhpKind::VariableName)
        ) {
            continue;
        }
        let inner: Vec<_> = xot.children(child).collect();
        for c in inner {
            xot.detach(c)?;
            xot.insert_before(child, c)?;
        }
        xot.detach(child)?;
    }
    xot.with_renamed(node, Variable)
        .with_prepended_marker(node, Global)?;
    Ok(TransformAction::Continue)
}

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
