//! Per-kind transformations for Java.
//!
//! Each function is a `Rule::Custom` target — `rule(JavaKind) -> Rule`
//! references these by name. A transformation owns the renaming,
//! child reshaping, and `TransformAction` choice for kinds that don't
//! fit a shared `Rule` variant.
//!
//! Simple flattens / pure renames / `extract op + rename` patterns
//! live as data in `rule()` (see `rules.rs`), not here.

use xot::{Xot, Node as XotNode};

use crate::transform::{TransformAction, helpers::*};
use crate::transform::generic_type::rewrite_generic_type;
use crate::transform::operators::{extract_operator, is_prefix_form};

use super::input::JavaKind;
use super::output::TractorNode;
use super::output::TractorNode::{
    Call, Comment as CommentName, Else, Expression, Generic, Generics, If, Leading, Method, Name,
    Package, Prefix, Private, Protected, Public, Returns, Static, Final, Abstract, Synchronized,
    Volatile, Transient, Native, Strictfp, Super, Ternary, This, Trailing, Type, Unary, Void,
};

/// Kinds whose name happens to match our semantic vocabulary already
/// (`guard`, `pattern`, `super`, `this`, `throws`) or supertypes the
/// grammar emits but the transform never rewrites.
pub fn passthrough(_xot: &mut Xot, _node: XotNode) -> Result<TransformAction, xot::Error> {
    Ok(TransformAction::Continue)
}

/// `parenthesized_expression` — pure grammar grouping; drop the
/// wrapper before children are visited (children's parent context
/// becomes the enclosing block / class body).
pub fn skip(_xot: &mut Xot, _node: XotNode) -> Result<TransformAction, xot::Error> {
    Ok(TransformAction::Skip)
}

/// `expression_statement` — wrap value-producing statements in an
/// `<expression>` host (Principle #15). Java's
/// `expression_statement` only wraps expressions in statement
/// position; control-flow forms have their own statement kinds
/// (`if_statement`, etc.), so there's no inner-kind dispatch needed
/// — every Java `expression_statement` produces a value.
pub fn expression_statement(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_renamed(node, Expression);
    Ok(TransformAction::Continue)
}

/// `update_expression` — `++x`, `x++`, `--x`, `x--`. Tree-sitter uses
/// one kind for both prefix and postfix forms, distinguished only by
/// child order. Extract the operator into `<op>`, rename to `<unary>`,
/// and prepend `<prefix/>` when the source form was prefix so
/// `//unary[prefix][op[increment]]` matches `++x` cross-language
/// (parallels C#'s explicit `prefix_unary_expression` shape).
pub fn update_expression(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let was_prefix = is_prefix_form(xot, node);
    extract_operator(xot, node)?;
    xot.with_renamed(node, Unary);
    if was_prefix {
        xot.with_prepended_empty_element(node, Prefix)?;
    }
    Ok(TransformAction::Continue)
}

/// `<name>` field wrapper inserted by the builder for nodes with a
/// `field=name` attribute. Inline the single identifier child as text:
///   `<name><identifier>foo</identifier></name>`           → `<name>foo</name>`
///   `<name><type_identifier>Foo</type_identifier></name>` → `<name>Foo</name>`
///
/// Called from the dispatcher's wrapper branch, not from the rule
/// table — the node has no `kind=` attribute since it was synthesised
/// by the builder, not emitted by tree-sitter.
pub fn name_wrapper(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let children: Vec<_> = xot.children(node).collect();
    for child in &children {
        let child_kind = get_kind(xot, *child).and_then(|kind| kind.parse::<JavaKind>().ok());
        if !matches!(child_kind, Some(JavaKind::Identifier) | Some(JavaKind::TypeIdentifier)) {
            continue;
        }
        let text = match get_text_content(xot, *child) {
            Some(t) => t,
            None => continue,
        };
        xot.with_only_text(node, &text)?;
        return Ok(TransformAction::Continue);
    }
    Ok(TransformAction::Continue)
}

/// `line_comment` / `block_comment` — normalise both to `<comment>` and
/// run the shared trailing/leading/floating classifier with `//` line-
/// comment grouping (Principle #1 / #2).
pub fn comment(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_renamed(node, CommentName);
    static CLASSIFIER: crate::languages::comments::CommentClassifier =
        crate::languages::comments::CommentClassifier { line_prefixes: &["//"] };
    CLASSIFIER.classify_and_group(xot, node, Trailing, Leading)
}

/// `boolean_type` / `floating_point_type` / `integral_type` — primitive
/// type keywords. Render as `<type><name>int</name></type>` for uniform
/// querying.
pub fn primitive_type(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_renamed(node, Type);
    wrap_text_in_name(xot, node)?;
    Ok(TransformAction::Continue)
}

/// `type_identifier` — same `<type><name>` shape as primitives, but
/// separated because tree-sitter uses this kind specifically for type
/// references.
pub fn type_identifier(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_renamed(node, Type);
    wrap_text_in_name(xot, node)?;
    Ok(TransformAction::Continue)
}

/// `void_type` — gets the same `<type><name>void</name></type>` shape
/// as any other type PLUS a `<void/>` marker — void is the one
/// primitive that's special enough to warrant a shortcut predicate
/// (`//type[void]`) because it's return-only and conceptually "no
/// value", not a regular data type. The marker is *additional*, not a
/// replacement for `<name>`: JSON keeps `"name": "void"` and adds
/// `"void": true` as the shortcut flag.
pub fn void_type(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_renamed(node, Type);
    wrap_text_in_name(xot, node)?;
    xot.with_prepended_empty_element(node, Void)?;
    Ok(TransformAction::Continue)
}

/// `identifier` — Java is type-stable: `type_identifier` is its own
/// grammar kind, so a bare `identifier` is always a name (definition
/// or reference). Rename to `<name>`.
pub fn identifier(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_renamed(node, Name);
    Ok(TransformAction::Continue)
}

/// `generic_type` — apply the cross-language pattern:
///   `generic_type(<type_identifier>Foo</type_identifier>, type_arguments)`
///     → `<type><generic/>Foo <type field="arguments">Bar</type>...</type>`
pub fn generic_type(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    rewrite_generic_type(xot, node, &["type_identifier", "scoped_type_identifier"])?;
    Ok(TransformAction::Continue)
}

/// `if_statement` — Java's tree-sitter doesn't emit an `else_clause`
/// wrapper: the `alternative` field of an if_statement points directly
/// at the nested if_statement (for `else if`) or a block (for final
/// `else {…}`). Wrap the alternative in `<else>` surgically so the
/// shared conditional-shape post-transform can collapse the chain
/// uniformly.
pub fn if_statement(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_wrapped_field_child(node, "alternative", Else)?
        .with_renamed(node, If);
    Ok(TransformAction::Continue)
}

/// `ternary_expression` — `a ? b : c`. Wrap `alternative` field child
/// in `<else>` so the shared conditional-shape post-transform can
/// collapse the chain uniformly.
pub fn ternary_expression(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    xot.with_wrapped_field_child(node, "alternative", Else)?
        .with_renamed(node, Ternary);
    Ok(TransformAction::Continue)
}

/// `type_parameter` — tree-sitter puts the parameter's name as a
/// sibling `type_identifier`; bounds follow as sibling `type_bound`
/// elements. Replace the identifier with a `<name>TEXT</name>` child
/// so the eventual shape is
///   `<generic><name>T</name><bound>...</bound></generic>`,
/// not the over-wrapped `<generic><type><name>T</name></type>...`.
pub fn type_parameter(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    replace_identifier_with_name_child(xot, node, &["type_identifier"])?;
    xot.with_renamed(node, Generic);
    Ok(TransformAction::Continue)
}

/// `type_parameters` — generic parameter list. Distribute `field=
/// "generics"` to each child, rename to `<generics>`, then flatten so
/// the children land directly under the enclosing declaration.
pub fn type_parameters(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    distribute_field_to_children(xot, node, "generics");
    xot.with_renamed(node, Generics);
    Ok(TransformAction::Flatten)
}

/// `modifiers` — Java wraps modifiers in a `<modifiers>` element
/// containing space-separated keyword tokens. Lift each keyword to an
/// empty marker in source order, then flatten the wrapper so the
/// literal `public abstract static` text survives as dangling siblings
/// — the enclosing declaration's XPath string-value then contains the
/// actual source keywords. Also inserts `<package/>` if no access
/// modifier was found (Principle #9 — mutually-exclusive access is
/// exhaustive).
pub fn modifiers(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let mut markers: Vec<TractorNode> = match get_text_content(xot, node) {
        Some(text) => text
            .split_whitespace()
            .filter_map(parse_modifier)
            .filter(|marker| is_known_modifier(*marker))
            .collect(),
        None => Vec::new(),
    };
    let has_access = markers.iter().copied().any(is_access_modifier);
    if !has_access {
        markers.insert(0, Package);
    }

    for marker in &markers {
        xot.with_inserted_empty_before(node, marker)?;
    }

    Ok(TransformAction::Flatten)
}

/// `explicit_constructor_invocation` — `this(args)` / `super(args)` at
/// the start of a constructor body. Render as `<call>` with a
/// `<this/>` or `<super/>` marker so `//call[this]` / `//call[super]`
/// work uniformly with other call sites.
pub fn explicit_constructor_invocation(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    let children: Vec<_> = xot.children(node).collect();
    for child in children {
        let tag = match get_kind(xot, child).and_then(|kind| kind.parse::<JavaKind>().ok()) {
            Some(JavaKind::This) => This,
            Some(JavaKind::Super) => Super,
            _ => continue,
        };
        let text = get_text_content(xot, child).unwrap_or_default();
        xot.detach(child)?;
        let marker = prepend_empty_element(xot, node, tag)?;
        xot.with_inserted_text_after(marker, &text)?;
        break;
    }
    xot.with_renamed(node, Call);
    Ok(TransformAction::Continue)
}

/// `method_declaration` — combines the shared `default-access-then-
/// rename` shape with Java's method-specific return-type wrapping.
/// Java's grammar tags the method return type as `field="type"` (the
/// same field name used on parameters), so the builder can't wrap it
/// generically; do it here.
pub fn method_declaration(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    if let Some(marker) = default_access_for_declaration(xot, node) {
        xot.with_prepended_empty_element(node, marker)?;
    }
    wrap_method_return_type(xot, node)?;
    xot.with_renamed(node, Method);
    Ok(TransformAction::Continue)
}

// ---------------------------------------------------------------------
// Default-access resolver consumed by `Rule::DefaultAccessThenRename`.
// 5 of Java's 6 declaration kinds (class / interface / enum /
// constructor / field) use this directly via the rule variant; the
// 6th (method) calls it from a Custom handler that adds method-
// specific return-type wrapping.
// ---------------------------------------------------------------------

/// Returns `Some(marker_name)` when the declaration node has no
/// modifiers wrapper child (no source modifiers were written) and
/// should receive a default access marker; `None` when modifiers are
/// already present (the `modifiers` handler inserts the implicit
/// `<package/>` itself when no access keyword appears in the wrapper).
///
/// Default depends on enclosing scope: members of an interface
/// declaration are implicitly public (Java spec §9.4); top-level
/// types and class members default to package access.
pub fn default_access_for_declaration(
    xot: &Xot,
    node: XotNode,
) -> Option<TractorNode> {
    if has_modifiers_child(xot, node) {
        return None;
    }
    if is_inside_interface(xot, node) {
        Some(Public)
    } else {
        Some(Package)
    }
}

// ---------------------------------------------------------------------
// Local helpers used by handlers above.
// ---------------------------------------------------------------------

fn parse_modifier(text: &str) -> Option<TractorNode> {
    text.parse().ok()
}

fn is_access_modifier(name: TractorNode) -> bool {
    matches!(name, Public | Private | Protected)
}

fn is_known_modifier(name: TractorNode) -> bool {
    matches!(
        name,
        Public | Private | Protected
            | Static | Final | Abstract | Synchronized
            | Volatile | Transient | Native | Strictfp
    )
}

fn has_modifiers_child(xot: &Xot, node: XotNode) -> bool {
    for child in xot.children(node) {
        if matches!(
            get_kind(xot, child).and_then(|kind| kind.parse::<JavaKind>().ok()),
            Some(JavaKind::Modifiers)
        ) {
            return true;
        }
    }
    false
}

/// Walk up from `node` looking for an enclosing `interface_declaration`.
/// Stops at the first class/enum/record (which would override the default).
fn is_inside_interface(xot: &Xot, node: XotNode) -> bool {
    let mut current = get_parent(xot, node);
    while let Some(parent) = current {
        if let Some(java_kind) = get_kind(xot, parent).and_then(|kind| kind.parse::<JavaKind>().ok()) {
            match java_kind {
                JavaKind::InterfaceDeclaration => return true,
                JavaKind::ClassDeclaration
                | JavaKind::EnumDeclaration
                | JavaKind::RecordDeclaration => return false,
                // class_body / interface_body / etc. are transparent
                _ => {}
            }
        }
        current = get_parent(xot, parent);
    }
    false
}

/// Wrap a method's return type (the child with `field="type"`) in a
/// `<returns>` element so it's symmetric with C# / Rust / TS. Java's
/// tree-sitter grammar uses the ambiguous field name `type` for both
/// return types and parameter types, so this can't be done generically
/// by the builder.
fn wrap_method_return_type(xot: &mut Xot, method: XotNode) -> Result<(), xot::Error> {
    let children: Vec<XotNode> = xot.children(method).collect();
    for child in children {
        if xot.element(child).is_none() {
            continue;
        }
        if get_attr(xot, child, "field").as_deref() != Some("type") {
            continue;
        }
        let returns_name = get_name(xot, Returns);
        let wrapper = xot.new_element(returns_name);
        xot.with_source_location_from(wrapper, child)
            .with_attr(wrapper, "field", "returns")
            .with_wrap_child(child, wrapper)?
            .with_removed_attr(child, "field");
        break;
    }
    Ok(())
}
