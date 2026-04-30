//! Per-kind transformations for TypeScript / JavaScript.
//!
//! Each function is a `Rule::Custom` target — `rule(TsKind) -> Rule`
//! references these by name. Simple flattens / pure renames /
//! `extract op + rename` patterns live as data in `rule()` (see
//! `rules.rs`), not here.

use xot::{Xot, Node as XotNode};

use crate::transform::{TransformAction, helpers::*};
use crate::transform::generic_type::rewrite_generic_type;

use super::output::TsName::{
    self, Abstract, Alias, Annotation, Arrow, Async, Comment as CommentName, Const, Default, Else,
    Export, Extends, Field, Function, Generator, Generic, Generics, Get, Leading, Let, Method, Name,
    Optional, Parameter, Private, Property, Protected, Public, Required, Set, Ternary, Trailing,
    Type, Var, Variable,
};

/// Kinds whose name happens to match our semantic vocabulary already
/// (`array`, `constraint`, `object`, `pair`, `super`, `this`,
/// `undefined`) or grammar supertypes.
pub fn passthrough(_xot: &mut Xot, _node: XotNode) -> Result<TransformAction, xot::Error> {
    Ok(TransformAction::Continue)
}

/// `expression_statement` — drop the wrapper before children are
/// visited so children's parent context becomes the enclosing block.
pub fn skip(_xot: &mut Xot, _node: XotNode) -> Result<TransformAction, xot::Error> {
    Ok(TransformAction::Skip)
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
        let ts_kind = get_kind(xot, child);
        if matches!(
            ts_kind.as_deref(),
            Some("array_pattern") | Some("object_pattern"),
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
    rename(xot, node, CommentName);
    static CLASSIFIER: crate::languages::comments::CommentClassifier =
        crate::languages::comments::CommentClassifier { line_prefixes: &["//"] };
    CLASSIFIER.classify_and_group(xot, node, Trailing.as_str(), Leading.as_str())
}

/// `identifier` / `property_identifier` — always names. TypeScript uses
/// `type_identifier` for type positions, so bare identifiers are never
/// types.
pub fn identifier(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    rename(xot, node, Name);
    Ok(TransformAction::Continue)
}

/// `type_identifier` — type reference. `<type><name>Foo</name></type>`.
pub fn type_identifier(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    rename(xot, node, Type);
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
            insert_text_after(xot, node, &text)?;
            return Ok(TransformAction::Done);
        }
    }
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

/// `extends_clause` — `class Foo extends Bar`. Tree-sitter tags the
/// base-class identifier as `field="value"`; retag as `<type>` for
/// the uniform namespace vocabulary.
pub fn extends_clause(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    retag_value_as_type(xot, node)?;
    rename(xot, node, Extends);
    Ok(TransformAction::Continue)
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
    rename(xot, node, Alias);
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
    wrap_field_child(xot, node, "alternative", Else)?;
    rename(xot, node, Ternary);
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
    rename(xot, node, Variable);
    Ok(TransformAction::Continue)
}

/// `optional_parameter` — `foo?: T`. Prepend `<optional/>` marker,
/// rename to `<parameter>`.
pub fn optional_parameter(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    prepend_empty_element(xot, node, Optional)?;
    rename(xot, node, Parameter);
    Ok(TransformAction::Continue)
}

/// `required_parameter` — `foo: T`. Prepend `<required/>` marker
/// (exhaustive with optional), rename to `<parameter>`.
pub fn required_parameter(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    prepend_empty_element(xot, node, Required)?;
    rename(xot, node, Parameter);
    Ok(TransformAction::Continue)
}

/// Function-family declarations (function_declaration,
/// function_expression, arrow_function, generator_function,
/// generator_function_declaration). Lift `async` / `*` / `get|set`
/// keywords as marker children, then rename to the appropriate target.
fn function_with_markers(
    xot: &mut Xot,
    node: XotNode,
    to: TsName,
) -> Result<TransformAction, xot::Error> {
    extract_function_markers(xot, node)?;
    rename(xot, node, to);
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
        prepend_empty_element(xot, node, Public)?;
    }
    rename(xot, node, Method);
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
        prepend_empty_element(xot, node, Public)?;
    }
    rename(xot, node, Method);
    prepend_empty_element(xot, node, Abstract)?;
    Ok(TransformAction::Continue)
}

/// `public_field_definition` — class field. Default public.
pub fn public_field_definition(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    if !has_visibility_marker(xot, node) {
        prepend_empty_element(xot, node, Public)?;
    }
    rename(xot, node, Field);
    Ok(TransformAction::Continue)
}

// ---------------------------------------------------------------------
// Local helpers used by handlers above.
// ---------------------------------------------------------------------

fn extract_function_markers(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    let texts = get_text_children(xot, node);
    let mut has_async = false;
    let mut has_star = false;
    let mut accessor_kind: Option<TsName> = None;
    for t in &texts {
        for tok in t.split_whitespace() {
            if tok == "async" {
                has_async = true;
            }
            if tok == "*" || tok.ends_with('*') || tok.starts_with('*') {
                has_star = true;
            }
            match tok {
                "get" => accessor_kind = Some(Get),
                "set" => accessor_kind = Some(Set),
                _ => {}
            }
        }
    }
    if let Some(k) = accessor_kind {
        prepend_empty_element(xot, node, k)?;
    }
    if has_star {
        prepend_empty_element(xot, node, Generator)?;
    }
    if has_async {
        prepend_empty_element(xot, node, Async)?;
    }
    Ok(())
}

fn extract_keyword_modifiers(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    let texts = get_text_children(xot, node);
    let found: Vec<TsName> = texts.iter()
        .filter_map(|t| match t.as_str() {
            "let" => Some(Let),
            "const" => Some(Const),
            "var" => Some(Var),
            "async" => Some(Async),
            "export" => Some(Export),
            "default" => Some(Default),
            _ => None,
        })
        .collect();
    for modifier in found.into_iter().rev() {
        prepend_empty_element(xot, node, modifier)?;
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
        let param_name = xot.add_name(Parameter.as_str());
        let param = xot.new_element(param_name);
        copy_source_location(xot, child, param);
        xot.insert_before(child, param)?;
        xot.detach(child)?;
        xot.append(param, child)?;
    }
    Ok(())
}

fn retag_value_as_type(xot: &mut Xot, parent: XotNode) -> Result<(), xot::Error> {
    let value_child = xot.children(parent)
        .filter(|&c| xot.element(c).is_some())
        .find(|&c| get_element_name(xot, c).as_deref() == Some("value"));
    if let Some(v) = value_child {
        rename(xot, v, Type);
        set_attr(xot, v, "field", "type");
    }
    Ok(())
}

fn has_visibility_marker(xot: &Xot, node: XotNode) -> bool {
    for child in xot.children(node) {
        if xot.element(child).is_none() { continue; }
        let ts_kind = get_kind(xot, child);
        if ts_kind.as_deref() == Some("accessibility_modifier") {
            return true;
        }
        if let Some(name) = get_element_name(xot, child) {
            if name == Public.as_str() || name == Private.as_str() || name == Protected.as_str() {
                return true;
            }
        }
    }
    false
}

fn inline_single_identifier(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    let children: Vec<_> = xot.children(node).collect();
    for child in children {
        let child_name = match get_element_name(xot, child) {
            Some(n) => n,
            None => continue,
        };
        if !matches!(
            child_name.as_str(),
            "identifier" | "property_identifier" | "private_property_identifier" | "type_identifier",
        ) {
            continue;
        }
        let text = match get_text_content(xot, child) {
            Some(t) => t,
            None => continue,
        };
        let is_private = child_name == "private_property_identifier";
        let clean_text = if is_private {
            text.trim_start_matches('#').to_string()
        } else {
            text
        };
        let all_children: Vec<_> = xot.children(node).collect();
        for c in all_children {
            xot.detach(c)?;
        }
        let text_node = xot.new_text(&clean_text);
        xot.append(node, text_node)?;
        if is_private {
            if let Some(parent) = get_parent(xot, node) {
                let already = xot.children(parent).any(|c| {
                    get_element_name(xot, c).as_deref() == Some(Private.as_str())
                });
                if !already {
                    prepend_empty_element(xot, parent, Private)?;
                }
            }
        }
        return Ok(());
    }
    Ok(())
}
