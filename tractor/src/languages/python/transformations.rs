//! Per-kind transformations for Python.
//!
//! Each function is a `Rule::Custom` target — `rule(PyKind) -> Rule`
//! references these by name. A transformation owns the renaming,
//! child reshaping, and `TransformAction` choice for kinds that don't
//! fit a shared `Rule` variant.
//!
//! Simple flattens / pure renames / `extract op + rename` patterns
//! live as data in `rule()` (see `rules.rs`), not here.

use xot::{Xot, Node as XotNode};

use crate::transform::{TransformAction, helpers::*};
use crate::transform::generic_type::rewrite_generic_type;

use super::input::PyKind;
use super::output::TractorNode::{
    self, Async, Await, Comment as CommentName, Comprehension, Dict, Else, Expression, Function,
    Generic, Leading, List, Literal, Parameter, Private, Protected, Public, Set, Ternary, Trailing,
};

/// `expression_statement` — wrap value-producing statements in an
/// `<expression>` host (Principle #15). Python's `expression_statement`
/// is also used for plain `assignment`s in tree-sitter's grammar; let
/// `assignment` (renamed `<assign>`) live directly under the body so
/// it stays a peer of other declaration-like statements.
pub fn expression_statement(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let inner_kind = xot.children(node)
        .find(|&c| xot.element(c).is_some())
        .and_then(|c| get_kind(xot, c));
    let is_control_flow_or_decl = matches!(
        inner_kind.as_deref(),
        Some(
            "assignment" | "augmented_assignment"
            | "yield" | "raise_statement"
        )
    );
    if is_control_flow_or_decl {
        Ok(TransformAction::Skip)
    } else {
        xot.with_renamed(node, Expression);
        Ok(TransformAction::Continue)
    }
}

/// Legacy skip used by other kinds; retained while migration proceeds.
pub fn skip(_xot: &mut Xot, _node: XotNode) -> Result<TransformAction, xot::Error> {
    Ok(TransformAction::Skip)
}


/// `await` — Python's `await foo()`. Prefix marker.
pub fn await_expression(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_renamed(node, Expression)
        .with_prepended_marker(node, Await)?;
    Ok(TransformAction::Continue)
}

/// `<name>` field wrapper inserted by the builder for nodes with a
/// `field=name` attribute. Python-specific cases:
///   - Single child of kind `aliased_import` / `import_from_statement`:
///     not a name but a compound — flatten so the import becomes a
///     direct child of the enclosing import_statement.
///   - Single child of post-rename `<import>` / `<from>`: same.
///   - Otherwise: inline single identifier as text.
pub fn name_wrapper(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let element_children: Vec<_> = xot
        .children(node)
        .filter(|&c| xot.element(c).is_some())
        .collect();
    if element_children.len() == 1 {
        let child = element_children[0];
        let ts_kind = get_kind(xot, child);
        let el_name = get_element_name(xot, child);
        if matches!(
            ts_kind.as_deref(),
            Some("aliased_import") | Some("import_from_statement"),
        ) || matches!(
            el_name.as_deref(),
            Some("import") | Some("from"),
        ) {
            return Ok(TransformAction::Flatten);
        }
    }
    inline_single_identifier(xot, node)?;
    Ok(TransformAction::Continue)
}

/// `comment` — Python only has `#` line comments (block strings are
/// `<string>`, not `<comment>`; see python::docstring tests). Rename
/// to `<comment>` and run the shared trailing/leading/floating
/// classifier with `#` line-comment grouping.
pub fn comment(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_renamed(node, CommentName);
    static CLASSIFIER: crate::languages::comments::CommentClassifier =
        crate::languages::comments::CommentClassifier { line_prefixes: &["#"] };
    CLASSIFIER.classify_and_group(xot, node, Trailing, Leading)
}

/// `generic_type` — rewrite `List[X]` as
///   `<type><generic/>List<type field="arguments">X</type></type>`
/// matching the cross-language pattern used by C# / Java / TS.
pub fn generic_type(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    rewrite_generic_type(xot, node, &["identifier", "type_identifier"])?;
    Ok(TransformAction::Continue)
}

/// `type` — Python's tree-sitter wraps a single identifier (or a
/// `generic_type`) as a type annotation. Inline the identifier text
/// then wrap in `<name>` for the unified vocabulary
/// (`<type><name>int</name></type>`). If the content is a
/// `generic_type` (already rewritten into its own `<type>`), drop
/// the outer wrapper to avoid double-nesting.
pub fn type_node(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let single_child = xot
        .children(node)
        .filter(|&c| xot.element(c).is_some())
        .next();
    if let Some(child) = single_child {
        if get_kind(xot, child).as_deref() == Some("generic_type") {
            return Ok(TransformAction::Flatten);
        }
    }
    inline_single_identifier(xot, node)?;
    wrap_text_in_name(xot, node)?;
    Ok(TransformAction::Continue)
}

/// `type_parameter` serves double duty in tree-sitter Python:
/// PEP 695 declaration params (`def f[T]()`) and subscript generic
/// args (`Optional[str]`). Dispatch by parent kind.
pub fn type_parameter(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    let parent_kind = get_parent(xot, node).and_then(|p| get_kind(xot, p));
    if parent_kind.as_deref() == Some("generic_type") {
        // Subscript form: parent is already `<type[generic]>`. Flatten
        // so type-args become direct children — matches TS shape.
        Ok(TransformAction::Flatten)
    } else {
        // PEP 695 declaration form: rename to `<generic>` and let it
        // sit as a direct child of the function / class / type-alias.
        xot.with_renamed(node, Generic);
        Ok(TransformAction::Continue)
    }
}

/// `for_statement` — `for x in xs:` / `async for x in xs:`. Extract
/// `<async/>` marker if the source is an `async for`.
pub fn for_statement(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    let texts = get_text_children(xot, node);
    if texts.iter().any(|t| t.split_whitespace().any(|tok| tok == "async")) {
        xot.with_prepended_marker(node, Async)?;
    }
    xot.with_renamed(node, super::output::TractorNode::For);
    Ok(TransformAction::Continue)
}

/// `with_statement` — `with x:` / `async with x:`. Same async-marker
/// extraction as for_statement.
pub fn with_statement(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    let texts = get_text_children(xot, node);
    if texts.iter().any(|t| t.split_whitespace().any(|tok| tok == "async")) {
        xot.with_prepended_marker(node, Async)?;
    }
    xot.with_renamed(node, super::output::TractorNode::With);
    Ok(TransformAction::Continue)
}

/// `parameters` — Python's parameter list. Bare positional parameters
/// surface as plain `<identifier>` children (which become `<name>`),
/// inconsistent with `<parameter>` for default/typed params and
/// cross-language convention where every parameter is a `<parameter>`
/// with markers (Principle #5).
///
/// Wrap each bare `identifier` child in a `<parameter>` element so
/// `//parameter` finds positional bare params. Splat patterns
/// (`*args`, `**kwargs`) keep their `<spread[list]>` / `<spread[dict]>`
/// shape for now — separate iter to unify those under `<parameter>`
/// with marker vocab.
pub fn parameters(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    let children: Vec<XotNode> = xot
        .children(node)
        .filter(|&c| xot.element(c).is_some())
        .collect();
    for child in children {
        if get_kind(xot, child).as_deref() == Some("identifier") {
            let param_name_id = xot.add_name(Parameter.as_str());
            let param = xot.new_element(param_name_id);
            xot.with_source_location_from(param, child);
            xot.insert_before(child, param)?;
            xot.detach(child)?;
            xot.append(param, child)?;
        }
    }
    distribute_field_to_children(xot, node, "parameters");
    Ok(TransformAction::Flatten)
}

/// `function_definition` — extract async modifier if present;
/// inject a visibility marker when the function lives directly inside
/// a `class_definition` body (Principle #9). Python's convention:
///   `__x__` (dunder) → public (interface hook)
///   `__x`            → private
///   `_x`             → protected
///   `x`              → public
pub fn function_definition(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    let texts = get_text_children(xot, node);
    if texts.iter().any(|t| t.contains("async")) {
        xot.with_prepended_marker(node, Async)?;
    }
    if is_inside_class_body(xot, node) {
        if let Some(vis) = python_visibility_from_def(xot, node) {
            xot.with_prepended_marker(node, vis)?;
        }
    }
    xot.with_renamed(node, Function);
    Ok(TransformAction::Continue)
}

/// `decorated_definition` — `@foo\ndef bar(): …`. Tree-sitter wraps
/// this as `decorated_definition(decorator, …, class|function)`. Move
/// every `decorator` child INTO the inner declaration (so it surfaces
/// as a direct child of `<class>` / `<function>` matching the
/// cross-language topology), then flatten the outer wrapper.
pub fn decorated_definition(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    let children: Vec<_> = xot.children(node).collect();
    let decl = children.iter().copied().find(|&c| matches!(
        get_kind(xot, c).as_deref(),
        Some("class_definition") | Some("function_definition")
            | Some("async_function_definition"),
    ));
    if let Some(decl) = decl {
        let decorators: Vec<_> = xot.children(node)
            .filter(|&c| get_kind(xot, c).as_deref() == Some("decorator"))
            .collect();
        for dec in decorators.into_iter().rev() {
            xot.detach(dec)?;
            xot.prepend(decl, dec)?;
        }
    }
    Ok(TransformAction::Flatten)
}

/// `conditional_expression` — `b if cond else c`. Wrap the
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

/// `list` literal — `[1, 2, 3]`. Prepend `<literal/>` marker so the
/// construction form is exhaustive (Principle #9):
///   `<list><literal/>...</list>`        — `[1, 2, 3]`
///   `<list><comprehension/>...</list>`  — `[x for x in xs]`
pub fn list_literal(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_prepended_marker(node, Literal)?;
    Ok(TransformAction::Continue)
}

/// `set` literal — `{1, 2, 3}`. Same shape as `list_literal`.
pub fn set_literal(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_prepended_marker(node, Literal)?;
    Ok(TransformAction::Continue)
}

/// `dictionary` literal — `{k: v}`. Prepend `<literal/>` marker, then
/// rename to `<dict>` for the cross-language vocabulary.
pub fn dictionary_literal(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    xot.with_prepended_marker(node, Literal)?
        .with_renamed(node, Dict);
    Ok(TransformAction::Continue)
}

/// `list_comprehension` — `[x for x in xs]`. Prepend
/// `<comprehension/>` marker, then rename to `<list>` for the unified
/// collection vocabulary.
pub fn list_comprehension(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    xot.with_prepended_marker(node, Comprehension)?
        .with_renamed(node, List);
    Ok(TransformAction::Continue)
}

/// `dictionary_comprehension` — `{k: v for k, v in items}`. Prepend
/// `<comprehension/>` marker, then rename to `<dict>`.
pub fn dictionary_comprehension(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    xot.with_prepended_marker(node, Comprehension)?
        .with_renamed(node, Dict);
    Ok(TransformAction::Continue)
}

/// `argument_list` — context-aware. In a class's superclasses
/// position, wrap each positional element in `<extends>` (Principle
/// #18: name relationships after the operator; Principle #12: no
/// list container). In a regular call context, distribute
/// `field="arguments"` and flatten as before.
pub fn argument_list(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    let in_class = get_parent(xot, node)
        .and_then(|p| get_kind(xot, p))
        .and_then(|k| k.parse::<PyKind>().ok())
        == Some(PyKind::ClassDefinition);
    if in_class {
        let elem_children: Vec<XotNode> = xot.children(node)
            .filter(|&c| xot.element(c).is_some())
            .filter(|&c| {
                !matches!(
                    get_kind(xot, c).and_then(|k| k.parse::<PyKind>().ok()),
                    Some(PyKind::KeywordArgument)
                )
            })
            .collect();
        for child in elem_children {
            let extends_elt = xot.add_name("extends");
            let extends_node = xot.new_element(extends_elt);
            xot.insert_before(child, extends_node)?;
            xot.detach(child)?;
            xot.append(extends_node, child)?;
            xot.with_attr(extends_node, "list", "extends");
        }
    } else {
        distribute_field_to_children(xot, node, "arguments");
    }
    Ok(TransformAction::Flatten)
}

/// `set_comprehension` — `{x for x in xs}`. Prepend `<comprehension/>`
/// marker, then rename to `<set>`.
pub fn set_comprehension(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    xot.with_prepended_marker(node, Comprehension)?
        .with_renamed(node, Set);
    Ok(TransformAction::Continue)
}

/// `keyword_pattern` — `x=0` inside a class pattern. Strips the bare
/// `=` text leaf, adds `[keyword]` marker, renames to `<pattern>`.
/// The resulting shape is `<pattern[keyword]>{<name>x</name><int>0</int>}`
/// — name first, value second. The marker distinguishes from
/// positional class-pattern args.
pub fn keyword_pattern(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    use super::output::TractorNode::{Keyword, Pattern as PatternName};
    for child in xot.children(node).collect::<Vec<_>>() {
        if let Some(text) = xot.text_str(child) {
            if text.trim() == "=" {
                xot.detach(child)?;
            }
        }
    }
    xot.with_prepended_marker(node, Keyword)?
        .with_renamed(node, PatternName);
    Ok(TransformAction::Continue)
}

// ---------------------------------------------------------------------
// Local helpers used by handlers above.
// ---------------------------------------------------------------------

fn inline_single_identifier(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    let children: Vec<_> = xot.children(node).collect();
    for child in children {
        let ts_kind = get_kind(xot, child);
        let el_name = get_element_name(xot, child);
        let matches_kind = matches!(
            ts_kind.as_deref(),
            Some("identifier") | Some("type_identifier"),
        );
        let matches_el = matches!(
            el_name.as_deref(),
            Some("name"),
        );
        let matches_dotted = matches!(
            ts_kind.as_deref(),
            Some("dotted_name") | Some("relative_import"),
        ) && single_identifier_descendant(xot, child);
        if !(matches_kind || matches_el || matches_dotted) {
            continue;
        }
        let text = if matches_dotted {
            descendant_text(xot, child).trim().to_string()
        } else {
            match get_text_content(xot, child) {
                Some(t) => t,
                None => continue,
            }
        };
        if text.is_empty() {
            continue;
        }
        xot.with_only_text(node, &text)?;
        return Ok(());
    }
    Ok(())
}

fn is_inside_class_body(xot: &Xot, node: XotNode) -> bool {
    let mut current = get_parent(xot, node);
    while let Some(parent) = current {
        if let Some(kind) = get_kind(xot, parent) {
            if let Some(py_kind) = PyKind::from_str(&kind) {
                match py_kind {
                    PyKind::ClassDefinition => return true,
                    PyKind::FunctionDefinition => return false,
                    _ => {}
                }
            }
        }
        current = get_parent(xot, parent);
    }
    false
}

fn python_visibility_from_def(xot: &Xot, node: XotNode) -> Option<TractorNode> {
    let name_wrapper_node = xot.children(node).find(|&c| {
        xot.element(c).is_some()
            && get_element_name(xot, c).as_deref() == Some("name")
            && get_kind(xot, c).is_none()
    })?;
    let ident_text = descendant_text(xot, name_wrapper_node);
    let name = ident_text.trim();
    if name.is_empty() { return None; }
    if name.starts_with("__") && name.ends_with("__") && name.len() > 4 {
        return Some(Public);
    }
    if name.starts_with("__") {
        return Some(Private);
    }
    if name.starts_with('_') {
        return Some(Protected);
    }
    Some(Public)
}

fn single_identifier_descendant(xot: &Xot, node: XotNode) -> bool {
    let mut count = 0usize;
    let mut is_ident = false;
    let mut stack = vec![node];
    while let Some(n) = stack.pop() {
        for c in xot.children(n) {
            if xot.element(c).is_some() {
                count += 1;
                if count > 1 {
                    return false;
                }
                let kind = get_kind(xot, c);
                is_ident = matches!(
                    kind.as_deref(),
                    Some("identifier") | Some("type_identifier"),
                );
                stack.push(c);
            }
        }
    }
    count == 1 && is_ident
}
