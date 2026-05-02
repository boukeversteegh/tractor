//! Per-kind transformations for Go.
//!
//! Each function is a `Rule::Custom` target — `rule(GoKind) -> Rule`
//! references these by name. A transformation owns the renaming,
//! child reshaping, and `TransformAction` choice for kinds that don't
//! fit a shared `Rule` variant.
//!
//! Simple flattens / pure renames / `extract op + rename` patterns
//! live as data in `rule()` (see `semantic.rs`), not here.

use xot::{Xot, Node as XotNode};

use crate::transform::{TransformAction, helpers::*};

use super::input::GoKind;
use super::output::TractorNode::{
    self, Alias, Blank, Comment as CommentName, Dot, Else, Exported, Field, Function, If, Import,
    Interface, Leading, Method, Name, Path, Raw, Short, String as GoString, Struct, Trailing, Type,
    Unexported, Variable,
};

/// `selector_expression` — `obj.field`. Wraps the operand and field
/// names in role-named containers (`<object>` / `<property>`) so
/// the two `<name>` siblings under `<member>` no longer collide on
/// element name. Matches TS/Java/Python member-access shape
/// (Principle #19).
pub fn selector_expression(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    use super::output::TractorNode::{Member, Object, Property};
    let elem_children: Vec<XotNode> = xot.children(node)
        .filter(|&c| xot.element(c).is_some())
        .collect();
    for child in elem_children {
        let field = get_attr(xot, child, "field");
        let wrapper = match field.as_deref() {
            Some("operand") => Object,
            Some("field") => Property,
            _ => continue,
        };
        let wrapper_id = xot.add_name(wrapper.as_str());
        let wrapper_node = xot.new_element(wrapper_id);
        xot.with_source_location_from(wrapper_node, child)
            .with_wrap_child(child, wrapper_node)?;
    }
    xot.with_renamed(node, Member);
    Ok(TransformAction::Continue)
}

/// `type_instantiation_expression` — `Map[int, string]` standalone
/// (e.g. `var mapper = Map[int, string]`). Tree-sitter emits the
/// head type and each argument as flat siblings of the wrapper. Per
/// Principle #5 (within-Go consistency with the parameter-position
/// `Container[T]` shape that goes through `type_arguments`): rename
/// to `<type[generic]>` and tag every type sibling AFTER the head
/// with `field="arguments" list="true"` so JSON serializers
/// reconstruct as an `arguments` array.
pub fn type_instantiation_expression(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    let elem_children: Vec<XotNode> = xot.children(node)
        .filter(|&c| xot.element(c).is_some())
        .collect();
    // Skip the head (first element child); the remaining elements are
    // the type arguments.
    for child in elem_children.iter().skip(1) {
        xot.with_attr(*child, "list", "arguments");
    }
    xot.with_renamed(node, Type)
        .with_prepended_marker(node, super::output::TractorNode::Generic)?;
    Ok(TransformAction::Continue)
}

/// `type_arguments` — `[T, string]` after a generic type. Tree-sitter
/// nests each argument under a transparent `type_elem` wrapper; lift
/// each `type_elem`'s element child up first so the subsequent flatten
/// + `distribute_field("arguments")` lands the field attribute on the
/// real type child (otherwise it sits on the `type_elem` wrapper and
/// gets lost in flatten).
pub fn type_arguments(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let elem_children: Vec<XotNode> = xot.children(node)
        .filter(|&c| xot.element(c).is_some())
        .collect();
    for child in elem_children {
        if get_kind(xot, child).as_deref() != Some("type_elem") {
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
    }
    distribute_list_to_children(xot, node, "arguments");
    Ok(TransformAction::Flatten)
}

/// `expression_statement` is a pure grammar wrapper around a single
/// expression. Skip its subtree so the inner expression's transform
/// drives the output (matches the previous behavior of returning
/// `TransformAction::Skip` directly).
pub fn expression_statement(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_renamed(node, super::output::TractorNode::Expression);
    Ok(TransformAction::Continue)
}

/// `parameter_list` does double duty in Go: formal parameters AND
/// multi-value return specs. The builder has already wrapped the
/// returns case in a `<returns>` element; check the parent to decide.
pub fn parameter_list(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let in_returns = get_parent(xot, node)
        .and_then(|p| get_element_name(xot, p))
        .as_deref()
        == Some("returns");
    if in_returns {
        collapse_return_param_list(xot, node)?;
    } else {
        distribute_list_to_children(xot, node, "parameters");
    }
    Ok(TransformAction::Flatten)
}

/// `type_declaration` — move the leading `type` keyword text into
/// the inner `type_spec` / `type_alias` so the keyword stays attached
/// when the outer wrapper is flattened.
pub fn type_declaration(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    move_type_keyword_into_spec(xot, node)?;
    Ok(TransformAction::Flatten)
}

/// `raw_string_literal` — render as `<string>` with a `<raw/>` marker.
pub fn raw_string_literal(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_prepended_marker(node, Raw)?
        .with_renamed(node, GoString);
    Ok(TransformAction::Continue)
}

/// `short_var_declaration` (`x := 42`) — render as `<variable>` with
/// a `<short/>` marker to distinguish from `var x = 42`.
pub fn short_var_declaration(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_prepended_marker(node, Short)?
        .with_renamed(node, Variable);
    Ok(TransformAction::Continue)
}

/// `function_declaration` — prepend `<exported/>` / `<unexported/>`
/// based on the function name's capitalisation, then rename to
/// `<function>`.
pub fn function_declaration(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let marker = get_export_marker(xot, node);
    xot.with_prepended_marker(node, marker)?
        .with_renamed(node, Function);
    Ok(TransformAction::Continue)
}

/// `method_declaration` — same export-marker pattern, rename to
/// `<method>`.
pub fn method_declaration(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let marker = get_export_marker(xot, node);
    xot.with_prepended_marker(node, marker)?
        .with_renamed(node, Method);
    Ok(TransformAction::Continue)
}

/// `field_declaration` — same export-marker pattern (Go capitalisation
/// rule applies to struct fields too), rename to `<field>`.
pub fn field_declaration(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let marker = get_export_marker(xot, node);
    xot.with_prepended_marker(node, marker)?
        .with_renamed(node, Field);
    Ok(TransformAction::Continue)
}

/// `type_spec` — three shapes:
///   - `type Hello struct {…}`   → `<struct><name>Hello</name>…</struct>`
///   - `type Greeter interface…`  → `<interface><name>Greeter</name>…</interface>`
///   - `type MyInt int`           → `<type><name>MyInt</name><type>int</type></type>`
///
/// For the first two, hoist the inner shape up so the declaration
/// reads "I'm declaring a struct named Hello" (Goal #5).
pub fn type_spec(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let marker = get_export_marker(xot, node);
    xot.with_prepended_marker(node, marker)?;

    let inner = xot
        .children(node)
        .filter(|&c| xot.element(c).is_some())
        .find(|&c| matches!(
            get_kind(xot, c).and_then(|kind| kind.parse::<GoKind>().ok()),
            Some(GoKind::StructType | GoKind::InterfaceType)
        ));

    if let Some(inner) = inner {
        let inner_kind = get_kind(xot, inner).and_then(|kind| kind.parse::<GoKind>().ok());
        let new_name = if inner_kind == Some(GoKind::StructType) { Struct } else { Interface };
        // For interface types, wrap each embedding entry (a `type_elem`
        // with a single plain-type child) in `<extends>` so
        // cross-language `//interface/extends/type[name='X']` finds Go's
        // embedded interfaces. Type-set elements (negated_type, unions
        // involving `|`) keep their shape — they're constraint types,
        // not parent types.
        if inner_kind == Some(GoKind::InterfaceType) {
            wrap_interface_embeds_in_extends(xot, inner)?;
        }
        xot.with_renamed(node, new_name);
        let inner_children: Vec<_> = xot.children(inner).collect();
        for c in inner_children {
            xot.detach(c)?;
            xot.insert_before(inner, c)?;
        }
        xot.detach(inner)?;
    } else {
        xot.with_renamed(node, Type);
    }
    Ok(TransformAction::Continue)
}

fn wrap_interface_embeds_in_extends(
    xot: &mut Xot,
    interface_type_node: XotNode,
) -> Result<(), xot::Error> {
    use super::output::TractorNode::Extends;
    let elem_children: Vec<XotNode> = xot.children(interface_type_node)
        .filter(|&c| xot.element(c).is_some())
        .collect();
    for child in elem_children {
        if get_kind(xot, child).as_deref() != Some("type_elem") {
            continue;
        }
        let inner_elements: Vec<XotNode> = xot.children(child)
            .filter(|&c| xot.element(c).is_some())
            .collect();
        if inner_elements.len() != 1 {
            continue;
        }
        let inner_kind = get_kind(xot, inner_elements[0]);
        let is_plain_type = matches!(
            inner_kind.as_deref(),
            Some("qualified_type")
                | Some("type_identifier")
                | Some("generic_type")
        );
        if !is_plain_type {
            continue;
        }
        let ext_elt = xot.add_name(Extends.as_str());
        let ext_node = xot.new_element(ext_elt);
        xot.with_source_location_from(ext_node, child)
            .with_attr(ext_node, "list", "extends");
        xot.insert_before(child, ext_node)?;
        xot.detach(child)?;
        xot.append(ext_node, child)?;
    }
    Ok(())
}

/// `type_alias` (`type Color = int`) — distinct from `type MyInt int`.
/// Rename to `<alias>` with the export marker.
pub fn type_alias(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let marker = get_export_marker(xot, node);
    xot.with_prepended_marker(node, marker)?
        .with_renamed(node, Alias);
    Ok(TransformAction::Continue)
}

/// `if_statement` — Go's tree-sitter doesn't emit an `else_clause`
/// wrapper; the `alternative` field points directly at a nested
/// `if_statement` (for `else if`) or a block. Wrap the alternative in
/// `<else>` so the shared conditional-shape post-transform can
/// collapse the chain uniformly.
pub fn if_statement(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_wrapped_field_child(node, "alternative", Else)?
        .with_renamed(node, If);
    Ok(TransformAction::Continue)
}

/// `type_identifier` — rename to `<type>` and wrap the text in
/// `<name>` so `//type[name='Foo']` matches uniformly across
/// declaration and reference sites.
pub fn type_identifier(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_renamed(node, Type);
    wrap_text_in_name(xot, node)?;
    Ok(TransformAction::Continue)
}

/// `<name>` field wrapper inserted by the builder for nodes with a
/// `field=name` attribute. Inline the single identifier-like child
/// as text:
///   `<name><identifier>foo</identifier></name>` → `<name>foo</name>`
///
/// Also accepts:
///   - `package_identifier` (Go import alias `myio "io"`),
///   - already-renamed `<name>` (walk-order race),
///   - `dot` (Go's `import . "pkg"`),
///   - `blank_identifier` (Go's `_`).
///
/// Called from the dispatcher's wrapper branch, not from the rule
/// table — the node has no `kind=` attribute since it was synthesised
/// by the builder, not emitted by tree-sitter.
pub fn name_wrapper(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let children: Vec<_> = xot.children(node).collect();
    for child in children {
        let child_kind = match get_element_name(xot, child) {
            Some(n) => n,
            None => continue,
        }
        .parse::<GoKind>()
        .ok();
        let child_name = get_element_name(xot, child)
            .and_then(|name| name.parse::<TractorNode>().ok());
        if !matches!(
            child_kind,
            Some(
                GoKind::Identifier
                    | GoKind::TypeIdentifier
                    | GoKind::FieldIdentifier
                    | GoKind::PackageIdentifier
                    | GoKind::Dot
                    | GoKind::BlankIdentifier
            ),
        ) && child_name != Some(Name) {
            continue;
        }
        let text = match get_text_content(xot, child) {
            Some(t) => t,
            None => continue,
        };
        xot.with_only_text(node, &text)?;
        break;
    }
    Ok(TransformAction::Continue)
}

/// `comment` — normalise to `<comment>` and run the shared
/// trailing/leading/floating classifier with `//` line-comment
/// grouping.
pub fn comment(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    xot.with_renamed(node, CommentName);
    static CLASSIFIER: crate::languages::comments::CommentClassifier =
        crate::languages::comments::CommentClassifier { line_prefixes: &["//"] };
    CLASSIFIER.classify_and_group(xot, node, Trailing, Leading)
}

/// `import_declaration` — strips the `import`/`(`/`)` punctuation
/// tokens then flattens. Single-import case: the inner `import_spec`
/// becomes a sibling at the file level. Block case: every spec
/// becomes a sibling — the parens are a Go-specific syntax sugar
/// with no shared prefix, so flat siblings is the right shape (per
/// `imports-grouping.md`).
pub fn import_declaration(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    let to_drop: Vec<_> = xot.children(node)
        .filter(|&c| {
            let Some(text) = xot.text_str(c) else { return false; };
            let trimmed = text.trim();
            trimmed == "import" || trimmed == "(" || trimmed == ")" || trimmed.is_empty()
        })
        .collect();
    for c in to_drop {
        xot.detach(c)?;
    }
    Ok(TransformAction::Flatten)
}

/// `import_spec` — build the unified `<import>` shape:
///   - `import "fmt"`         → `<import><path>fmt</path></import>`
///   - `import myio "io"`     → `<import[alias]><path>io</path><alias><name>myio</name></alias></import>`
///   - `import . "strings"`   → `<import[dot]><path>strings</path></import>`
///   - `import _ "..."`       → `<import[blank]><path>...</path></import>`
///
/// Tree-sitter Go assigns `name` and `path` fields to import_spec
/// children. The builder wraps `name` in a `<name>` element (per
/// `GO_FIELD_WRAPPINGS`); `path` is unwrapped because it isn't in
/// the field-wrapping table. So at handler time the children are:
///   - `<name>` wrapper (containing package_identifier / blank_
///     identifier / dot) — text content tells us which.
///   - `interpreted_string_literal` (or `raw_string_literal`) —
///     descendant text is the quoted path.
pub fn import_spec(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    enum PrefixKind { Alias, Blank, Dot }
    let mut path_text: Option<String> = None;
    let mut prefix: Option<(PrefixKind, String)> = None;
    for child in xot.children(node).collect::<Vec<_>>() {
        // Path: tree-sitter's `interpreted_string_literal` /
        // `raw_string_literal` (no field wrapper).
        if matches!(
            get_kind(xot, child).and_then(|k| k.parse::<GoKind>().ok()),
            Some(GoKind::InterpretedStringLiteral | GoKind::RawStringLiteral)
        ) {
            let raw = descendant_text(xot, child);
            let stripped = raw
                .trim()
                .trim_start_matches('"')
                .trim_end_matches('"')
                .trim_start_matches('`')
                .trim_end_matches('`')
                .to_string();
            path_text = Some(stripped);
            continue;
        }
        // Prefix: builder wrapped `name` field as `<name>` element.
        if get_element_name(xot, child).as_deref() == Some("name") {
            let text = descendant_text(xot, child).trim().to_string();
            let kind = match text.as_str() {
                "_" => PrefixKind::Blank,
                "." => PrefixKind::Dot,
                "" => continue,
                _ => PrefixKind::Alias,
            };
            prefix = Some((kind, text));
        }
    }

    let path_text = match path_text {
        Some(p) => p,
        None => return Ok(TransformAction::Continue),
    };

    // Detach ALL original children — we'll build the new shape from scratch.
    for child in xot.children(node).collect::<Vec<_>>() {
        xot.detach(child)?;
    }

    // Append <path>TEXT</path>.
    let path_elt = xot.add_name(Path.as_str());
    let path_node = xot.new_element(path_elt);
    xot.append(node, path_node)?;
    let path_text_node = xot.new_text(&path_text);
    xot.append(path_node, path_text_node)?;

    if let Some((kind, text)) = prefix {
        match kind {
            PrefixKind::Blank => {
                xot.with_prepended_marker(node, Blank)?;
            }
            PrefixKind::Dot => {
                xot.with_prepended_marker(node, Dot)?;
            }
            PrefixKind::Alias => {
                let alias_elt = xot.add_name(Alias.as_str());
                let alias_node = xot.new_element(alias_elt);
                xot.append(node, alias_node)?;
                let name_elt = xot.add_name(Name.as_str());
                let name_node = xot.new_element(name_elt);
                xot.append(alias_node, name_node)?;
                let alias_text_node = xot.new_text(&text);
                xot.append(name_node, alias_text_node)?;
                xot.with_prepended_marker(node, Alias)?;
            }
        }
    }

    xot.with_renamed(node, Import);
    Ok(TransformAction::Done)
}

/// `import_spec_list` — strip `(`/`)` punctuation tokens, then flatten.
/// Without the strip, the parens promote to the parent and leak as
/// bare text leaves at file-level. Also used for `const_spec_list`
/// and `var_spec_list` (same `(...)` block shape).
pub fn import_spec_list(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    let to_drop: Vec<_> = xot.children(node)
        .filter(|&c| {
            let Some(text) = xot.text_str(c) else { return false; };
            let trimmed = text.trim();
            trimmed == "(" || trimmed == ")" || trimmed.is_empty()
        })
        .collect();
    for c in to_drop {
        xot.detach(c)?;
    }
    Ok(TransformAction::Flatten)
}

/// `const_declaration` / `var_declaration` — same flat-siblings
/// pattern as imports. Strip the leading keyword and any block
/// parens, then flatten. Each `const_spec` / `var_spec` (renamed to
/// `<const>` / `<var>` by their own rules) becomes its own sibling.
pub fn const_or_var_declaration(
    xot: &mut Xot,
    node: XotNode,
) -> Result<TransformAction, xot::Error> {
    let to_drop: Vec<_> = xot.children(node)
        .filter(|&c| {
            let Some(text) = xot.text_str(c) else { return false; };
            let trimmed = text.trim();
            // Tree-sitter Go combines bare `const`/`var` keywords
            // with the following `(` into a single anonymous text
            // token (`"const ("` / `"var ("`); also handle the
            // unsplit forms.
            trimmed == "const" || trimmed == "var"
                || trimmed == "const (" || trimmed == "var ("
                || trimmed == "(" || trimmed == ")" || trimmed == ";"
                || trimmed.is_empty()
        })
        .collect();
    for c in to_drop {
        xot.detach(c)?;
    }
    Ok(TransformAction::Flatten)
}

// ---------------------------------------------------------------------
// Local helpers — used by the handlers above. Mirror the same
// helpers in `transform.rs`; once the dispatcher swap lands and the
// match-based path is gone, the originals there can be deleted.
// ---------------------------------------------------------------------

/// Move the literal `type` keyword text from a `type_declaration` into
/// its inner `type_spec` / `type_alias` child.
fn move_type_keyword_into_spec(xot: &mut Xot, decl: XotNode) -> Result<(), xot::Error> {
    let keyword = match xot.children(decl).find(|&c| {
        xot.text_str(c).map(|t| t.trim() == "type").unwrap_or(false)
    }) {
        Some(k) => k,
        None => return Ok(()),
    };
    let spec = xot
        .children(decl)
        .filter(|&c| xot.element(c).is_some())
        .find(|&c| matches!(
            get_kind(xot, c).and_then(|kind| kind.parse::<GoKind>().ok()),
            Some(GoKind::TypeSpec | GoKind::TypeAlias)
        ));
    let spec = match spec {
        Some(s) => s,
        None => return Ok(()),
    };
    xot.detach(keyword)?;
    xot.prepend(spec, keyword)?;
    Ok(())
}

/// Strip the `<param>` wrapper from each return-type entry so a
/// returns list reads as a sequence of types, not parameters.
fn collapse_return_param_list(xot: &mut Xot, list: XotNode) -> Result<(), xot::Error> {
    let children: Vec<XotNode> = xot.children(list).filter(|&c| xot.element(c).is_some()).collect();
    for child in children {
        if get_kind(xot, child).and_then(|kind| kind.parse::<GoKind>().ok())
            != Some(GoKind::ParameterDeclaration) {
            continue;
        }
        let type_child = xot.children(child).find(|&c| {
            get_element_name(xot, c)
                .and_then(|name| name.parse::<TractorNode>().ok())
                == Some(Type)
                || matches!(
                    get_kind(xot, c).and_then(|kind| kind.parse::<GoKind>().ok()),
                    Some(
                        GoKind::TypeIdentifier
                            | GoKind::PointerType
                            | GoKind::SliceType
                            | GoKind::ArrayType
                            | GoKind::MapType
                            | GoKind::ChannelType
                            | GoKind::InterfaceType
                            | GoKind::StructType
                            | GoKind::GenericType
                    )
                )
        });
        if let Some(type_node) = type_child {
            xot.detach(type_node)?;
            xot.insert_before(child, type_node)?;
            xot.detach(child)?;
        }
    }
    Ok(())
}

/// Determine `<exported/>` vs `<unexported/>` from the name child's
/// first-character capitalisation.
fn get_export_marker(xot: &Xot, node: XotNode) -> TractorNode {
    for child in xot.children(node) {
        if let Some(name) = get_element_name(xot, child) {
            if name.parse::<TractorNode>().ok() == Some(Name) {
                for grandchild in xot.children(child) {
                    if let Some(text) = get_text_content(xot, grandchild) {
                        if text.starts_with(|c: char| c.is_uppercase()) {
                            return Exported;
                        }
                        return Unexported;
                    }
                }
                if let Some(text) = get_text_content(xot, child) {
                    if text.starts_with(|c: char| c.is_uppercase()) {
                        return Exported;
                    }
                    return Unexported;
                }
            }
            if matches!(name.parse::<GoKind>().ok(), Some(GoKind::Identifier | GoKind::TypeIdentifier)) {
                if let Some(field) = get_attr(xot, child, "field") {
                    if field == "name" {
                        if let Some(text) = get_text_content(xot, child) {
                            if text.starts_with(|c: char| c.is_uppercase()) {
                                return Exported;
                            }
                            return Unexported;
                        }
                    }
                }
            }
        }
    }
    Unexported
}
