//! Xot tree transformation infrastructure
//!
//! Provides a generic tree walker and low-level helpers for xot manipulation.
//! No assumptions about AST structure - each language defines its own transform logic.
//!
//! ## Architecture
//! ```text
//! AST → build_raw() → xot tree → apply_field_wrappings → walk_transform(lang_fn) → transformed tree
//! ```

pub mod builder;
pub mod conditionals;
pub mod data_keys;
pub mod generic_type;
pub mod operators;
pub mod singletons;

use xot::{Xot, Node as XotNode, NameId};

// =============================================================================
// TRANSFORM ACTION - Control flow for the walker
// =============================================================================

/// Result of transforming a node - controls how the walker proceeds
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransformAction {
    /// Continue processing children normally
    Continue,
    /// Skip this node entirely - detach it, promote children to parent
    Skip,
    /// Flatten this node - transform children first, then detach node and promote them
    Flatten,
    /// Node fully handled - don't recurse into children
    Done,
}

// =============================================================================
// TREE WALKER - Language-agnostic traversal
// =============================================================================

/// Walk an xot tree and apply a transform function to each element node.
///
/// The transform function receives each node and returns a `TransformAction`
/// to control how the walker proceeds.
pub fn walk_transform<F>(xot: &mut Xot, root: XotNode, mut transform_fn: F) -> Result<(), xot::Error>
where
    F: FnMut(&mut Xot, XotNode) -> Result<TransformAction, xot::Error>,
{
    // Find the actual content root (skip document wrapper)
    let content_root = find_content_root(xot, root);

    // Apply transform to the content root, but protect it from being
    // removed (Flatten/Skip) since it's the document element.
    if xot.element(content_root).is_some() {
        let action = transform_fn(xot, content_root)?;
        match action {
            TransformAction::Flatten | TransformAction::Skip | TransformAction::Continue => {
                // Process children regardless — Flatten/Skip at the root just means
                // "this wrapper is unimportant", but we can't remove the document element.
                let children: Vec<XotNode> = xot.children(content_root)
                    .filter(|&c| xot.element(c).is_some())
                    .collect();
                for child in children {
                    walk_node(xot, child, &mut transform_fn)?;
                }
            }
            TransformAction::Done => {}
        }
        Ok(())
    } else {
        walk_node(xot, content_root, &mut transform_fn)
    }
}

/// Walk and transform starting from a specific node (no wrapper skipping).
pub fn walk_transform_node<F>(xot: &mut Xot, node: XotNode, mut transform_fn: F) -> Result<(), xot::Error>
where
    F: FnMut(&mut Xot, XotNode) -> Result<TransformAction, xot::Error>,
{
    walk_node(xot, node, &mut transform_fn)
}

/// Find the actual content root, skipping the document node wrapper
fn find_content_root(xot: &Xot, node: XotNode) -> XotNode {
    if xot.is_document(node) {
        if let Ok(elem) = xot.document_element(node) {
            return elem;
        }
    }
    node
}

/// Apply per-language field-wrapping rules to the raw builder output.
///
/// The builder is mechanical: every element carries a `field="X"`
/// attribute for tree-sitter's field name (if any), and no wrapping is
/// performed. Each language then decides which fields should be wrapped
/// in a semantic element (for example, TS wraps `return_type` in
/// `<returns>`). `wrappings` is a slice of `(tree_sitter_field,
/// wrapper_element_name)` pairs; elements with `field=X` matching a
/// pair are moved inside a new `<Y>` wrapper that inherits the element's
/// source location. The wrapper's *element name* is the JSON key
/// (Principle #19); no `field=` is written on the wrapper or the inner
/// element by this pass.
pub fn apply_field_wrappings(
    xot: &mut Xot,
    root: XotNode,
    wrappings: &[(&str, &str)],
) -> Result<(), xot::Error> {
    use helpers::*;
    if wrappings.is_empty() {
        return Ok(());
    }
    let root = find_content_root(xot, root);

    // Collect (element, wrapper_name) pairs first so we can mutate afterwards.
    let mut targets: Vec<(XotNode, String)> = Vec::new();
    collect_wrap_targets(xot, root, wrappings, &mut targets);

    for (element, wrapper_name) in targets {
        let wrapper_id = xot.add_name(&wrapper_name);
        let wrapper = xot.new_element(wrapper_id);
        xot.with_source_location_from(wrapper, element)
            .with_wrap_child(element, wrapper)?;
        // The wrapper element's name IS the JSON key (Principle #19;
        // role-uniform singleton wrappers). The inner element keeps any
        // tree-sitter `field=` attribute it carried — preserved for
        // `--meta` debug output, ignored by JSON.
    }
    Ok(())
}

fn collect_wrap_targets(
    xot: &Xot,
    node: XotNode,
    wrappings: &[(&str, &str)],
    out: &mut Vec<(XotNode, String)>,
) {
    use helpers::*;
    if xot.element(node).is_none() {
        return;
    }
    if let Some(field) = get_attr(xot, node, "field") {
        for (ts_field, wrapper_name) in wrappings {
            if field == *ts_field {
                out.push((node, (*wrapper_name).to_string()));
                break;
            }
        }
    }
    for child in xot.children(node) {
        collect_wrap_targets(xot, child, wrappings, out);
    }
}

/// Wrap the first element child of every "expression position" slot
/// in an `<expression>` host (Principle #15: stable expression hosts).
///
/// `slot_names` lists the wrapper element names that mark expression
/// positions for the language — typically `value`, `condition`,
/// `left`, `right`, etc. (whichever fields the language's field-wrap
/// table produced).
///
/// Idempotent: if the slot's child is already an `<expression>`, no
/// double-wrap. Synthesized hosts carry no source location (they're
/// position markers, not source tokens).
pub fn wrap_expression_positions(
    xot: &mut Xot,
    root: XotNode,
    slot_names: &[&str],
) -> Result<(), xot::Error> {
    use helpers::*;
    if slot_names.is_empty() {
        return Ok(());
    }
    let root = find_content_root(xot, root);

    let mut targets: Vec<XotNode> = Vec::new();
    collect_expression_position_targets(xot, root, slot_names, &mut targets);

    for child in targets {
        let host_id = xot.add_name("expression");
        let host = xot.new_element(host_id);
        xot.with_wrap_child(child, host)?;
    }
    Ok(())
}

/// Wrap value-producing direct children of body-like containers in
/// `<expression>` hosts (Principle #15 stable expression hosts).
///
/// `body_names`: container element names whose children are body
/// statements (e.g. `["body", "then", "else"]` for Ruby).
/// `value_kinds`: opt-IN list of element names that are
/// value-producing — only these get wrapped. Statement-only kinds
/// (declarations, control flow, jump statements, comments) are
/// left bare.
///
/// Idempotent: skips children that are already `<expression>`.
/// Synthesized hosts carry no source location (position markers).
pub fn wrap_body_value_children(
    xot: &mut Xot,
    root: XotNode,
    body_names: &[&str],
    value_kinds: &[&str],
) -> Result<(), xot::Error> {
    use helpers::*;
    if body_names.is_empty() || value_kinds.is_empty() {
        return Ok(());
    }
    let root = find_content_root(xot, root);

    let mut targets: Vec<XotNode> = Vec::new();
    collect_body_value_targets(xot, root, body_names, value_kinds, &mut targets);

    for child in targets {
        let host_id = xot.add_name("expression");
        let host = xot.new_element(host_id);
        xot.with_wrap_child(child, host)?;
    }
    Ok(())
}

/// Ensure every immediate element child of a type-reference slot
/// (`<extends>`, `<implements>`, `<throws>`, `<returns>`) is a
/// `<type>` element (Principle #14: namespace vocabulary —
/// type-reference slots use `<type>` uniformly across languages).
///
/// Pre-iter-117 inconsistency:
///   Java/TS: `<extends><type><name>Foo</name></type></extends>`  ✓
///   C#/Python/Ruby/PHP: `<extends><name>Foo</name></extends>`    ✗
///
/// This pass walks the tree post-transform; for any inner `<name>`
/// (or other non-`<type>` name-shaped child), it wraps the child in
/// a `<type>` element. Already-`<type>` children and inner element
/// kinds that aren't type references (e.g. C#'s primary-constructor
/// argument lists) are left alone.
pub fn wrap_relationship_targets_in_type(
    xot: &mut Xot,
    root: XotNode,
) -> Result<(), xot::Error> {
    use helpers::*;
    const RELATIONSHIPS: &[&str] = &["extends", "implements", "throws", "returns"];
    let root = find_content_root(xot, root);
    let mut targets: Vec<XotNode> = Vec::new();
    fn collect(
        xot: &Xot,
        node: XotNode,
        out: &mut Vec<XotNode>,
    ) {
        if xot.element(node).is_some() {
            if let Some(name) = get_element_name(xot, node) {
                if RELATIONSHIPS.contains(&name.as_str()) {
                    out.push(node);
                }
            }
        }
        for c in xot.children(node) {
            collect(xot, c, out);
        }
    }
    collect(xot, root, &mut targets);
    for parent in targets {
        let elem_children: Vec<XotNode> = xot.children(parent)
            .filter(|&c| xot.element(c).is_some())
            .collect();
        // Only wrap when the inner element is a bare `<name>` — that's
        // the recognizable type-reference shape that escaped wrapping.
        // `<type>` and `<type[generic]>` are already correct; other
        // element shapes (e.g. C# constructor-arg passthrough) aren't
        // type references and should be left alone.
        for child in elem_children {
            let child_name = match get_element_name(xot, child) {
                Some(n) => n,
                None => continue,
            };
            if child_name != "name" {
                continue;
            }
            let type_id = xot.add_name("type");
            let type_node = xot.new_element(type_id);
            xot.with_source_location_from(type_node, child)
                .with_wrap_child(child, type_node)?;
        }
    }
    Ok(())
}

/// Flatten nested `<path>` elements: when a `<path>` contains another
/// `<path>` as a direct child, lift the inner `<path>`'s children up
/// into the outer one and detach the inner wrapper. Repeated until no
/// nesting remains, so right-deep grammar shapes
/// (`scoped_identifier(scoped_identifier(scoped_identifier(...)), name)`)
/// collapse to a single `<path>` with flat `<name>` segments. Each
/// `<name>` segment also gains `list="name"` so JSON renders the
/// path as `path.name: ["com", "example", "foo"]` instead of the
/// scalar-vs-children fallback.
///
/// Without this pass, dotted paths like `com.example.foo` render
/// right-deep in XML and produce ugly JSON
/// (`path.path.path.name` collisions). With it, every path is a flat
/// list of segments — `//path/name[1]='com'`,
/// `//path/name[last()]='foo'` work positionally.
pub fn flatten_nested_paths(xot: &mut Xot, root: XotNode) -> Result<(), xot::Error> {
    use helpers::*;
    let root = find_content_root(xot, root);
    loop {
        let mut targets: Vec<XotNode> = Vec::new();
        fn collect(
            xot: &Xot,
            node: XotNode,
            out: &mut Vec<XotNode>,
        ) {
            if xot.element(node).is_some()
                && get_element_name(xot, node).as_deref() == Some("path")
            {
                let has_inner_path = xot.children(node).any(|c| {
                    xot.element(c).is_some()
                        && get_element_name(xot, c).as_deref() == Some("path")
                });
                if has_inner_path {
                    out.push(node);
                }
            }
            for c in xot.children(node) {
                collect(xot, c, out);
            }
        }
        collect(xot, root, &mut targets);
        if targets.is_empty() {
            break;
        }
        for outer in targets {
            let inner_paths: Vec<XotNode> = xot.children(outer)
                .filter(|&c| {
                    xot.element(c).is_some()
                        && get_element_name(xot, c).as_deref() == Some("path")
                })
                .collect();
            for inner in inner_paths {
                let inner_children: Vec<XotNode> = xot.children(inner).collect();
                for child in inner_children {
                    xot.detach(child)?;
                    xot.insert_before(inner, child)?;
                }
                xot.detach(inner)?;
            }
        }
    }
    // Tag every `<name>` child of every `<path>` with `list="name"`
    // so JSON renders the path as a list of segments rather than
    // collapsing to scalar+children.
    let mut all_paths: Vec<XotNode> = Vec::new();
    fn collect_paths(xot: &Xot, node: XotNode, out: &mut Vec<XotNode>) {
        if xot.element(node).is_some()
            && get_element_name(xot, node).as_deref() == Some("path")
        {
            out.push(node);
        }
        for c in xot.children(node) {
            collect_paths(xot, c, out);
        }
    }
    collect_paths(xot, root, &mut all_paths);
    for path in all_paths {
        let name_children: Vec<XotNode> = xot.children(path)
            .filter(|&c| {
                xot.element(c).is_some()
                    && get_element_name(xot, c).as_deref() == Some("name")
            })
            .collect();
        for name in name_children {
            if get_attr(xot, name, "list").is_none() {
                xot.with_attr(name, "list", "name");
            }
        }
    }
    Ok(())
}

/// Distribute `list="<element-name>"` to every direct element child of
/// every node whose element name is a statement-container (`body`,
/// `block`, `unit`, `file`, `program`, `module`, etc.). The container's
/// children are role-mixed by element name (methods, fields, properties,
/// statements) but role-uniform within each name — i.e. multiple
/// `<method>` siblings under `<body>` should JSON-serialize as
/// `body.method: [{...}, {...}]` regardless of count, never collapsing
/// to a scalar for the 1-method case.
///
/// Without this pass, a 1-method body produces `body.method: {...}`
/// (scalar) while a 2-method body produces `body.method` collision +
/// fallback-children — content-dependent shape, contrary to Principle
/// #12 / #19. With this pass, every body member carries
/// `list="<element-name>"` so JSON always emits an array under that
/// element-name key.
///
/// Idempotent: skips children that already have `list=`.
pub fn distribute_member_list_attrs(
    xot: &mut Xot,
    root: XotNode,
    container_names: &[&str],
) -> Result<(), xot::Error> {
    use helpers::*;
    let root = find_content_root(xot, root);
    let mut targets: Vec<XotNode> = Vec::new();
    fn collect(
        xot: &Xot,
        node: XotNode,
        container_names: &[&str],
        out: &mut Vec<XotNode>,
    ) {
        if xot.element(node).is_some() {
            if let Some(name) = get_element_name(xot, node) {
                if container_names.contains(&name.as_str()) {
                    out.push(node);
                }
            }
        }
        for c in xot.children(node) {
            collect(xot, c, container_names, out);
        }
    }
    collect(xot, root, container_names, &mut targets);
    for container in targets {
        let elem_children: Vec<XotNode> = xot.children(container)
            .filter(|&c| xot.element(c).is_some())
            .collect();
        for child in elem_children {
            // Skip if already tagged.
            if get_attr(xot, child, "list").is_some() {
                continue;
            }
            // Skip self-closing markers — they're presence flags, not
            // list members. JSON serializes them as boolean properties
            // regardless of `list=`. Tagging them adds attribute noise
            // and produces misleading XPath signals like
            // `static[@list="static"]`.
            if !xot.children(child).any(|c| xot.element(c).is_some() || xot.text_str(c).is_some()) {
                continue;
            }
            let element_name = match get_element_name(xot, child) {
                Some(n) => n,
                None => continue,
            };
            xot.with_attr(child, "list", &element_name);
        }
    }
    Ok(())
}

/// Strip `{` / `}` / `;` punctuation text leaves from `<body>`-shaped
/// elements (and any other element name in `body_names`) recursively
/// across the tree. C-family languages emit braces as anonymous text
/// children of body wrappers; the braces are pure syntax — the body
/// element itself already conveys "block here." Removing them keeps
/// queries clean. Source-text reconstruction can re-add braces.
pub fn strip_body_braces(
    xot: &mut Xot,
    root: XotNode,
    body_names: &[&str],
) -> Result<(), xot::Error> {
    use helpers::*;
    let mut bodies: Vec<XotNode> = Vec::new();
    fn collect(
        xot: &Xot,
        node: XotNode,
        body_names: &[&str],
        out: &mut Vec<XotNode>,
    ) {
        if xot.element(node).is_some()
            && get_element_name(xot, node)
                .as_deref()
                .map_or(false, |n| body_names.contains(&n))
        {
            out.push(node);
        }
        for c in xot.children(node) {
            collect(xot, c, body_names, out);
        }
    }
    collect(xot, root, body_names, &mut bodies);
    // Reverse document order so children are processed before
    // parents — that way, when `<body>` contains `<block>` and the
    // block strips to empty, the body sees an empty child set and
    // detaches itself.
    bodies.reverse();
    for body in bodies {
        // Build a set of element-child names — any text content that
        // exactly matches a sibling marker name is a keyword leak and
        // can be stripped (the marker captures the same fact).
        let element_child_names: std::collections::HashSet<String> = xot.children(body)
            .filter_map(|c| get_element_name(xot, c))
            .collect();
        let children: Vec<XotNode> = xot.children(body).collect();
        for c in children {
            let Some(text) = xot.text_str(c).map(|s| s.to_string()) else { continue };
            let trimmed = text.trim();
            if trimmed.is_empty() || trimmed == ";" {
                xot.detach(c)?;
                continue;
            }
            // Strip leading `{` / `(` and trailing `}` / `)` —
            // delimiters that the parent element already implies.
            let after_open = if let Some(rest) = trimmed.strip_prefix('{') {
                rest.trim_start()
            } else if let Some(rest) = trimmed.strip_prefix('(') {
                rest.trim_start()
            } else {
                trimmed
            };
            let stripped = if let Some(rest) = after_open.strip_suffix('}') {
                rest.trim_end()
            } else if let Some(rest) = after_open.strip_suffix(')') {
                rest.trim_end()
            } else {
                after_open
            };
            // If, after brace-stripping, the remaining text is a bare
            // keyword that matches a sibling marker (e.g. `continue`
            // matching `<continue/>` marker), strip it too — the
            // marker already captures the keyword.
            let final_text = if element_child_names.contains(stripped) {
                ""
            } else {
                stripped
            };
            if final_text == trimmed {
                continue;
            }
            if final_text.is_empty() {
                xot.detach(c)?;
            } else {
                let new_text = xot.new_text(final_text);
                xot.insert_before(c, new_text)?;
                xot.detach(c)?;
            }
        }
        // If the body is now empty (no children at all), detach it.
        // Empty `<body/>` would render as a `[body]` marker on the
        // parent — since there's no content, the marker carries no
        // useful info. Better to drop the element entirely.
        if xot.children(body).next().is_none() {
            xot.detach(body)?;
        }
    }
    Ok(())
}

fn collect_body_value_targets(
    xot: &Xot,
    node: XotNode,
    body_names: &[&str],
    value_kinds: &[&str],
    out: &mut Vec<XotNode>,
) {
    use helpers::*;
    if xot.element(node).is_none() {
        return;
    }
    let element_name = get_element_name(xot, node);
    let is_body = element_name
        .as_deref()
        .map_or(false, |n| body_names.contains(&n));
    if is_body {
        for child in xot.children(node) {
            if xot.element(child).is_none() {
                continue;
            }
            let child_name = get_element_name(xot, child);
            let Some(name) = child_name.as_deref() else { continue };
            if !value_kinds.contains(&name) {
                continue;
            }
            out.push(child);
        }
    }
    for child in xot.children(node) {
        collect_body_value_targets(xot, child, body_names, value_kinds, out);
    }
}

fn collect_expression_position_targets(
    xot: &Xot,
    node: XotNode,
    slot_names: &[&str],
    out: &mut Vec<XotNode>,
) {
    use helpers::*;
    if xot.element(node).is_none() {
        return;
    }
    let element_name = get_element_name(xot, node);
    let is_slot = element_name
        .as_deref()
        .map_or(false, |n| slot_names.contains(&n));
    if is_slot {
        // Wrap *every* element child of the slot. Most slots
        // (`value`, `condition`, `left`, `right`) hold a single
        // expression, but some — Python's `return 1, 2` after
        // expression_list flattening — hold a list of sibling
        // expressions, and each is its own expression position.
        for child in xot.children(node) {
            if xot.element(child).is_none() {
                continue;
            }
            let child_name = get_element_name(xot, child);
            // Skip if the child is already an <expression> host —
            // idempotent under repeat application.
            if child_name.as_deref() == Some("expression") {
                continue;
            }
            // Skip if the child is a `<type>` element AND the
            // surrounding slot sits inside a known type-only context
            // (Principle #14). The slot's grandparent kind
            // distinguishes type-only contexts (where T: Clone, alias
            // body, chan value-type, conditional-type LHS/RHS) from
            // value-position uses where the expression happens to
            // render as a type (TS `Array<number>` instantiation,
            // Java `String.class`). A bare `<type>` child only loses
            // its expression host when its grandparent confirms the
            // type-only context.
            if child_name.as_deref() == Some("type") {
                let grandparent_name = xot.parent(node)
                    .filter(|&p| xot.element(p).is_some())
                    .and_then(|p| get_element_name(xot, p));
                let in_type_context = matches!(
                    grandparent_name.as_deref(),
                    Some("bound")     // Rust where T: ...
                  | Some("alias")     // Python type alias body
                  | Some("chan")      // Go chan value-type
                  | Some("map")       // Go map value-type
                  | Some("type")      // TS conditional-type slots (grandparent is type[conditional])
                  | Some("instanceof") // Java instanceof RHS (the type checked against)
                );
                if in_type_context {
                    continue;
                }
            }
            out.push(child);
        }
    }
    for child in xot.children(node) {
        collect_expression_position_targets(xot, child, slot_names, out);
    }
}

/// Recursively walk and transform a node
fn walk_node<F>(xot: &mut Xot, node: XotNode, transform_fn: &mut F) -> Result<(), xot::Error>
where
    F: FnMut(&mut Xot, XotNode) -> Result<TransformAction, xot::Error>,
{
    // Skip non-element nodes
    if xot.element(node).is_none() {
        return Ok(());
    }

    // Apply transform to this node
    let action = transform_fn(xot, node)?;

    match action {
        TransformAction::Continue => {
            // Process children recursively
            let children: Vec<XotNode> = xot.children(node)
                .filter(|&c| xot.element(c).is_some())
                .collect();
            for child in children {
                walk_node(xot, child, transform_fn)?;
            }
        }
        TransformAction::Skip => {
            // Move children to parent, transform them, then remove this node
            let children: Vec<XotNode> = xot.children(node).collect();
            for child in children {
                xot.detach(child)?;
                xot.insert_before(node, child)?;
                if xot.element(child).is_some() {
                    walk_node(xot, child, transform_fn)?;
                }
            }
            xot.detach(node)?;
        }
        TransformAction::Flatten => {
            // Transform children first, then move them to parent and remove node
            let children: Vec<XotNode> = xot.children(node)
                .filter(|&c| xot.element(c).is_some())
                .collect();
            for child in children {
                walk_node(xot, child, transform_fn)?;
            }
            helpers::flatten_node(xot, node)?;
        }
        TransformAction::Done => {
            // Node fully handled, don't recurse
        }
    }

    Ok(())
}

// =============================================================================
// HELPERS - Low-level xot operations, no semantic meaning
// =============================================================================

pub mod helpers {
    use super::*;

    /// Fluent in-place mutation helpers for `Xot`.
    ///
    /// These keep transformation code at the semantic level while
    /// centralizing the low-level `xot` plumbing in one place.
    pub trait XotWithExt {
        fn with_attr(&mut self, node: XotNode, name: &str, value: &str) -> &mut Self;
        fn with_removed_attr(&mut self, node: XotNode, name: &str) -> &mut Self;
        fn with_source_location_from(&mut self, to: XotNode, from: XotNode) -> &mut Self;
        fn with_renamed<N: AsRef<str>>(&mut self, node: XotNode, new_name: N) -> &mut Self;
        fn with_marker<N: AsRef<str>>(&mut self, node: XotNode, name: N) -> Result<&mut Self, xot::Error>;
        fn with_detach(&mut self, node: XotNode) -> Result<&mut Self, xot::Error>;
        fn with_insert_before(&mut self, sibling: XotNode, node: XotNode) -> Result<&mut Self, xot::Error>;
        fn with_insert_after(&mut self, sibling: XotNode, node: XotNode) -> Result<&mut Self, xot::Error>;
        fn with_append(&mut self, parent: XotNode, child: XotNode) -> Result<&mut Self, xot::Error>;
        fn with_prepend(&mut self, parent: XotNode, child: XotNode) -> Result<&mut Self, xot::Error>;
        fn with_wrap_child(&mut self, child: XotNode, wrapper: XotNode) -> Result<&mut Self, xot::Error>;
        fn with_wrapped_field_child<N: AsRef<str>>(&mut self, parent: XotNode, field: &str, wrapper: N) -> Result<&mut Self, xot::Error>;
        fn with_prepended_empty_element<N: AsRef<str>>(&mut self, parent: XotNode, name: N) -> Result<&mut Self, xot::Error>;
        fn with_prepended_marker_from<N: AsRef<str>>(&mut self, parent: XotNode, name: N, source: XotNode) -> Result<&mut Self, xot::Error>;
        /// Shorthand for `with_prepended_marker_from(node, name, node)`
        /// — the marker takes its source location from the parent
        /// itself. Covers the ~85% case where the marker is a
        /// modifier of the renamed element (e.g. `<async/>` on the
        /// async function it modifies).
        fn with_prepended_marker<N: AsRef<str>>(&mut self, parent: XotNode, name: N) -> Result<&mut Self, xot::Error>;
        fn with_appended_empty_element<N: AsRef<str>>(&mut self, parent: XotNode, name: N) -> Result<&mut Self, xot::Error>;
        fn with_appended_marker_from<N: AsRef<str>>(&mut self, parent: XotNode, name: N, source: XotNode) -> Result<&mut Self, xot::Error>;
        /// Shorthand for `with_appended_marker_from(node, name, node)`.
        fn with_appended_marker<N: AsRef<str>>(&mut self, parent: XotNode, name: N) -> Result<&mut Self, xot::Error>;
        fn with_inserted_empty_before<N: AsRef<str>>(&mut self, sibling: XotNode, name: N) -> Result<&mut Self, xot::Error>;
        fn with_prepended_element_with_text<N: AsRef<str>>(&mut self, parent: XotNode, name: N, text: &str) -> Result<&mut Self, xot::Error>;
        fn with_inserted_text_after(&mut self, sibling: XotNode, text: &str) -> Result<&mut Self, xot::Error>;
        fn with_appended_text(&mut self, parent: XotNode, text: &str) -> Result<&mut Self, xot::Error>;
        fn with_detached_children(&mut self, node: XotNode) -> Result<&mut Self, xot::Error>;
        fn with_only_text(&mut self, node: XotNode, text: &str) -> Result<&mut Self, xot::Error>;
    }

    impl XotWithExt for Xot {
        fn with_attr(&mut self, node: XotNode, name: &str, value: &str) -> &mut Self {
            set_attr(self, node, name, value);
            self
        }

        fn with_removed_attr(&mut self, node: XotNode, name: &str) -> &mut Self {
            remove_attr(self, node, name);
            self
        }

        fn with_source_location_from(&mut self, to: XotNode, from: XotNode) -> &mut Self {
            copy_source_location(self, from, to);
            self
        }

        fn with_renamed<N: AsRef<str>>(&mut self, node: XotNode, new_name: N) -> &mut Self {
            rename(self, node, new_name);
            self
        }

        fn with_marker<N: AsRef<str>>(&mut self, node: XotNode, name: N) -> Result<&mut Self, xot::Error> {
            rename_to_marker(self, node, name)?;
            Ok(self)
        }

        fn with_detach(&mut self, node: XotNode) -> Result<&mut Self, xot::Error> {
            self.detach(node)?;
            Ok(self)
        }

        fn with_insert_before(&mut self, sibling: XotNode, node: XotNode) -> Result<&mut Self, xot::Error> {
            self.insert_before(sibling, node)?;
            Ok(self)
        }

        fn with_insert_after(&mut self, sibling: XotNode, node: XotNode) -> Result<&mut Self, xot::Error> {
            self.insert_after(sibling, node)?;
            Ok(self)
        }

        fn with_append(&mut self, parent: XotNode, child: XotNode) -> Result<&mut Self, xot::Error> {
            self.append(parent, child)?;
            Ok(self)
        }

        fn with_prepend(&mut self, parent: XotNode, child: XotNode) -> Result<&mut Self, xot::Error> {
            self.prepend(parent, child)?;
            Ok(self)
        }

        fn with_wrap_child(&mut self, child: XotNode, wrapper: XotNode) -> Result<&mut Self, xot::Error> {
            self.insert_before(child, wrapper)?;
            self.detach(child)?;
            self.append(wrapper, child)?;
            Ok(self)
        }

        fn with_wrapped_field_child<N: AsRef<str>>(&mut self, parent: XotNode, field: &str, wrapper: N) -> Result<&mut Self, xot::Error> {
            wrap_field_child(self, parent, field, wrapper)?;
            Ok(self)
        }

        fn with_prepended_empty_element<N: AsRef<str>>(&mut self, parent: XotNode, name: N) -> Result<&mut Self, xot::Error> {
            prepend_empty_element(self, parent, name)?;
            Ok(self)
        }

        fn with_prepended_marker_from<N: AsRef<str>>(&mut self, parent: XotNode, name: N, source: XotNode) -> Result<&mut Self, xot::Error> {
            prepend_marker_from(self, parent, name, source)?;
            Ok(self)
        }

        fn with_prepended_marker<N: AsRef<str>>(&mut self, parent: XotNode, name: N) -> Result<&mut Self, xot::Error> {
            prepend_marker_from(self, parent, name, parent)?;
            Ok(self)
        }

        fn with_appended_empty_element<N: AsRef<str>>(&mut self, parent: XotNode, name: N) -> Result<&mut Self, xot::Error> {
            append_empty_element(self, parent, name)?;
            Ok(self)
        }

        fn with_appended_marker_from<N: AsRef<str>>(&mut self, parent: XotNode, name: N, source: XotNode) -> Result<&mut Self, xot::Error> {
            append_marker_from(self, parent, name, source)?;
            Ok(self)
        }

        fn with_appended_marker<N: AsRef<str>>(&mut self, parent: XotNode, name: N) -> Result<&mut Self, xot::Error> {
            append_marker_from(self, parent, name, parent)?;
            Ok(self)
        }

        fn with_inserted_empty_before<N: AsRef<str>>(&mut self, sibling: XotNode, name: N) -> Result<&mut Self, xot::Error> {
            insert_empty_before(self, sibling, name)?;
            Ok(self)
        }

        fn with_prepended_element_with_text<N: AsRef<str>>(&mut self, parent: XotNode, name: N, text: &str) -> Result<&mut Self, xot::Error> {
            prepend_element_with_text(self, parent, name, text)?;
            Ok(self)
        }

        fn with_inserted_text_after(&mut self, sibling: XotNode, text: &str) -> Result<&mut Self, xot::Error> {
            insert_text_after(self, sibling, text)?;
            Ok(self)
        }

        fn with_appended_text(&mut self, parent: XotNode, text: &str) -> Result<&mut Self, xot::Error> {
            let text_node = self.new_text(text);
            self.append(parent, text_node)?;
            Ok(self)
        }

        fn with_detached_children(&mut self, node: XotNode) -> Result<&mut Self, xot::Error> {
            let children: Vec<XotNode> = self.children(node).collect();
            for child in children {
                self.detach(child)?;
            }
            Ok(self)
        }

        fn with_only_text(&mut self, node: XotNode, text: &str) -> Result<&mut Self, xot::Error> {
            self.with_detached_children(node)?;
            self.with_appended_text(node, text)
        }
    }

    /// Get the local name of an element node
    pub fn get_element_name(xot: &Xot, node: XotNode) -> Option<String> {
        xot.element(node).map(|element| {
            xot.local_name_str(element.name()).to_string()
        })
    }

    /// Get the original TreeSitter kind from the `kind` attribute
    /// This is the robust way to identify node types - it doesn't change after renames
    pub fn get_kind(xot: &Xot, node: XotNode) -> Option<String> {
        get_attr(xot, node, "kind")
    }

    /// Get or create a NameId for a name string
    pub fn get_name(xot: &mut Xot, name: impl AsRef<str>) -> NameId {
        xot.add_name(name.as_ref())
    }

    /// Rename an element node.
    ///
    /// The `field` attribute is always preserved — it carries the grammar-level
    /// singleton signal that the JSON serializer relies on for property lifting.
    /// If `field` matches the old element name, it is updated to the new name
    /// so that it stays in sync after the rename.
    pub fn rename(xot: &mut Xot, node: XotNode, new_name: impl AsRef<str>) {
        let new_name = new_name.as_ref();
        let old_name = get_element_name(xot, node);
        let name_id = xot.add_name(new_name);
        if let Some(element) = xot.element_mut(node) {
            element.set_name(name_id);
        }
        // Keep field in sync: if field matched the old name, update to new name
        if let Some(old) = old_name {
            if let Some(field_value) = get_attr(xot, node, "field") {
                if field_value == old {
                    set_attr(xot, node, "field", new_name);
                }
            }
        }
    }


    /// Set an attribute on an element
    pub fn set_attr(xot: &mut Xot, node: XotNode, name: &str, value: &str) {
        let name_id = xot.add_name(name);
        xot.attributes_mut(node).insert(name_id, value.to_string());
    }

    /// Get an attribute value from an element
    pub fn get_attr(xot: &Xot, node: XotNode, name: &str) -> Option<String> {
        let attrs = xot.attributes(node);
        for (name_id, value) in attrs.iter() {
            if xot.local_name_str(name_id) == name {
                return Some(value.to_string());
            }
        }
        None
    }

    /// Remove an attribute from an element
    pub fn remove_attr(xot: &mut Xot, node: XotNode, name: &str) {
        let mut to_remove = None;
        {
            let attrs = xot.attributes(node);
            for (name_id, _) in attrs.iter() {
                if xot.local_name_str(name_id) == name {
                    to_remove = Some(name_id);
                    break;
                }
            }
        }
        if let Some(name_id) = to_remove {
            xot.attributes_mut(node).remove(name_id);
        }
    }

    /// Get all text content from immediate children (for extracting operators, keywords)
    /// Filters out whitespace-only text nodes and trims the text content.
    pub fn get_text_children(xot: &Xot, node: XotNode) -> Vec<String> {
        xot.children(node)
            .filter_map(|child| {
                xot.text_str(child).and_then(|s| {
                    let trimmed = s.trim();
                    if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed.to_string())
                    }
                })
            })
            .collect()
    }

    /// Get all element children
    pub fn get_element_children(xot: &Xot, node: XotNode) -> Vec<XotNode> {
        xot.children(node)
            .filter(|&child| xot.element(child).is_some())
            .collect()
    }

    /// Check if node has any element children
    pub fn has_element_children(xot: &Xot, node: XotNode) -> bool {
        xot.children(node).any(|child| xot.element(child).is_some())
    }

    /// Get text content of a node (concatenated **direct** text
    /// children). Does NOT descend into element children — for the
    /// full source text of a subtree, use `descendant_text`.
    pub fn get_text_content(xot: &Xot, node: XotNode) -> Option<String> {
        let text: String = xot.children(node)
            .filter_map(|child| xot.text_str(child))
            .collect();
        if text.is_empty() { None } else { Some(text) }
    }

    /// Concatenate every text descendant of `node` in document order
    /// — the XPath string-value, basically. Use when a handler needs
    /// the full source text of a tree-sitter node (e.g. a
    /// `visibility_modifier` whose text "pub(crate)" is split across
    /// sibling tokens and a nested `<crate>` element).
    pub fn descendant_text(xot: &Xot, node: XotNode) -> String {
        let mut out = String::new();
        collect_descendant_text(xot, node, &mut out);
        out
    }

    /// Extract a numeric value from a position attribute.
    /// Position attributes are set by the xot builder from tree-sitter positions.
    /// E.g. `get_line(xot, node, "line")` on a node with `line="3"` returns `Some(3)`.
    pub fn get_line(xot: &Xot, node: XotNode, attr: &str) -> Option<usize> {
        get_attr(xot, node, attr)?
            .parse()
            .ok()
    }

    /// Check if a node starts on the same line as its previous element sibling ends.
    /// Useful for detecting inline/trailing constructs (e.g. trailing comments).
    /// Returns false if there is no previous element sibling or position data is missing.
    ///
    /// Note: `xot.preceding_siblings()` includes the node itself, so we skip it.
    pub fn is_inline_node(xot: &Xot, node: XotNode) -> bool {
        let start_line = match get_line(xot, node, "line") {
            Some(l) => l,
            None => return false,
        };

        let prev = xot.preceding_siblings(node)
            .filter(|&s| s != node)
            .find(|&s| xot.element(s).is_some());

        match prev {
            Some(prev) => {
                let prev_end_line = get_line(xot, prev, "end_line").unwrap_or(0);
                prev_end_line == start_line
            }
            None => false,
        }
    }

    /// Prepend an empty element as first child
    pub fn prepend_empty_element(xot: &mut Xot, parent: XotNode, name: impl AsRef<str>) -> Result<XotNode, xot::Error> {
        let name_id = xot.add_name(name.as_ref());
        let element = xot.new_element(name_id);
        xot.prepend(parent, element)?;
        Ok(element)
    }

    /// Prepend an empty marker as first child, copying source-location
    /// attributes (`line`/`column`/`end_line`/`end_column`) from
    /// `source`. Use when the marker is "tied to" a real source token
    /// (Principle #10) — e.g. `<async/>` for the `async` keyword,
    /// `<try/>` for `?`, `<await/>` for `await`. The `source` should
    /// be the element whose source range covers the keyword token;
    /// when the keyword is anonymous text, pass its containing element.
    pub fn prepend_marker_from(
        xot: &mut Xot,
        parent: XotNode,
        name: impl AsRef<str>,
        source: XotNode,
    ) -> Result<XotNode, xot::Error> {
        let marker = prepend_empty_element(xot, parent, name)?;
        copy_source_location(xot, source, marker);
        Ok(marker)
    }

    /// Append an empty marker as last child, copying source-location
    /// attributes from `source`. See [`prepend_marker_from`].
    pub fn append_marker_from(
        xot: &mut Xot,
        parent: XotNode,
        name: impl AsRef<str>,
        source: XotNode,
    ) -> Result<XotNode, xot::Error> {
        let marker = append_empty_element(xot, parent, name)?;
        copy_source_location(xot, source, marker);
        Ok(marker)
    }

    /// Insert an empty element before a sibling
    pub fn insert_empty_before(xot: &mut Xot, sibling: XotNode, name: impl AsRef<str>) -> Result<XotNode, xot::Error> {
        let name_id = xot.add_name(name.as_ref());
        let element = xot.new_element(name_id);
        xot.insert_before(sibling, element)?;
        Ok(element)
    }

    /// Prepend an element with text content as first child
    pub fn prepend_element_with_text(xot: &mut Xot, parent: XotNode, name: impl AsRef<str>, text: &str) -> Result<XotNode, xot::Error> {
        let name_id = xot.add_name(name.as_ref());
        let element = xot.new_element(name_id);
        let text_node = xot.new_text(text);
        xot.append(element, text_node)?;
        xot.prepend(parent, element)?;
        Ok(element)
    }

    /// Append an empty element as last child
    pub fn append_empty_element(xot: &mut Xot, parent: XotNode, name: impl AsRef<str>) -> Result<XotNode, xot::Error> {
        let name_id = xot.add_name(name.as_ref());
        let element = xot.new_element(name_id);
        xot.append(parent, element)?;
        Ok(element)
    }

    /// Append a marker element with optional flat children
    pub fn append_marker(xot: &mut Xot, parent: XotNode, name: &str, children: &[&str]) -> Result<XotNode, xot::Error> {
        let el = append_empty_element(xot, parent, name)?;
        for child in children {
            append_empty_element(xot, el, child)?;
        }
        Ok(el)
    }


    /// Detach a node from the tree
    pub fn detach(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
        xot.detach(node)
    }

    /// Move all children of a node to its parent, then remove the node
    pub fn flatten_node(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
        let children: Vec<XotNode> = xot.children(node).collect();
        for child in children {
            xot.detach(child)?;
            xot.insert_before(node, child)?;
        }
        xot.detach(node)?;
        Ok(())
    }

    /// Copy source location attributes from one node to another
    pub fn copy_source_location(xot: &mut Xot, from: XotNode, to: XotNode) {
        for attr in &["line", "column", "end_line", "end_column"] {
            if let Some(v) = get_attr(xot, from, attr) {
                set_attr(xot, to, attr, &v);
            }
        }
    }

    /// Get parent element (if any)
    pub fn get_parent(xot: &Xot, node: XotNode) -> Option<XotNode> {
        xot.parent(node).filter(|&p| xot.element(p).is_some())
    }

    /// Get following siblings that are elements
    pub fn get_following_siblings(xot: &Xot, node: XotNode) -> Vec<XotNode> {
        xot.following_siblings(node)
            .filter(|&s| xot.element(s).is_some())
            .collect()
    }



    /// Rename an element to a marker: renames, removes text children.
    /// Preserves `start`/`end` and `kind` attributes (source location for keyword-based markers).
    /// Rename `node` to `name` and strip its text children so the
    /// element is a true marker (`<public/>` not `<public>public</public>`).
    /// The source keyword, if any, should be re-inserted as a sibling
    /// by the caller via `insert_text_after` — this keeps markers
    /// genuinely empty (Principle #7) while the enclosing node's
    /// XPath string-value still includes the keyword for `-v value`.
    pub fn rename_to_marker(xot: &mut Xot, node: XotNode, name: impl AsRef<str>) -> Result<(), xot::Error> {
        rename(xot, node, name);
        remove_text_children(xot, node)?;
        Ok(())
    }

    /// Insert a text node immediately after `node` in its parent.
    /// Used after `rename_to_marker` to preserve the source keyword
    /// as a dangling sibling, so a class / function / ... whose XPath
    /// string-value is queried with `-v value` still contains the
    /// original `public` / `pub` / `async` token.
    pub fn insert_text_after(xot: &mut Xot, node: XotNode, text: &str) -> Result<(), xot::Error> {
        let text_node = xot.new_text(text);
        xot.insert_after(node, text_node)?;
        Ok(())
    }

    /// Replace the first child of `parent` whose tree-sitter `kind`
    /// matches one of `kinds` with a `<name>TEXT</name>` element
    /// holding that child's text. Siblings are untouched.
    ///
    /// Used to normalise the declared name of a generic-parameter-like
    /// construct (`type_parameter` in Java / TS / Rust) where the
    /// identifier is a sibling of other children (bounds, constraints)
    /// — the full-wrapper `inline_single_identifier` would wipe those
    /// siblings, and leaving the identifier alone would let it get
    /// re-wrapped to `<type><name>T</name></type>` by the per-language
    /// type rename.
    ///
    /// Returns `Ok(())` whether or not a match was found.
    pub fn replace_identifier_with_name_child(
        xot: &mut Xot,
        parent: XotNode,
        kinds: &[&str],
    ) -> Result<(), xot::Error> {
        let target = xot.children(parent).find(|&c| {
            xot.element(c).is_some()
                && get_kind(xot, c).as_deref().map_or(false, |k| kinds.contains(&k))
        });
        let target = match target {
            Some(t) => t,
            None => return Ok(()),
        };
        let text: Option<String> = xot
            .children(target)
            .find_map(|c| xot.text_str(c).map(|s| s.to_string()));
        let text = match text {
            Some(t) => t,
            None => return Ok(()),
        };
        let name_id = xot.add_name("name");
        let name_el = xot.new_element(name_id);
        xot.with_source_location_from(name_el, target);
        let text_node = xot.new_text(&text);
        xot.append(name_el, text_node)?;
        xot.insert_before(target, name_el)?;
        xot.detach(target)?;
        Ok(())
    }

    /// Wrap the direct text content of `node` in a `<name>` child element.
    ///
    /// `<type>Foo</type>` becomes `<type><name>Foo</name></type>`. No-op if
    /// the node has no direct text child. Used to unify the type vocabulary:
    /// every named `<type>` reference carries its name in a `<name>` child
    /// so queries like `//type[name='Foo']` work uniformly and the JSON
    /// serialisation is an object rather than a bare string (see design.md
    /// Principle #14 / namespace vocabulary).
    ///
    /// Collects and joins *all* direct text children (there may be several
    /// when the source had interleaved whitespace), and detaches them.
    /// Leaves any element children intact at their original positions.
    pub fn wrap_text_in_name(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
        let mut buf = String::new();
        let text_children: Vec<XotNode> = xot.children(node)
            .filter(|&c| xot.text_str(c).is_some())
            .collect();
        if text_children.is_empty() {
            return Ok(());
        }
        for child in &text_children {
            if let Some(t) = xot.text_str(*child) {
                buf.push_str(t);
            }
        }
        let trimmed = buf.trim().to_string();
        if trimmed.is_empty() {
            return Ok(());
        }
        // Remove the old text children.
        for child in text_children {
            xot.detach(child)?;
        }
        // Create <name>TEXT</name> and prepend it. JSON serializer
        // (post-iter-139) keys by element name "name" — no `field=`
        // needed.
        let name_id = xot.add_name("name");
        let name_el = xot.new_element(name_id);
        let text_node = xot.new_text(&trimmed);
        xot.append(name_el, text_node)?;
        xot.prepend(node, name_el)?;
        Ok(())
    }

    /// Distribute a `list="<name>"` attribute to every element child of `node`.
    ///
    /// Used with `TransformAction::Flatten` to implement Principle #12
    /// (Flat Lists): a purely-grouping wrapper is replaced by its children,
    /// which inherit a `list="<plural>"` attribute. Non-XML serializers
    /// (JSON/YAML) read the attribute as the JSON key and emit the
    /// children as an array — deterministically, regardless of cardinality
    /// (so a 1-arg call emits `"arguments": [{...}]` matching a 3-arg
    /// call's `"arguments": [{...}, {...}, {...}]`).
    ///
    /// Renamed from `distribute_list_to_children` in iter 145 to
    /// reflect the attribute it actually writes (`list=`, not
    /// `field=`). Tree-sitter's own `field=` attributes are preserved
    /// untouched (they survive into `--meta` debug output but are
    /// ignored by the JSON serializer).
    pub fn distribute_list_to_children(xot: &mut Xot, node: XotNode, field: impl AsRef<str>) {
        let field = field.as_ref();
        let children: Vec<XotNode> = xot.children(node)
            .filter(|&c| xot.element(c).is_some())
            .collect();
        for child in children {
            xot.with_attr(child, "list", field);
        }
    }

    /// Wrap the child of `parent` with `field="<field>"` in a new element
    /// named `wrapper`. Used for surgical field-wrapping that can't be a
    /// global `FIELD_WRAPPINGS` rule — for example, wrapping a ternary
    /// expression's `alternative` field in `<else>` while leaving the
    /// if-statement's `alternative` unwrapped (where `else_clause`
    /// already renames to `<else>` and a global wrap would double-nest).
    ///
    /// No-op if no matching child is found.
    pub fn wrap_field_child(
        xot: &mut Xot,
        parent: XotNode,
        field: &str,
        wrapper: impl AsRef<str>,
    ) -> Result<(), xot::Error> {
        let child = xot
            .children(parent)
            .filter(|&c| xot.element(c).is_some())
            .find(|&c| get_attr(xot, c, "field").as_deref() == Some(field));
        let child = match child {
            Some(c) => c,
            None => return Ok(()),
        };
        let wrapper = wrapper.as_ref();
        let wrapper_id = xot.add_name(wrapper);
        let wrapper_node = xot.new_element(wrapper_id);
        xot.with_source_location_from(wrapper_node, child)
            .with_wrap_child(child, wrapper_node)?;
        // Wrapper element name IS the JSON key (Principle #19); inner
        // child keeps its tree-sitter field= for --meta. JSON ignores
        // field=.
        Ok(())
    }


    /// Walk `node`'s descendants and append every text-node's content to `buf`.
    pub(crate) fn collect_descendant_text(xot: &Xot, node: XotNode, buf: &mut String) {
        for child in xot.children(node) {
            if let Some(text) = xot.text_str(child) {
                buf.push_str(text);
            } else if xot.element(child).is_some() {
                collect_descendant_text(xot, child, buf);
            }
        }
    }

    /// Remove all text children from a node
    pub fn remove_text_children(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
        let text_children: Vec<XotNode> = xot.children(node)
            .filter(|&child| xot.text_str(child).is_some())
            .collect();
        for child in text_children {
            xot.detach(child)?;
        }
        Ok(())
    }

    /// Convert a bare keyword statement into an empty container/marker.
    /// Tree-sitter emits the keyword (`return`, `break`, `continue`,
    /// `throw`, `pass`, `goto`, `fallthrough`, `next`, `redo`, `retry`,
    /// `yield`) as an anonymous text leaf inside the statement element
    /// — this leaks the grammar token into the output (Principle #2 /
    /// #13: `<break>break;</break>` instead of `<break/>`).
    ///
    /// Strategy: detach text children whose trimmed content (with any
    /// trailing `;` removed) equals `keyword`, then re-insert the
    /// keyword as a fresh text-node sibling immediately after `node`.
    /// Element children of `node` are untouched, so labelled
    /// `break LABEL;` and `return value;` are preserved. Same pattern
    /// as `rename_to_marker` + `insert_text_after` — see PHP / TS
    /// modifier handlers.
    ///
    /// The fresh sibling text keeps the parent's XPath string-value
    /// intact, so `query -v value` on the enclosing block still shows
    /// `{ return a + b; }` rather than `{ a + b; }`.
    pub fn strip_keyword_text(
        xot: &mut Xot,
        node: XotNode,
        keyword: &str,
    ) -> Result<(), xot::Error> {
        let text_children: Vec<XotNode> = xot
            .children(node)
            .filter(|&c| {
                xot.text_str(c)
                    .map(|s| s.trim().trim_end_matches(';').trim() == keyword)
                    .unwrap_or(false)
            })
            .collect();
        if text_children.is_empty() {
            return Ok(());
        }
        // Capture the original text (with any surrounding whitespace) so
        // a trailing space carries through to the sibling form.
        let original_text: String = text_children
            .iter()
            .filter_map(|&c| xot.text_str(c).map(|s| s.to_string()))
            .collect::<Vec<_>>()
            .join("");
        let text_with_trailing_space = if original_text.ends_with(char::is_whitespace) {
            original_text
        } else {
            format!("{} ", keyword)
        };
        for child in text_children {
            xot.detach(child)?;
        }
        let text_node = xot.new_text(&text_with_trailing_space);
        xot.insert_before(node, text_node)?;
        Ok(())
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use super::helpers::*;
    use super::data_keys::sanitize_xml_name;

    fn create_test_xot() -> (Xot, XotNode) {
        let mut xot = Xot::new();
        let root_name = xot.add_name("root");
        let root = xot.new_element(root_name);
        let doc = xot.new_document_with_element(root).unwrap();
        (xot, doc)
    }

    #[test]
    fn test_get_element_name() {
        let (xot, doc) = create_test_xot();
        let root = xot.document_element(doc).unwrap();
        assert_eq!(get_element_name(&xot, root), Some("root".to_string()));
    }

    #[test]
    fn test_rename() {
        let (mut xot, doc) = create_test_xot();
        let root = xot.document_element(doc).unwrap();
        rename(&mut xot, root, "renamed");
        assert_eq!(get_element_name(&xot, root), Some("renamed".to_string()));
    }

    #[test]
    fn test_set_and_get_attr() {
        let (mut xot, doc) = create_test_xot();
        let root = xot.document_element(doc).unwrap();
        set_attr(&mut xot, root, "op", "+");
        assert_eq!(get_attr(&xot, root, "op"), Some("+".to_string()));
    }

    #[test]
    fn test_walk_transform_continue() {
        let (mut xot, doc) = create_test_xot();
        let root = xot.document_element(doc).unwrap();

        // Add a child
        let child_name = xot.add_name("child");
        let child = xot.new_element(child_name);
        xot.append(root, child).unwrap();

        let mut visited = Vec::new();
        walk_transform(&mut xot, doc, |xot, node| {
            if let Some(name) = get_element_name(xot, node) {
                visited.push(name);
            }
            Ok(TransformAction::Continue)
        }).unwrap();

        assert_eq!(visited, vec!["root", "child"]);
    }

    #[test]
    fn test_sanitize_xml_name() {
        assert_eq!(sanitize_xml_name("foo"), "foo");
        assert_eq!(sanitize_xml_name("foo_bar"), "foo_bar");
        assert_eq!(sanitize_xml_name("foo-bar"), "foo-bar");
        assert_eq!(sanitize_xml_name("foo.bar"), "foo.bar");
        assert_eq!(sanitize_xml_name("123"), "_123");
        assert_eq!(sanitize_xml_name("key with spaces"), "key_with_spaces");
        assert_eq!(sanitize_xml_name(""), "_");
        assert_eq!(sanitize_xml_name("-hyphen"), "_-hyphen");
        assert_eq!(sanitize_xml_name("DB_HOST"), "DB_HOST");
        assert_eq!(sanitize_xml_name("a:b"), "a_b");
    }
}
