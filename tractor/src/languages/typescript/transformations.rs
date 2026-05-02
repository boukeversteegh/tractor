//! Per-kind transformations for TypeScript / JavaScript.
//!
//! Each function is a `Rule::Custom` target — `rule(TsKind) -> Rule`
//! references these by name. Simple flattens / pure renames /
//! `extract op + rename` patterns live as data in `rule()` (see
//! `rules.rs`), not here.

use xot::{Xot, Node as XotNode};

use crate::transform::{TransformAction, helpers::*};
use crate::transform::generic_type::rewrite_generic_type;
use crate::transform::operators::{extract_operator, is_prefix_form};

use super::input::TsKind;
use super::output::TractorNode::{
    self, Abstract, Alias, Arrow, Asserts, Async, Await, Comment as CommentName, Const, Default,
    Else, Export, Expression, Extends, Field, Function, Generator, Get, Leading, Let, Method,
    Name, NonNull, Optional, Override, Pair, Parameter, Predicate, Prefix, Private, Property,
    Protected, Public, Readonly, Required, Set, Static, Ternary, Trailing, Type, Unary,
    Var, Variable,
};

/// `expression_statement` — wrap value-producing statements in an
/// `<expression>` host (Principle #15). Control-flow constructs used
/// as statement-context expressions (`if`, `for`, `while`, `return`,
/// `break`, `continue`, `throw`, `try`) drop the wrapper — they sit
/// directly in the body. (TypeScript's `if`/`for`/etc. are normally
/// statements, but a `function*()` body or comma operator can put them
/// in an expression slot; defensively skipping by inner kind matches
/// the conservative carve-out used elsewhere.)
pub fn expression_statement(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let inner_kind = xot.children(node)
        .find(|&c| xot.element(c).is_some())
        .and_then(|c| get_kind(xot, c));
    let is_control_flow = matches!(
        inner_kind.as_deref(),
        Some(
            "if_statement" | "for_statement" | "for_in_statement" | "while_statement"
            | "do_statement" | "return_statement" | "break_statement" | "continue_statement"
            | "throw_statement" | "try_statement" | "switch_statement" | "labeled_statement"
            | "block_statement"
        )
    );
    if is_control_flow {
        Ok(TransformAction::Skip)
    } else {
        xot.with_renamed(node, Expression);
        Ok(TransformAction::Continue)
    }
}

/// Legacy skip used by other kinds that are pure grammar-level
/// wrappers; retained while the migration proceeds.
pub fn skip(_xot: &mut Xot, _node: XotNode) -> Result<TransformAction, xot::Error> {
    Ok(TransformAction::Skip)
}

/// `await_expression` — `await foo()`. Prefix marker.
///
/// When parent is already `<expression>` (e.g. from
/// `expression_statement`), lift the marker and flatten this node
/// to avoid `<expression>/<expression[await]>` double-wrap (caught
/// by `tree_invariants::no_repeated_parent_child_name`).
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

/// `non_null_expression` — `foo!`. Postfix marker. Replaces the prior
/// (incorrect) `Rename(Unary)` that classified `foo!` as a unary `!`
/// not-operator and extracted the `!` into `<op>`.
pub fn non_null_expression(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_renamed(node, Expression)
        .with_appended_marker(node, NonNull)?;
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
        xot.with_prepended_marker(node, Prefix)?;
    }
    Ok(TransformAction::Continue)
}

/// `<name>` field wrapper inserted by the builder. TypeScript-specific:
/// destructuring patterns (`const [a, b] = ...`, `const {x, y} = ...`)
/// appear as `name: array_pattern | object_pattern`. A pattern is not
/// a single name — flatten so the pattern becomes a direct child of
/// the declarator. Otherwise inline the standard identifier-family
/// children. `private_property_identifier` (`#foo`) strips the `#`
/// and lifts a `<private/>` marker onto the enclosing field/property.
pub fn name_wrapper(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let element_children: Vec<_> = xot
        .children(node)
        .filter(|&c| xot.element(c).is_some())
        .collect();
    if element_children.len() == 1 {
        let child = element_children[0];
        let ts_kind = get_kind(xot, child).and_then(|kind| kind.parse::<TsKind>().ok());
        if matches!(
            ts_kind,
            Some(TsKind::ArrayPattern | TsKind::ObjectPattern),
        ) {
            return Ok(TransformAction::Flatten);
        }
    }
    inline_single_identifier(xot, node)?;
    Ok(TransformAction::Continue)
}

/// `comment` — TypeScript / JavaScript emit a single `comment` kind
/// for `//` and `/* */`. Rename to `<comment>` and run the shared
/// trailing/leading/floating classifier.
pub fn comment(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_renamed(node, CommentName);
    static CLASSIFIER: crate::languages::comments::CommentClassifier =
        crate::languages::comments::CommentClassifier { line_prefixes: &["//"] };
    CLASSIFIER.classify_and_group(xot, node, Trailing, Leading)
}

/// `identifier` / `property_identifier` — always names. TypeScript uses
/// `type_identifier` for type positions, so bare identifiers are never
/// types.
pub fn identifier(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_renamed(node, Name);
    Ok(TransformAction::Continue)
}

/// `type_identifier` — type reference. `<type><name>Foo</name></type>`.
pub fn type_identifier(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_renamed(node, Type);
    wrap_text_in_name(xot, node)?;
    Ok(TransformAction::Continue)
}

/// `accessibility_modifier` (public/private/protected), `override_modifier`,
/// `readonly_modifier` — text → marker conversion. Source keyword
/// remains as a dangling sibling.
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

/// `asserts_annotation` — `asserts X is T`. Tree-sitter nests the inner
/// `type_predicate` (`X is T`) underneath the `asserts` keyword token.
/// Reshape to a single `<predicate>` carrying an `<asserts/>` marker:
///   1. Promote the nested `type_predicate`'s children directly under
///      this node, so the predicate is queryable without the raw grammar
///      wrapper.
///   2. Remove the nested `type_predicate` / `asserts` wrapper.
///   3. Prepend an `<asserts/>` empty marker.
///   4. Rename to `<predicate>`.
/// Result: `//predicate[asserts]` finds asserting predicates;
/// `//predicate` matches both forms (asserts and bare).
pub fn asserts_annotation(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    let children: Vec<_> = xot.children(node).collect();
    let mut asserts_wrapper: Option<XotNode> = None;
    let mut nested_predicate: Option<XotNode> = None;
    for child in &children {
        match get_kind(xot, *child).and_then(|k| k.parse::<TsKind>().ok()) {
            Some(TsKind::TypePredicate) => {
                nested_predicate = Some(*child);
            }
            _ if get_element_name(xot, *child).as_deref() == Some("asserts") => {
                asserts_wrapper = Some(*child);
                nested_predicate = xot.children(*child).find(|&grandchild| {
                    get_kind(xot, grandchild).and_then(|k| k.parse::<TsKind>().ok())
                        == Some(TsKind::TypePredicate)
                });
            }
            _ => {}
        }
    }

    if let Some(predicate) = nested_predicate {
        let insert_before = asserts_wrapper.unwrap_or(node);
        let inner: Vec<_> = xot.children(predicate).collect();
        for inner_child in inner {
            xot.detach(inner_child)?;
            xot.insert_before(insert_before, inner_child)?;
        }
        xot.detach(predicate)?;
    }

    if let Some(wrapper) = asserts_wrapper {
        xot.detach(wrapper)?;
    }

    xot.with_prepended_marker(node, Asserts)?
        .with_renamed(node, Predicate);
    Ok(TransformAction::Continue)
}

/// `formal_parameters` — wrap bare identifier params (JS shape) in
/// `<param>`, distribute `field="parameters"` to children, then
/// flatten so each parameter becomes a direct sibling of the function.
pub fn formal_parameters(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    wrap_bare_identifier_params(xot, node)?;
    distribute_field_to_children(xot, node, "parameters");
    Ok(TransformAction::Flatten)
}

/// `conditional_type` — `T extends X ? Y : Z`. Tree-sitter tags the
/// branches as `consequence` (then-type) and `alternative` (else-type).
/// `consequence` is mapped to `<then>` via the language's field
/// wrappings, but `alternative` has no generic mapping (the comment
/// in `mod.rs` notes it's done surgically per-language). Wrap the
/// alternative in `<else>` so the conditional has symmetric branch
/// slots, then rename to `<type[conditional]>`.
pub fn conditional_type(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    use super::output::TractorNode::{Conditional, Else};
    xot.with_wrapped_field_child(node, "alternative", Else)?
        .with_renamed(node, Type)
        .with_prepended_marker(node, Conditional)?;
    Ok(TransformAction::Continue)
}

/// `type_parameter` — `<T>` / `<T extends Shape>` / `<T = number>`.
/// Strips the `<value>` field-wrapper around the `default_type`
/// child so the post-transform `wrap_expression_positions` pass
/// doesn't add a value-namespace `<expression>` host around a
/// type-namespace slot. The `default_type` element itself stays —
/// its `RenameWithMarker(Type, Default)` rule fires on the next
/// walker step and produces `<type[default]>` as a direct child of
/// `<generic>`. Renames `type_parameter` to `<generic>`.
pub fn type_parameter(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let element_children: Vec<XotNode> = xot.children(node)
        .filter(|&c| xot.element(c).is_some())
        .collect();
    for child in element_children {
        if get_element_name(xot, child).as_deref() != Some("value") {
            continue;
        }
        let inner: Vec<XotNode> = xot.children(child)
            .filter(|&c| xot.element(c).is_some())
            .collect();
        for inner_child in inner {
            xot.detach(inner_child)?;
            xot.insert_before(child, inner_child)?;
        }
        xot.detach(child)?;
        break;
    }
    xot.with_renamed(node, super::output::TractorNode::Generic);
    Ok(TransformAction::Continue)
}

/// `constraint` — `<T extends Shape>` inside a `<type_parameter>`.
/// Tree-sitter wraps the `extends` keyword + bound type. Strip the
/// keyword text (the element name carries the meaning), rename to
/// `<extends>` per Principle #18 (name after operator), and add
/// `field="extends" list="true"` to match the cross-language
/// relationship shape (Java type_bound, Rust trait_bounds).
pub fn constraint(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    for child in xot.children(node).collect::<Vec<_>>() {
        if let Some(text) = xot.text_str(child) {
            if text.trim() == "extends" {
                xot.detach(child)?;
            }
        }
    }
    xot.with_renamed(node, Extends)
        .with_attr(node, "list", "extends");
    Ok(TransformAction::Continue)
}

/// `extends_clause` — `class Foo extends Bar` or `extends Base<T>`.
/// TS classes only allow one extends.
///
/// Tree-sitter quirk: inside `extends_clause`, a generic base like
/// `Base<T>` is emitted as flat sibling children
/// (`identifier "Base"` + `type_arguments<T>`) rather than nested
/// inside a `generic_type` node like every other type position.
/// Synthesize a `generic_type` wrapper around the pair so the
/// existing `generic_type` handler (run when the walker descends)
/// produces the canonical `<type[generic]>{name=Base, type=T}` shape.
///
/// After the synthetic wrap, retag the `<value>` field-wrapper (used
/// for non-generic forms) as `<type>` for the uniform namespace
/// vocabulary, then add `field="extends"` and `list="true"` for JSON
/// array consistency.
pub fn extends_clause(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    synthesize_generic_type_in_extends(xot, node)?;
    retag_value_as_type(xot, node)?;
    xot.with_renamed(node, Extends)
        .with_attr(node, "list", "extends");
    Ok(TransformAction::Continue)
}

/// Detect a base-name child + `type_arguments` sibling inside an
/// extends_clause and combine them into a synthetic `generic_type`
/// element so the walker's normal handler produces the canonical
/// generic-type shape.
///
/// The base-name child is the `<value>` field-wrapper inserted by the
/// builder (extends_clause's value field carries the parent class
/// identifier).
fn synthesize_generic_type_in_extends(
    xot: &mut Xot,
    node: XotNode,
) -> Result<(), xot::Error> {
    let children: Vec<XotNode> = xot.children(node)
        .filter(|&c| xot.element(c).is_some())
        .collect();
    let value_wrapper = children.iter().copied().find(|&c|
        get_element_name(xot, c).as_deref() == Some("value")
    );
    let type_args = children.iter().copied().find(|&c|
        get_kind(xot, c).as_deref() == Some("type_arguments")
    );
    let (value, args) = match (value_wrapper, type_args) {
        (Some(v), Some(a)) => (v, a),
        _ => return Ok(()),
    };
    let gt_id = xot.add_name("generic_type");
    let gt = xot.new_element(gt_id);
    xot.with_source_location_from(gt, value)
        .with_attr(gt, "kind", "generic_type");
    xot.insert_before(value, gt)?;
    let inner_elements: Vec<XotNode> = xot.children(value)
        .filter(|&c| xot.element(c).is_some())
        .collect();
    for inner in inner_elements {
        xot.detach(inner)?;
        xot.append(gt, inner)?;
    }
    xot.detach(value)?;
    xot.detach(args)?;
    xot.append(gt, args)?;
    Ok(())
}

/// `extends_type_clause` — `interface I extends A, B, C`. Multiple
/// targets allowed; produce flat `<extends>` siblings (Principle
/// #12 + #18) with `field="extends"`.
pub fn extends_type_clause(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    let elem_children: Vec<XotNode> = xot.children(node)
        .filter(|&c| xot.element(c).is_some())
        .collect();
    for child in elem_children {
        let extends_elt = xot.add_name(Extends.as_str());
        let extends_node = xot.new_element(extends_elt);
        xot.insert_before(child, extends_node)?;
        xot.detach(child)?;
        xot.append(extends_node, child)?;
        xot.with_attr(extends_node, "list", "extends");
    }
    Ok(TransformAction::Flatten)
}

/// `implements_clause` — `class Foo implements A, B, C`. Multiple
/// `<implements>` siblings with `field="implements"`.
pub fn implements_clause(
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

/// `type_alias_declaration` — `type Foo = …`. Drop the `<value>`
/// wrapper around the aliased type so it lives directly inside
/// `<alias>`. The walker then gives it its own `<type>` wrapper.
pub fn type_alias_declaration(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    let value_child = xot.children(node)
        .filter(|&c| xot.element(c).is_some())
        .find(|&c| get_element_name(xot, c).as_deref() == Some("value"));
    if let Some(v) = value_child {
        flatten_node(xot, v)?;
    }
    xot.with_renamed(node, Alias);
    Ok(TransformAction::Continue)
}

/// `ternary_expression` — wrap `alternative` in `<else>`, rename to
/// `<ternary>`. Cannot use the global field wrapping because
/// if_statement's `alternative` is already an `else_clause` →
/// `<else>` (a global wrap would double-nest there).
pub fn ternary_expression(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    xot.with_wrapped_field_child(node, "alternative", Else)?
        .with_renamed(node, Ternary);
    Ok(TransformAction::Continue)
}

/// `generic_type` — rewrite `Promise<T>` as
///   `<type><generic/>Promise<type field="arguments">T</type></type>`
/// matching the cross-language pattern.
pub fn generic_type(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    rewrite_generic_type(xot, node, &["type_identifier", "identifier"])?;
    Ok(TransformAction::Continue)
}

/// `lexical_declaration` / `variable_declaration` — extract keyword
/// modifiers (let/const/var/async/export/default) as marker children,
/// then rename to `<variable>`.
pub fn variable_declaration(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    extract_keyword_modifiers(xot, node)?;
    xot.with_renamed(node, Variable);
    Ok(TransformAction::Continue)
}

/// `optional_parameter` — `foo?: T`. Prepend `<optional/>` marker,
/// extract any modifier keywords (`readonly` for parameter properties),
/// rename to `<parameter>`.
pub fn optional_parameter(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    extract_field_modifiers(xot, node)?;
    xot.with_prepended_marker(node, Optional)?
        .with_renamed(node, Parameter);
    Ok(TransformAction::Continue)
}

/// `property_signature` — interface property declaration. Extracts
/// modifier keywords (`readonly`) into markers, then renames to
/// `<property>`.
pub fn property_signature(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    extract_field_modifiers(xot, node)?;
    xot.with_renamed(node, Property);
    Ok(TransformAction::Continue)
}

/// `required_parameter` — `foo: T`. Prepend `<required/>` marker
/// (exhaustive with optional), extract any modifier keywords
/// (`readonly` for parameter properties), rename to `<parameter>`.
pub fn required_parameter(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    extract_field_modifiers(xot, node)?;
    xot.with_prepended_marker(node, Required)?
        .with_renamed(node, Parameter);
    Ok(TransformAction::Continue)
}

/// Function-family declarations (function_declaration,
/// function_expression, arrow_function, generator_function,
/// generator_function_declaration). Lift `async` / `*` / `get|set`
/// keywords as marker children, then rename to the appropriate target.
fn function_with_markers(
    xot: &mut Xot,
    node: XotNode,
    to: TractorNode,
) -> Result<TransformAction, xot::Error> {
    extract_function_markers(xot, node)?;
    xot.with_renamed(node, to);
    Ok(TransformAction::Continue)
}

pub fn function_declaration(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    function_with_markers(xot, node, Function)
}

pub fn function_expression(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    function_with_markers(xot, node, Function)
}

pub fn arrow_function(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    function_with_markers(xot, node, Arrow)
}

pub fn generator_function(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    function_with_markers(xot, node, Function)
}

pub fn generator_function_declaration(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    function_with_markers(xot, node, Function)
}

/// `method_definition` — class method. Extract markers, default
/// public, rename METHOD.
pub fn method_definition(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    extract_function_markers(xot, node)?;
    if !has_visibility_marker(xot, node) {
        xot.with_prepended_marker(node, Public)?;
    }
    xot.with_renamed(node, Method);
    Ok(TransformAction::Continue)
}

/// `abstract_method_signature` — abstract class method. Extract
/// markers, default public, rename METHOD with `<abstract/>` marker.
pub fn abstract_method_signature(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    extract_function_markers(xot, node)?;
    if !has_visibility_marker(xot, node) {
        xot.with_prepended_marker(node, Public)?;
    }
    xot.with_renamed(node, Method)
        .with_prepended_marker(node, Abstract)?;
    Ok(TransformAction::Continue)
}

/// `shorthand_property_identifier` — `{ x }` is shorthand for
/// `{ x: x }`. Wrap in `<pair><name>x</name></pair>` so the shape
/// matches structured pairs (within-language Principle #5).
///
/// Caveat: tree-sitter also uses this kind for the `x` inside
/// `{ ...x }` (spread within an object literal). Pair-wrapping
/// would mislabel a spread target as a key/value pair, so detect
/// the spread parent context and rename only to `<name>` there.
pub fn shorthand_property_identifier(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    // Walk up: if any ancestor between us and the enclosing object
    // literal is a `spread_element`, treat as a name (no pair wrap).
    let mut cur = get_parent(xot, node);
    let mut in_spread = false;
    while let Some(p) = cur {
        let kind = get_kind(xot, p).and_then(|k| k.parse::<TsKind>().ok());
        match kind {
            Some(TsKind::SpreadElement) => { in_spread = true; break; }
            Some(TsKind::Object | TsKind::ObjectPattern) => break,
            _ => cur = get_parent(xot, p),
        }
    }
    if in_spread {
        xot.with_renamed(node, Name);
        return Ok(TransformAction::Continue);
    }
    let text = get_text_content(xot, node).unwrap_or_default().trim().to_string();
    xot.with_only_text(node, "")?;
    let name_elt = xot.add_name(Name.as_str());
    let name_node = xot.new_element(name_elt);
    xot.append(node, name_node)?;
    let text_node = xot.new_text(&text);
    xot.append(name_node, text_node)?;
    xot.with_renamed(node, Pair);
    Ok(TransformAction::Continue)
}

/// `public_field_definition` — class field. Default public.
/// Also extracts modifier keywords from the leading text (`static`,
/// `readonly`, `override`, `abstract`) into empty markers
/// (Principle #2 / #7).
pub fn public_field_definition(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    if !has_visibility_marker(xot, node) {
        xot.with_prepended_marker(node, Public)?;
    }
    extract_field_modifiers(xot, node)?;
    xot.with_renamed(node, Field);
    Ok(TransformAction::Continue)
}

/// Walk text children of a class field/property and convert
/// keyword tokens (`static`, `readonly`, `override`, `abstract`)
/// into empty marker children. Idempotent.
fn extract_field_modifiers(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    let texts = get_text_children(xot, node);
    let mut found: Vec<TractorNode> = Vec::new();
    for t in &texts {
        for tok in t.split_whitespace() {
            if let Ok(name) = tok.parse::<TractorNode>() {
                if matches!(name, Static | Readonly | Override | Abstract) && !found.contains(&name) {
                    found.push(name);
                }
            }
        }
    }
    // Skip if marker already present (idempotent re-application).
    let existing: std::collections::HashSet<String> = xot
        .children(node)
        .filter_map(|c| get_element_name(xot, c))
        .collect();
    for marker in found.into_iter().rev() {
        if !existing.contains(marker.as_str()) {
            xot.with_prepended_marker(node, marker)?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------
// Local helpers used by handlers above.
// ---------------------------------------------------------------------

fn extract_function_markers(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    let texts = get_text_children(xot, node);
    let mut has_async = false;
    let mut has_star = false;
    let mut accessor_kind: Option<TractorNode> = None;
    for t in &texts {
        for tok in t.split_whitespace() {
            let marker = tok.parse::<TractorNode>().ok();
            if marker == Some(Async) {
                has_async = true;
            }
            if tok == "*" || tok.ends_with('*') || tok.starts_with('*') {
                has_star = true;
            }
            match marker {
                Some(Get) => accessor_kind = Some(Get),
                Some(Set) => accessor_kind = Some(Set),
                _ => {}
            }
        }
    }
    if let Some(k) = accessor_kind {
        xot.with_prepended_marker(node, k)?;
    }
    if has_star {
        xot.with_prepended_marker(node, Generator)?;
    }
    if has_async {
        xot.with_prepended_marker(node, Async)?;
    }
    Ok(())
}

fn extract_keyword_modifiers(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    let texts = get_text_children(xot, node);
    let found: Vec<TractorNode> = texts.iter()
        .filter_map(|t| t.parse().ok())
        .filter(|name| matches!(name, Let | Const | Var | Async | Export | Default))
        .collect();
    for modifier in found.into_iter().rev() {
        xot.with_prepended_marker(node, modifier)?;
    }
    Ok(())
}

fn wrap_bare_identifier_params(xot: &mut Xot, list: XotNode) -> Result<(), xot::Error> {
    let children: Vec<XotNode> = xot.children(list)
        .filter(|&c| xot.element(c).is_some())
        .collect();
    for child in children {
        if get_kind(xot, child).as_deref() != Some("identifier") {
            continue;
        }
        let param_name = get_name(xot, Parameter);
        let param = xot.new_element(param_name);
        xot.with_source_location_from(param, child)
            .with_wrap_child(child, param)?;
    }
    Ok(())
}

fn retag_value_as_type(xot: &mut Xot, parent: XotNode) -> Result<(), xot::Error> {
    let value_child = xot.children(parent)
        .filter(|&c| xot.element(c).is_some())
        .find(|&c| get_element_name(xot, c).as_deref() == Some("value"));
    if let Some(v) = value_child {
        xot.with_renamed(v, Type);
    }
    Ok(())
}

fn has_visibility_marker(xot: &Xot, node: XotNode) -> bool {
    for child in xot.children(node) {
        if xot.element(child).is_none() { continue; }
        if matches!(
            get_kind(xot, child).and_then(|kind| kind.parse::<TsKind>().ok()),
            Some(TsKind::AccessibilityModifier)
        ) {
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

fn inline_single_identifier(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    let children: Vec<_> = xot.children(node).collect();
    for child in children {
        let child_kind = get_element_name(xot, child)
            .and_then(|name| name.parse::<TsKind>().ok());
        if !matches!(
            child_kind,
            Some(
                TsKind::Identifier
                    | TsKind::PropertyIdentifier
                    | TsKind::PrivatePropertyIdentifier
                    | TsKind::TypeIdentifier
            ),
        ) {
            continue;
        }
        let text = match get_text_content(xot, child) {
            Some(t) => t,
            None => continue,
        };
        let is_private = child_kind == Some(TsKind::PrivatePropertyIdentifier);
        let clean_text = if is_private {
            text.trim_start_matches('#').to_string()
        } else {
            text
        };
        xot.with_only_text(node, &clean_text)?;
        if is_private {
            if let Some(parent) = get_parent(xot, node) {
                // Remove any auto-added Public marker (the parent's
                // public_field_definition Custom ran before the `#`
                // prefix was visible here).
                let public_to_remove: Vec<_> = xot
                    .children(parent)
                    .filter(|&c| {
                        get_element_name(xot, c)
                            .and_then(|name| name.parse::<TractorNode>().ok())
                            == Some(Public)
                    })
                    .collect();
                for c in public_to_remove {
                    xot.detach(c)?;
                }
                let already = xot.children(parent).any(|c| {
                    get_element_name(xot, c)
                        .and_then(|name| name.parse::<TractorNode>().ok())
                        == Some(Private)
                });
                if !already {
                    // The `#` prefix on the property name IS the source
                    // token; copy `node`'s location onto the marker.
                    xot.with_prepended_marker_from(parent, Private, node)?;
                }
            }
        }
        return Ok(());
    }
    Ok(())
}
