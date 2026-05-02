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

use super::input::CsKind;
use super::output::TractorNode::{
    self, Accessor, Await, Base, Call, Else, Expression, File, Generic, If, Instance, Internal,
    Leading, Member, Name, Namespace, NonNull, Nullable, Optional, Private, Public, Protected,
    String as CsString, Ternary, This, Trailing, Type, Unary, Variable,
};

/// `file_scoped_namespace_declaration` — `namespace Foo;`. Rename
/// to `<namespace>` and add a `<file/>` marker. The post_transform
/// pass `unify_file_scoped_namespace` walks for these and folds
/// the trailing siblings under `<unit>` into a `<body>` child, so
/// both forms (block-scoped and file-scoped) share the same shape.
/// Closes todo/34.
/// `base_list` — C# `: A, B, C` after a class/struct/interface
/// declaration. C# uses `:` (no `extends`/`implements` keyword), so
/// per Principle #18 we name the relationship after the most
/// cross-language idiomatic operator name: `<extends>`. Per
/// Principle #12 (no list containers), each entry becomes its own
/// `<extends>` sibling — accepting the syntactic ambiguity that
/// we can't tell a base class from an interface at this level.
pub fn base_list(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    use super::output::TractorNode::Extends;
    let elem_children: Vec<XotNode> = xot.children(node)
        .filter(|&c| xot.element(c).is_some())
        .collect();
    for child in elem_children {
        let extends_elt = xot.add_name(Extends.as_str());
        let extends_node = xot.new_element(extends_elt);
        xot.insert_before(child, extends_node)?;
        xot.detach(child)?;
        xot.append(extends_node, child)?;
    }
    Ok(TransformAction::Flatten)
}

/// `constructor_initializer` — `: this(args)` / `: base(args)` in a
/// constructor declaration. Renames to `<call>` with a `[this]` or
/// `[base]` marker — matches Java's `<call[super]>` / `<call[this]>`
/// shape for the parallel `super(...)` / `this(...)` construct
/// inside Java constructors. Strips the bare `: this(` / `: base(`
/// text leaks (the markers carry the keyword).
pub fn constructor_initializer(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    let mut is_base = false;
    for child in xot.children(node).collect::<Vec<_>>() {
        if let Some(text) = xot.text_str(child) {
            if text.contains("base") {
                is_base = true;
            }
        }
    }
    for child in xot.children(node).collect::<Vec<_>>() {
        if xot.text_str(child).is_some() {
            xot.detach(child)?;
        }
    }
    xot.with_renamed(node, Call);
    let marker = if is_base { Base } else { This };
    xot.with_prepended_marker(node, marker)?;
    Ok(TransformAction::Continue)
}

pub fn file_scoped_namespace(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    xot.with_renamed(node, Namespace)
        .with_appended_marker(node, File)?;
    Ok(TransformAction::Continue)
}

/// `conditional_access_expression` — `Root.MaybeProperty?.Property`.
/// Per Principle #15 (markers on stable hosts), the conditional form
/// should be isomorphic to regular member access plus an
/// `<optional/>` marker. Pre-iter-57 shape was non-isomorphic: the
/// lhs sat under `<condition>/<expression>/<member[instance]>` while
/// regular member access had no condition wrapper, and the inner
/// `member_binding_expression` re-emitted `<member[optional]>`.
///
/// Target shape:
///   member[instance and optional]/
///     ├─ <member[instance]>...lhs...</member>
///     ├─ "?"   (separator)
///     ├─ "."   (separator, lifted from inner)
///     └─ <name>Property</name>   (lifted from inner)
///
/// Implementation:
/// 1. Strip the `<condition>` field-wrapper from the lhs (its
///    contents become a direct child).
/// 2. Flatten the inner `member_binding_expression`: lift its
///    children (the `.` text and the property name) up to be direct
///    children of `node`.
/// 3. Rename to `<member>` and add `<instance/>` + `<optional/>` markers.
pub fn conditional_access_expression(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    // Step 1: unwrap the <condition> field-wrapper (if present).
    let children: Vec<XotNode> = xot.children(node).collect();
    for child in &children {
        if get_attr(xot, *child, "field").as_deref() == Some("condition") {
            // Move all children of this wrapper out into `node`,
            // before the wrapper, then detach the wrapper.
            let inner: Vec<XotNode> = xot.children(*child).collect();
            for inner_child in inner {
                xot.detach(inner_child)?;
                xot.insert_before(*child, inner_child)?;
            }
            xot.detach(*child)?;
            break;
        }
    }

    // Step 2: locate inner member_binding_expression and flatten it.
    let children: Vec<XotNode> = xot.children(node).collect();
    let mut inner_binding: Option<XotNode> = None;
    for child in &children {
        if get_kind(xot, *child).as_deref() == Some("member_binding_expression") {
            inner_binding = Some(*child);
            break;
        }
    }
    if let Some(binding) = inner_binding {
        let lifted: Vec<XotNode> = xot.children(binding).collect();
        for c in lifted {
            xot.detach(c)?;
            xot.insert_before(binding, c)?;
        }
        xot.detach(binding)?;
    }

    // Step 3: rename to <member>, add <instance/> and <optional/>.
    xot.with_renamed(node, Member)
        .with_prepended_marker(node, Optional)?
        .with_prepended_marker(node, Instance)?;
    Ok(TransformAction::Continue)
}

/// `await_expression` — `await foo()`. C#'s `await` is prefix, so the
/// marker leads the operand. Promote to `<expression>` host with a
/// leading `<await/>` marker. See [Principle #15: Stable Expression
/// Hosts].
///
/// When the parent is already `<expression>` (e.g. from
/// `expression_statement`'s rename), avoid double-wrapping: lift
/// the `<await/>` marker onto the parent and flatten this node so
/// its children become direct siblings of the marker. Catches the
/// `<expression>/<expression[await]>` shape flagged by
/// `tree_invariants::no_repeated_parent_child_name`.
pub fn await_expression(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let parent_is_expression = get_parent(xot, node)
        .and_then(|p| get_element_name(xot, p))
        .as_deref()
        == Some("expression");
    if parent_is_expression {
        let parent = get_parent(xot, node).expect("parent_is_expression checked above");
        xot.with_prepended_marker_from(parent, Await, node)?;
        return Ok(TransformAction::Flatten);
    }
    xot.with_renamed(node, Expression)
        .with_prepended_marker(node, Await)?;
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
        let child_kind = get_kind(xot, child).and_then(|kind| kind.parse::<CsKind>().ok());
        let is_identifier = matches!(
            child_kind,
            Some(CsKind::Identifier)
        );
        let is_inlined_name = get_element_name(xot, child).as_deref() == Some("name");
        let is_qualified = matches!(
            child_kind,
            Some(CsKind::QualifiedName | CsKind::GenericName | CsKind::AliasQualifiedName)
        );
        if is_identifier || is_inlined_name {
            if let Some(text) = get_text_content(xot, child) {
                xot.with_only_text(node, &text)?;
                return Ok(TransformAction::Done);
            }
        } else if is_qualified {
            let text = descendant_text(xot, child);
            if !text.is_empty() {
                xot.with_only_text(node, &text)?;
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
    CLASSIFIER.classify_and_group(xot, node, Trailing, Leading)
}

/// `interpolated_string_expression` — rename to the shared `<string>`
/// so the cross-language shape holds: `<string>` wraps interpolation
/// children matching Python f-strings, TS templates, Ruby double-
/// quotes, and PHP.
pub fn interpolated_string_expression(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    xot.with_renamed(node, CsString);
    Ok(TransformAction::Continue)
}

/// `implicit_type` — C#'s `var` keyword in a type position. Render as
/// `<type><name>var</name></type>` for uniform querying.
pub fn implicit_type(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_renamed(node, Type);
    wrap_text_in_name(xot, node)?;
    Ok(TransformAction::Continue)
}

/// `predefined_type` — keywords like `int`, `string`. Same shape as
/// `implicit_type`.
pub fn predefined_type(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_renamed(node, Type);
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
            xot.with_renamed(node, accessor_kind);
            return Ok(TransformAction::Continue);
        }
    }
    xot.with_renamed(node, Accessor);
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
        if let Some(marker) = parse_known_modifier(&text) {
            xot.with_marker(node, marker)?
                .with_inserted_text_after(node, &text)?;
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
        if matches!(
            get_kind(xot, child).and_then(|kind| kind.parse::<CsKind>().ok()),
            Some(CsKind::Identifier | CsKind::PredefinedType)
        ) {
            if let Some(type_text) = get_text_content(xot, child) {
                xot.with_renamed(node, Type)
                    .with_only_text(node, &type_text)?;
                let nullable_name = get_name(xot, Nullable);
                let nullable_el = xot.new_element(nullable_name);
                xot.append(node, nullable_el)?;
                return Ok(TransformAction::Done);
            }
        }
    }
    xot.with_renamed(node, Type);
    Ok(TransformAction::Continue)
}

/// `identifier` — context-dependent classification. Decide whether
/// the identifier names a binding or a type reference based on parent
/// kind and sibling shape; rename accordingly. If classified as a
/// type reference, wrap its text in a `<name>` so `//type[name='Foo']`
/// matches uniformly across declaration and reference sites.
pub fn identifier(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let classification = classify_identifier(xot, node);
    xot.with_renamed(node, classification);
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

    xot.with_renamed(node, Type);

    let generic_name_id = get_name(xot, Generic);
    let generic_el = xot.new_element(generic_name_id);
    xot.prepend(node, generic_el)?;

    if !type_name.is_empty() {
        let name_id = get_name(xot, Name);
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
    xot.with_wrapped_field_child(node, "alternative", Else)?
        .with_renamed(node, Ternary);
    Ok(TransformAction::Continue)
}

/// `if_statement` — C#'s tree-sitter doesn't emit an `else_clause`
/// wrapper; the `alternative` field of an if_statement points
/// directly at the nested if_statement (for `else if`) or a block
/// (for final `else {…}`). Wrap the alternative in `<else>` so the
/// shared conditional-shape post-transform can collapse the chain
/// uniformly.
pub fn if_statement(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_wrapped_field_child(node, "alternative", Else)?
        .with_renamed(node, If);
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
        xot.with_renamed(node, Variable);
        Ok(TransformAction::Continue)
    }
}

/// `postfix_unary_expression` — `x!`, `x++`. Same shape as
/// `unary_expression` (extract operator + rename to `<unary>`); kept
/// as a Custom rather than `ExtractOpThenRename` because postfix
/// operators sit *after* the operand and we want a stable arm name
/// in case future C# additions need to differentiate.
/// `prefix_unary_expression` — `-x`, `!x`, `~x`, `++x`, `--x`. Extract
/// the operator into `<op>` (matching the binary / regular-unary
/// shape across languages). Only `++` and `--` have a postfix
/// counterpart, so only those carry the `<prefix/>` marker — for
/// `!x` / `-x` / `~x` the prefix-form is the only form, and the
/// marker would just be noise. This matches TS/Java/PHP's
/// `update_expression` (`<prefix/>` only on ++/--) vs.
/// `unary_expression` (no `<prefix/>` for !/-/~).
pub fn prefix_unary_expression(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    use crate::transform::helpers::get_text_children;
    let texts = get_text_children(xot, node);
    let is_increment = texts.iter().any(|t| {
        let trimmed = t.trim();
        trimmed == "++" || trimmed == "--"
    });
    extract_operator(xot, node)?;
    xot.with_renamed(node, Unary);
    if is_increment {
        xot.with_prepended_marker(node, super::output::TractorNode::Prefix)?;
    }
    Ok(TransformAction::Continue)
}

/// `postfix_unary_expression` — C#'s tree-sitter conflates two
/// semantically distinct constructs under one kind:
///
/// * `i++` / `i--`: arithmetic / mutation operators. Real unary
///   expressions; treated as `<unary>` with the operator extracted.
/// * `s!`: the **null-forgiving operator** (a.k.a. non-null
///   assertion). Suppresses a nullable-warning at the type-checker
///   level; doesn't change the value at runtime. Per Principle #15
///   (stable expression hosts) this is an annotational modifier on
///   the operand — the operand keeps its identity, the `!` becomes
///   a `<non_null/>` marker on an `<expression>` host. Mirrors the
///   TypeScript `non_null_expression` shape.
///
/// We dispatch on the operator text to pick the right shape.
pub fn postfix_unary_expression(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    let texts = get_text_children(xot, node);
    let is_null_forgiving = texts.iter().any(|t| t.trim() == "!");
    if is_null_forgiving {
        xot.with_renamed(node, Expression)
            .with_appended_marker(node, NonNull)?;
    } else {
        extract_operator(xot, node)?;
        xot.with_renamed(node, Unary);
    }
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
) -> Option<TractorNode> {
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
    matches!(kind.parse::<CsKind>().ok(), Some(
        CsKind::ClassDeclaration
            | CsKind::StructDeclaration
            | CsKind::InterfaceDeclaration
            | CsKind::EnumDeclaration
            | CsKind::RecordDeclaration
            | CsKind::NamespaceDeclaration
            | CsKind::MethodDeclaration
            | CsKind::ConstructorDeclaration
            | CsKind::PropertyDeclaration
            | CsKind::EnumMemberDeclaration
            | CsKind::Parameter
            | CsKind::VariableDeclarator
            | CsKind::TypeParameter
            | CsKind::Attribute
    ))
}

fn classify_identifier(xot: &Xot, node: XotNode) -> TractorNode {
    if let Some(field) = get_attr(xot, node, "field") {
        if field == "type" {
            return Type;
        }
    }

    let parent = match get_parent(xot, node) {
        Some(p) => p,
        None => return Type,
    };

    let parent_kind = get_kind(xot, parent).and_then(|kind| kind.parse::<CsKind>().ok());

    if get_element_name(xot, parent).as_deref() == Some("name") {
        if let Some(grandparent) = get_parent(xot, parent) {
            let grandparent_kind = get_kind(xot, grandparent).unwrap_or_default();
            if is_named_declaration(&grandparent_kind) {
                return Name;
            }
        }
    }

    let in_namespace = is_in_namespace_context(xot, node);
    if parent_kind == Some(CsKind::QualifiedName) && in_namespace {
        return Name;
    }

    let siblings = get_following_siblings(xot, node);
    let has_param_sibling = siblings.iter().any(|&s| {
        matches!(
            get_kind(xot, s).and_then(|kind| kind.parse::<CsKind>().ok()),
            Some(CsKind::ParameterList)
        ) || get_element_name(xot, s).as_deref() == Some("parameters")
    });

    match parent_kind {
        Some(CsKind::MethodDeclaration | CsKind::ConstructorDeclaration) if has_param_sibling => Name,
        Some(
            CsKind::ClassDeclaration
                | CsKind::StructDeclaration
                | CsKind::InterfaceDeclaration
                | CsKind::EnumDeclaration
                | CsKind::RecordDeclaration
                | CsKind::NamespaceDeclaration
                | CsKind::VariableDeclarator
                | CsKind::Parameter,
        ) => Name,
        Some(CsKind::GenericName | CsKind::TypeArgumentList | CsKind::TypeParameter | CsKind::BaseList) => Type,
        _ => Name,
    }
}

fn is_in_namespace_context(xot: &Xot, node: XotNode) -> bool {
    let mut current = get_parent(xot, node);
    while let Some(parent) = current {
        if let Some(kind) = get_kind(xot, parent).and_then(|kind| kind.parse::<CsKind>().ok()) {
            match kind {
                CsKind::NamespaceDeclaration => return true,
                CsKind::ClassDeclaration
                | CsKind::StructDeclaration
                | CsKind::InterfaceDeclaration
                | CsKind::EnumDeclaration
                | CsKind::RecordDeclaration => return false,
                _ => {}
            }
        }
        current = get_parent(xot, parent);
    }
    false
}

fn has_access_modifier_child(xot: &Xot, node: XotNode) -> bool {
    for child in xot.children(node) {
        if matches!(
            get_kind(xot, child).and_then(|kind| kind.parse::<CsKind>().ok()),
            Some(CsKind::Modifier)
        ) {
            if let Some(text) = get_text_content(xot, child) {
                if parse_access_modifier(text.trim()).is_some() {
                    return true;
                }
            }
        }
        if let Some(name) = get_element_name(xot, child) {
            if parse_access_modifier(&name).is_some() {
                return true;
            }
        }
    }
    false
}

fn parse_access_modifier(text: &str) -> Option<TractorNode> {
    text.parse()
        .ok()
        .filter(|name| matches!(name, Public | Private | Protected | Internal))
}

fn parse_known_modifier(text: &str) -> Option<TractorNode> {
    text.parse().ok().filter(|name| {
        super::transform::ACCESS_MODIFIERS.contains(name)
            || super::transform::OTHER_MODIFIERS.contains(name)
            || *name == This
    })
}

fn default_access_modifier(xot: &Xot, node: XotNode) -> TractorNode {
    let mut current = get_parent(xot, node);
    while let Some(parent) = current {
        if let Some(parent_kind) = get_kind(xot, parent).and_then(|kind| kind.parse::<CsKind>().ok()) {
            match parent_kind {
                CsKind::InterfaceDeclaration => return Public,
                CsKind::ClassDeclaration | CsKind::StructDeclaration | CsKind::RecordDeclaration => return Private,
                CsKind::DeclarationList => {}
                _ => break,
            }
        }
        current = get_parent(xot, parent);
    }
    Internal
}
