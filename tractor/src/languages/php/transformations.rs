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
    Arrow, Comment as CommentName, Foreach, Function, Global, Index, Leading, Member,
    Object, Pair, Prefix, Primitive, Private, Property, Protected, Public, Static as StaticMarker,
    String as PhpString, Trailing, Type, Unary, Value, Variable,
};

/// `class_constant_access_expression` — `Foo::BAR`. Tree-sitter
/// emits two `<name>` siblings (class name + constant name) with no
/// `field=` attributes. Wrap them by position: first → `<object>`,
/// second → `<property>`. Mirrors C# member-access (iter 178).
/// Renames to `<member>` and adds the `<static/>` marker.
///
/// Iter 342: marker name `<constant/>` → `<static/>` per
/// Principle #5 cross-language alignment with Ruby's `<member[static]>`
/// for `::` scope-resolution access. PHP's own `<call><static/>`
/// for static method calls already uses `<static/>`; this brings
/// PHP `Foo::BAR` (constant access) and Ruby `Foo::Bar`
/// (constant/module access) into uniform shape.
pub fn class_constant_access(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    let elem_children: Vec<XotNode> = xot.children(node)
        .filter(|&c| xot.element(c).is_some())
        .collect();
    // Find the two `<name>` children (skip pre-existing markers).
    let names: Vec<XotNode> = elem_children.iter()
        .copied()
        .filter(|&c| get_element_name(xot, c).as_deref() == Some("name"))
        .collect();
    if names.len() == 2 {
        // First name = class (object); second = constant (property).
        let object_id = xot.add_name(Object.as_str());
        let object_node = xot.new_element(object_id);
        xot.with_source_location_from(object_node, names[0])
            .with_wrap_child(names[0], object_node)?;
        let property_id = xot.add_name(Property.as_str());
        let property_node = xot.new_element(property_id);
        xot.with_source_location_from(property_node, names[1])
            .with_wrap_child(names[1], property_node)?;
    }
    xot.with_renamed(node, Member)
        .with_prepended_marker(node, StaticMarker)?;
    Ok(TransformAction::Continue)
}

/// `subscript_expression` — `$arr[$key]`. Tree-sitter emits two
/// element children (the array operand and the index) without
/// `field=` attributes. Both are typically `<variable>` (or
/// other expression elements), so the JSON serializer collapses
/// them: first becomes singleton `variable: {...}`, the second
/// overflows to `children`.
///
/// Wrap the FIRST element child (the array operand) in `<object>`
/// — matching member-access vocabulary (the array IS the object
/// being indexed). The index stays as a bare sibling. Mirrors Go
/// iter 284.
pub fn subscript_expression(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    let first_elem = xot.children(node)
        .find(|&c| xot.element(c).is_some());
    if let Some(operand) = first_elem {
        let object_id = xot.add_name(Object.as_str());
        let object_node = xot.new_element(object_id);
        xot.with_source_location_from(object_node, operand)
            .with_wrap_child(operand, object_node)?;
    }
    xot.with_renamed(node, Index);
    Ok(TransformAction::Continue)
}

/// `pair` — PHP foreach `$key => $item` and array `[$k => $v]`.
/// Tree-sitter emits two `<variable>` (or other expression)
/// siblings without `field=` attributes. Both become `<variable>`
/// after rename, colliding on the JSON `variable` key.
///
/// Wrap the SECOND element child (the value-side) in `<value>`.
/// First (key-side) stays bare. Mirrors iter 285's subscript fix
/// pattern (slot-wrap one of two role-mixed siblings).
pub fn pair(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let elem_children: Vec<XotNode> = xot.children(node)
        .filter(|&c| xot.element(c).is_some())
        .collect();
    if elem_children.len() >= 2 {
        let value_target = elem_children[1];
        let value_id = xot.add_name(Value.as_str());
        let value_node = xot.new_element(value_id);
        xot.with_source_location_from(value_node, value_target)
            .with_wrap_child(value_target, value_node)?;
    }
    xot.with_renamed(node, Pair);
    Ok(TransformAction::Continue)
}

/// `foreach_statement` — `foreach ($arr as $x) {}` or
/// `foreach ($arr as $key => $val) {}`. Tree-sitter emits the
/// iterable (`$arr`) and the binding (`$x` or `<pair>`) as
/// positional element children.
///
/// Iter 349: wrap the iterable in `<right>` and the binding in
/// `<left>` per Principle #5 — same `<left>`/`<right>` shape that
/// TS/C#/Python use for `for x in items`. Closes the cold-read
/// HIGH finding "PHP foreach value/ is the iterable, not the
/// element" — `<value>` was misleading because it implies an
/// "actual value" elsewhere (default, init, hash); the iterable
/// is semantically a `<right>` operand of the iteration binding.
pub fn foreach_statement(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    use crate::transform::helpers::XotWithExt;
    // Element children in source order: [iterable, binding, body].
    // Body is always last (it's the foreach `{}` block).
    let elem_children: Vec<XotNode> = xot
        .children(node)
        .filter(|&c| xot.element(c).is_some())
        .collect();
    // Wrap the FIRST element child (iterable) in `<right>`.
    if let Some(&iterable) = elem_children.first() {
        let right_id = xot.add_name("right");
        let right_node = xot.new_element(right_id);
        xot.with_source_location_from(right_node, iterable)
            .with_wrap_child(iterable, right_node)?;
    }
    // Wrap the SECOND element child (binding) in `<left>`, unless
    // it's the body wrapper or absent. Body of foreach renames to
    // <body>; the binding is everything else between iterable and
    // body. Foreach with empty body would have just iterable+body,
    // skip the wrap in that case.
    if elem_children.len() >= 3 {
        let binding = elem_children[1];
        let binding_name = get_element_name(xot, binding);
        if binding_name.as_deref() != Some("body") {
            let left_id = xot.add_name("left");
            let left_node = xot.new_element(left_id);
            xot.with_source_location_from(left_node, binding)
                .with_wrap_child(binding, left_node)?;
        }
    }
    xot.with_renamed(node, Foreach);
    Ok(TransformAction::Continue)
}

/// Pure-grammar wrappers (parenthesized expressions, etc.) — drop
/// the wrapper, promote children to parent.
pub fn skip(_xot: &mut Xot, _node: XotNode) -> Result<TransformAction, xot::Error> {
    Ok(TransformAction::Skip)
}

/// `arrow_function` — `fn($x) => expr`. PHP arrow functions are
/// syntactically always single-expression (no block bodies — that's
/// `function ($x) { ... }`). Re-tag the `<body>` wrapper to `<value>`
/// so `wrap_expression_positions` wraps the body's content in
/// `<expression>` host (Principle #15). Mirrors iter 161/162/167.
pub fn arrow_function(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let body_child = xot.children(node)
        .filter(|&c| xot.element(c).is_some())
        .find(|&c| get_element_name(xot, c).as_deref() == Some("body"));
    if let Some(body) = body_child {
        let value_id = xot.add_name("value");
        if let Some(elem) = xot.element_mut(body) {
            elem.set_name(value_id);
        }
    }
    xot.with_renamed(node, Function)
        .with_prepended_marker(node, Arrow)?;
    Ok(TransformAction::Continue)
}

/// `enum_declaration` — `enum Status: string { ... }` (backed enum).
/// Tree-sitter emits the storage type as a `primitive_type` child of
/// the enum_declaration. Tag it with `[underlying]` so cross-language
/// `//enum/type[underlying]` works uniformly with C# (iter 125).
/// Plain (non-backed) enums have no primitive_type child and are
/// unaffected.
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
///
/// Empty modifier nodes (tree-sitter-php emits an empty
/// `<static_modifier>` for non-static fields as a "presence absent"
/// marker) are detached: absence of the corresponding marker
/// (`<static/>`, etc.) means the property doesn't have it. Surfaced
/// by iter 315's grammar-suffix migration attempt.
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
    // Empty modifier node — detach (no semantic content; absence of
    // the marker on the parent is the signal).
    xot.detach(node)?;
    Ok(TransformAction::Done)
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
/// Renames to `<extends>` and adds `list="extends"` so JSON output
/// is consistently an array regardless of count.
pub fn base_clause(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    use super::output::TractorNode::Extends;
    xot.with_renamed(node, Extends)
        .with_attr(node, "list", "extends");
    Ok(TransformAction::Continue)
}

/// `class_interface_clause` — `implements A, B, C`. Wrap each
/// interface name in its own `<implements>` sibling with
/// `list="implements"` so JSON serializers reconstruct as an
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
