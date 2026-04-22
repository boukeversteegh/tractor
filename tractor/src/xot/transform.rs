//! Xot tree transformation infrastructure
//!
//! Provides a generic tree walker and low-level helpers for xot manipulation.
//! No assumptions about AST structure - each language defines its own transform logic.
//!
//! ## Architecture
//! ```text
//! AST → build_raw() → xot tree → apply_field_wrappings → walk_transform(lang_fn) → transformed tree
//! ```

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
/// source location and carries `field="Y"` itself. The inner element's
/// `field` attribute is rewritten to its own local-name so the JSON
/// serializer can reach it as a property of the wrapper.
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
        copy_source_location(xot, element, wrapper);
        set_attr(xot, wrapper, "field", &wrapper_name);

        // Rewrite the inner element's `field` to its own local-name so the
        // JSON serializer can treat the inner as a named property of the
        // wrapper. Preserves the semantics the old builder produced.
        let inner_local = xot
            .element(element)
            .map(|e| xot.local_name_str(e.name()).to_string());
        if let Some(local) = inner_local {
            set_attr(xot, element, "field", &local);
        }

        xot.insert_before(element, wrapper)?;
        xot.detach(element)?;
        xot.append(wrapper, element)?;
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
    pub fn get_name(xot: &mut Xot, name: &str) -> NameId {
        xot.add_name(name)
    }

    /// Rename an element node.
    ///
    /// The `field` attribute is always preserved — it carries the grammar-level
    /// singleton signal that the JSON serializer relies on for property lifting.
    /// If `field` matches the old element name, it is updated to the new name
    /// so that it stays in sync after the rename.
    pub fn rename(xot: &mut Xot, node: XotNode, new_name: &str) {
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

    /// Sanitize a string to be a valid XML element name.
    /// Replaces invalid characters with underscores.
    pub fn sanitize_xml_name(name: &str) -> String {
        if name.is_empty() {
            return "_".to_string();
        }

        let mut result = String::with_capacity(name.len());
        for (i, c) in name.chars().enumerate() {
            if i == 0 {
                if c.is_ascii_alphabetic() || c == '_' {
                    result.push(c);
                } else {
                    result.push('_');
                    if c.is_ascii_alphanumeric() || c == '-' || c == '.' {
                        result.push(c);
                    }
                }
            } else if c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' {
                result.push(c);
            } else {
                result.push('_');
            }
        }
        result
    }

    // /specs/tractor-parse/dual-view/data-branch/key-sanitization.md: Key Name Sanitization
    /// Rename an element to represent a data key.
    ///
    /// Sanitizes the key for use as an XML element name, renames the node,
    /// and stores the original key in a `key` attribute when sanitization
    /// was needed. Returns the sanitized name.
    pub fn rename_to_key(xot: &mut Xot, node: XotNode, key: &str) -> String {
        let safe_name = sanitize_xml_name(key);
        rename(xot, node, &safe_name);
        if safe_name != key {
            set_attr(xot, node, "key", key);
        }
        safe_name
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

    /// Get text content of a node (concatenated text children)
    pub fn get_text_content(xot: &Xot, node: XotNode) -> Option<String> {
        let text: String = xot.children(node)
            .filter_map(|child| xot.text_str(child))
            .collect();
        if text.is_empty() { None } else { Some(text) }
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
    pub fn prepend_empty_element(xot: &mut Xot, parent: XotNode, name: &str) -> Result<XotNode, xot::Error> {
        let name_id = xot.add_name(name);
        let element = xot.new_element(name_id);
        xot.prepend(parent, element)?;
        Ok(element)
    }

    /// Insert an empty element before a sibling
    pub fn insert_empty_before(xot: &mut Xot, sibling: XotNode, name: &str) -> Result<XotNode, xot::Error> {
        let name_id = xot.add_name(name);
        let element = xot.new_element(name_id);
        xot.insert_before(sibling, element)?;
        Ok(element)
    }

    /// Prepend an element with text content as first child
    pub fn prepend_element_with_text(xot: &mut Xot, parent: XotNode, name: &str, text: &str) -> Result<XotNode, xot::Error> {
        let name_id = xot.add_name(name);
        let element = xot.new_element(name_id);
        let text_node = xot.new_text(text);
        xot.append(element, text_node)?;
        xot.prepend(parent, element)?;
        Ok(element)
    }

    /// Append an empty element as last child
    pub fn append_empty_element(xot: &mut Xot, parent: XotNode, name: &str) -> Result<XotNode, xot::Error> {
        let name_id = xot.add_name(name);
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

    /// Prepend an `<op>` element with semantic markers and raw text
    pub fn prepend_op_element(xot: &mut Xot, parent: XotNode, op_text: &str) -> Result<XotNode, xot::Error> {
        let op_name = xot.add_name("op");
        let op_element = xot.new_element(op_name);
        add_operator_markers(xot, op_element, op_text)?;
        let text_node = xot.new_text(op_text);
        xot.append(op_element, text_node)?;
        xot.prepend(parent, op_element)?;
        Ok(op_element)
    }

    /// Add semantic marker children inside an `<op>` element based on operator text
    fn add_operator_markers(xot: &mut Xot, op: XotNode, text: &str) -> Result<(), xot::Error> {
        match text {
            // Equality
            "==" => { append_marker(xot, op, "equals", &[])?; }
            "===" => { append_marker(xot, op, "equals", &["strict"])?; }
            "!=" => { append_marker(xot, op, "not-equals", &[])?; }
            "!==" => { append_marker(xot, op, "not-equals", &["strict"])?; }
            // Comparison
            "<" => { append_marker(xot, op, "compare", &["less"])?; }
            ">" => { append_marker(xot, op, "compare", &["greater"])?; }
            "<=" => { append_marker(xot, op, "compare", &["less", "or-equal"])?; }
            ">=" => { append_marker(xot, op, "compare", &["greater", "or-equal"])?; }
            // Arithmetic
            "+" => { append_marker(xot, op, "plus", &[])?; }
            "-" => { append_marker(xot, op, "minus", &[])?; }
            "*" => { append_marker(xot, op, "multiply", &[])?; }
            "/" => { append_marker(xot, op, "divide", &[])?; }
            "%" => { append_marker(xot, op, "modulo", &[])?; }
            "**" => { append_marker(xot, op, "power", &[])?; }
            // Logical
            "&&" | "and" => { append_marker(xot, op, "logical", &["and"])?; }
            "||" | "or" => { append_marker(xot, op, "logical", &["or"])?; }
            "!" | "not" => { append_marker(xot, op, "logical", &["not"])?; }
            "??" => { append_marker(xot, op, "nullish-coalescing", &[])?; }
            // Bitwise
            "&" => { append_marker(xot, op, "bitwise", &["and"])?; }
            "|" => { append_marker(xot, op, "bitwise", &["or"])?; }
            "^" => { append_marker(xot, op, "bitwise", &["xor"])?; }
            "~" => { append_marker(xot, op, "bitwise", &["not"])?; }
            "<<" => { append_marker(xot, op, "shift", &["left"])?; }
            ">>" => { append_marker(xot, op, "shift", &["right"])?; }
            ">>>" => { append_marker(xot, op, "shift", &["right", "unsigned"])?; }
            // Assignment (bare = gets no marker — parent element disambiguates)
            // Compound assignment (arithmetic)
            "+=" => { append_marker(xot, op, "assign", &["plus"])?; }
            "-=" => { append_marker(xot, op, "assign", &["minus"])?; }
            "*=" => { append_marker(xot, op, "assign", &["multiply"])?; }
            "/=" => { append_marker(xot, op, "assign", &["divide"])?; }
            "%=" => { append_marker(xot, op, "assign", &["modulo"])?; }
            "**=" => { append_marker(xot, op, "assign", &["power"])?; }
            // Compound assignment (logical/bitwise/shift)
            "&&=" => { let a = append_marker(xot, op, "assign", &[])?; append_marker(xot, a, "logical", &["and"])?; }
            "||=" => { let a = append_marker(xot, op, "assign", &[])?; append_marker(xot, a, "logical", &["or"])?; }
            "??=" => { let a = append_marker(xot, op, "assign", &[])?; append_marker(xot, a, "nullish-coalescing", &[])?; }
            "<<=" => { let a = append_marker(xot, op, "assign", &[])?; append_marker(xot, a, "shift", &["left"])?; }
            ">>=" => { let a = append_marker(xot, op, "assign", &[])?; append_marker(xot, a, "shift", &["right"])?; }
            "&=" => { let a = append_marker(xot, op, "assign", &[])?; append_marker(xot, a, "bitwise", &["and"])?; }
            "|=" => { let a = append_marker(xot, op, "assign", &[])?; append_marker(xot, a, "bitwise", &["or"])?; }
            "^=" => { let a = append_marker(xot, op, "assign", &[])?; append_marker(xot, a, "bitwise", &["xor"])?; }
            // Python-specific
            "in" => { append_marker(xot, op, "contains", &[])?; }
            "not in" => { append_marker(xot, op, "contains", &["not"])?; }
            "is" => { append_marker(xot, op, "identity", &[])?; }
            "is not" => { append_marker(xot, op, "identity", &["not"])?; }
            // Unary prefix/postfix
            "++" => { append_marker(xot, op, "increment", &[])?; }
            "--" => { append_marker(xot, op, "decrement", &[])?; }
            // No marker — graceful degradation
            _ => {}
        }
        Ok(())
    }

    /// Check if an element name is an operator semantic marker
    pub fn is_operator_marker(name: &str) -> bool {
        matches!(name,
            "equals" | "not-equals" | "compare" | "less" | "greater" | "or-equal"
            | "plus" | "minus" | "multiply" | "divide" | "modulo" | "power"
            | "logical" | "bitwise" | "shift" | "nullish-coalescing"
            | "assign" | "increment" | "decrement"
            | "strict" | "left" | "right" | "unsigned" | "xor"
            | "contains" | "identity" | "not" | "and" | "or"
        )
    }

    /// Default singleton wrappers — wrappers that typically contain exactly
    /// one semantic child. Languages may adjust this list.
    pub const DEFAULT_SINGLETON_WRAPPERS: &[&str] = &[
        "value",        // assigned/initial value
        "left",         // binary left operand
        "right",        // binary right operand
        "condition",    // if/while/for condition
        "then",         // if true branch (flat-conditional shape)
        "returns",      // return type
        // Note: "body" and "else" excluded — "else" can either hold a block
        // (single child) or, for the non-C-like languages where the grammar
        // already produces flat alternatives, be unused. More importantly,
        // after transforms inline declaration lists, body can contain
        // multiple children. Lifting the first child would violate
        // cardinality-independence (issue #34).
    ];

    /// Mark the first element child of singleton wrappers with `field`.
    ///
    /// Singleton wrappers (e.g. `<value>`, `<returns>`) contain exactly one
    /// semantic child element. Adding `field="{element_name}"` to that child
    /// lets the JSON serializer lift it as a direct property instead of
    /// wrapping it in a `children` array.
    ///
    /// Call after language transforms with a language-specific wrapper list.
    pub fn lift_singleton_children(xot: &mut Xot, root: XotNode, singletons: &[&str]) {
        let all_elements = collect_all_elements(xot, root);
        for node in all_elements {
            let name = match get_element_name(xot, node) {
                Some(n) => n,
                None => continue,
            };
            if !singletons.contains(&name.as_str()) { continue; }
            // Must be a wrapper (has field attr, no kind attr)
            if get_attr(xot, node, "field").is_none() { continue; }
            if get_attr(xot, node, "kind").is_some() { continue; }
            // Add field to first element child if it doesn't already have one
            let children = get_element_children(xot, node);
            if let Some(&child) = children.first() {
                if get_attr(xot, child, "field").is_none() {
                    if let Some(child_name) = get_element_name(xot, child) {
                        set_attr(xot, child, "field", &child_name);
                    }
                }
            }
        }
    }

    /// Recursively collect all element nodes in document order
    fn collect_all_elements(xot: &Xot, node: XotNode) -> Vec<XotNode> {
        let mut result = Vec::new();
        collect_elements_recursive(xot, node, &mut result);
        result
    }

    fn collect_elements_recursive(xot: &Xot, node: XotNode, result: &mut Vec<XotNode>) {
        if xot.element(node).is_some() {
            result.push(node);
        }
        for child in xot.children(node) {
            collect_elements_recursive(xot, child, result);
        }
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

    /// Walk up the ancestor chain to find the nearest mapping pair that was
    /// renamed to its key name. Returns the key name for use as array item
    /// wrapper, enabling `//key[n]` instead of `//key/item[n]`.
    ///
    /// Returns `None` for top-level arrays (no named pair ancestor) or when
    /// the ancestor pair has a sanitized key (has `<key>` child).
    pub fn find_ancestor_key_name(xot: &Xot, node: XotNode) -> Option<String> {
        let mut current = xot.parent(node)?;
        loop {
            if let Some(kind) = get_kind(xot, current) {
                match kind.as_str() {
                    "block_mapping_pair" | "flow_pair" | "pair" => {
                        // Skip if key was sanitized (has key="..." attribute)
                        if get_attr(xot, current, "key").is_some() {
                            return None;
                        }
                        return get_element_name(xot, current);
                    }
                    _ => {}
                }
            }
            current = xot.parent(current)?;
        }
    }

    /// Check if a node has a sequence/array descendant (through wrapper nodes).
    /// Used by pair transforms to decide whether to Flatten for array values.
    pub fn has_sequence_child(xot: &Xot, node: XotNode) -> bool {
        for child in xot.children(node) {
            // Use kind attr for TreeSitter nodes, element name for TreeBuilder
            // wrappers (like <value>) which don't have a kind attr.
            let tag = get_kind(xot, child)
                .or_else(|| get_element_name(xot, child));
            if let Some(tag) = tag {
                match tag.as_str() {
                    "block_sequence" | "flow_sequence" | "array" => return true,
                    "block_node" | "flow_node" | "value" => {
                        if has_sequence_child(xot, child) { return true; }
                    }
                    _ => {}
                }
            }
        }
        false
    }

    /// Promote a `field` attribute to a wrapper element.
    ///
    /// Given `<identifier field="function">require</identifier>`, produces
    /// `<function><identifier>require</identifier></function>`.
    ///
    /// - Creates a wrapper element named after the field value
    /// - Copies source location attributes to the wrapper
    /// - Marks the wrapper with `field` for JSON property lifting
    /// - Removes `field` from the inner element
    /// - Returns the wrapper node, or `None` if the node has no matching field
    ///
    /// The caller specifies which field values to promote (e.g., `&["function", "object", "property"]`).
    pub fn promote_field_to_wrapper(
        xot: &mut Xot,
        node: XotNode,
        fields: &[&str],
    ) -> Result<Option<XotNode>, xot::Error> {
        let field_value = match get_attr(xot, node, "field") {
            Some(f) if fields.contains(&f.as_str()) => f,
            _ => return Ok(None),
        };

        // Create wrapper element
        let wrapper_name = get_name(xot, &field_value);
        let wrapper = xot.new_element(wrapper_name);

        // Copy source location to wrapper
        copy_source_location(xot, node, wrapper);

        // Mark wrapper as field-backed for JSON property lifting
        set_attr(xot, wrapper, "field", &field_value);

        // Remove field from inner element
        remove_attr(xot, node, "field");

        // Insert wrapper where node is, then move node inside
        xot.insert_before(node, wrapper)?;
        xot.detach(node)?;
        xot.append(wrapper, node)?;

        Ok(Some(wrapper))
    }

    /// Rename an element to a marker: renames, removes text children.
    /// Preserves `start`/`end` and `kind` attributes (source location for keyword-based markers).
    pub fn rename_to_marker(xot: &mut Xot, node: XotNode, name: &str) -> Result<(), xot::Error> {
        rename(xot, node, name);
        remove_text_children(xot, node)?;
        Ok(())
    }

    /// Replace the first child of `parent` whose tree-sitter `kind`
    /// matches one of `kinds` with a `<name field="name">TEXT</name>`
    /// element holding that child's text. Siblings are untouched.
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
        set_attr(xot, name_el, "field", "name");
        copy_source_location(xot, target, name_el);
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
        // Create <name field="name">TEXT</name> and prepend it. The
        // field attribute makes JSON/YAML serialisers lift it as a
        // named property (`"name": "TEXT"`) rather than a nested child.
        let name_id = xot.add_name("name");
        let name_el = xot.new_element(name_id);
        set_attr(xot, name_el, "field", "name");
        let text_node = xot.new_text(&trimmed);
        xot.append(name_el, text_node)?;
        xot.prepend(node, name_el)?;
        Ok(())
    }

    /// Distribute a `field=<name>` attribute to every element child of `node`.
    ///
    /// Used with `TransformAction::Flatten` to implement Principle #12
    /// (Flat Lists): a purely-grouping wrapper is replaced by its children,
    /// which inherit a `field="<plural>"` attribute so non-XML serializers
    /// (JSON/YAML) can collect same-field siblings into an array.
    pub fn distribute_field_to_children(xot: &mut Xot, node: XotNode, field: &str) {
        let children: Vec<XotNode> = xot.children(node)
            .filter(|&c| xot.element(c).is_some())
            .collect();
        for child in children {
            set_attr(xot, child, "field", field);
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
        wrapper: &str,
    ) -> Result<(), xot::Error> {
        let child = xot
            .children(parent)
            .filter(|&c| xot.element(c).is_some())
            .find(|&c| get_attr(xot, c, "field").as_deref() == Some(field));
        let child = match child {
            Some(c) => c,
            None => return Ok(()),
        };
        let wrapper_id = xot.add_name(wrapper);
        let wrapper_node = xot.new_element(wrapper_id);
        copy_source_location(xot, child, wrapper_node);
        set_attr(xot, wrapper_node, "field", wrapper);
        // Rewrite inner's field to its own local-name for JSON lifting.
        let inner_local = xot
            .element(child)
            .map(|e| xot.local_name_str(e.name()).to_string());
        if let Some(local) = inner_local {
            set_attr(xot, child, "field", &local);
        }
        xot.insert_before(child, wrapper_node)?;
        xot.detach(child)?;
        xot.append(wrapper_node, child)?;
        Ok(())
    }

    /// Rewrite a tree-sitter `generic_type` node into the canonical
    /// shape shared across languages:
    ///
    /// ```text
    /// <type>
    ///   <generic/>
    ///   Name
    ///   <type field="arguments">Arg1</type>
    ///   <type field="arguments">Arg2</type>
    /// </type>
    /// ```
    ///
    /// The `name_kinds` slice is the list of tree-sitter kinds to accept
    /// as the simple type name (typically `type_identifier` and
    /// `identifier`). Any child whose `kind` matches is collapsed to plain
    /// text; a `<name>` field wrapper around a simple name is also
    /// collapsed. Non-simple names (qualified identifiers, etc.) are left
    /// as a `<name>` sub-element so they remain queryable.
    pub fn rewrite_generic_type(
        xot: &mut Xot,
        node: XotNode,
        name_kinds: &[&str],
    ) -> Result<(), xot::Error> {
        let children: Vec<XotNode> = xot.children(node).collect();
        for child in children {
            let child_name = match get_element_name(xot, child) {
                Some(n) => n,
                None => continue,
            };

            // Case 1: the name is a <name> field-wrapper created by the
            // builder. If it wraps a single simple identifier, normalise
            // it to `<name>TEXT</name>` (collapse any inner identifier
            // element into plain text).
            if child_name == "name" {
                let grandchildren: Vec<XotNode> = xot.children(child).collect();
                let is_simple = !grandchildren.is_empty()
                    && grandchildren.iter().all(|&gc| {
                        match get_kind(xot, gc).as_deref() {
                            Some(k) => name_kinds.contains(&k),
                            None => xot.text_str(gc).is_some(),
                        }
                    });
                if is_simple {
                    let mut buf = String::new();
                    collect_descendant_text(xot, child, &mut buf);
                    if !buf.is_empty() {
                        // Replace any children with a single text node.
                        for gc in grandchildren {
                            xot.detach(gc)?;
                        }
                        let text_node = xot.new_text(&buf);
                        xot.append(child, text_node)?;
                    }
                }
                break;
            }

            // Case 2: the name is a bare identifier child with no field
            // wrapper (happens when the grammar doesn't tag it as a
            // field). Wrap in a `<name>` element.
            if let Some(kind) = get_kind(xot, child) {
                if name_kinds.contains(&kind.as_str()) {
                    let text_owned: Option<String> = xot
                        .children(child)
                        .find_map(|c| xot.text_str(c).map(|s| s.to_string()));
                    if let Some(text) = text_owned {
                        let name_id = xot.add_name("name");
                        let name_el = xot.new_element(name_id);
                        let text_node = xot.new_text(&text);
                        xot.append(name_el, text_node)?;
                        xot.insert_before(child, name_el)?;
                        xot.detach(child)?;
                    }
                    break;
                }
            }
        }
        prepend_empty_element(xot, node, "generic")?;
        rename(xot, node, "type");
        Ok(())
    }

    /// Walk `node`'s descendants and append every text-node's content to `buf`.
    fn collect_descendant_text(xot: &Xot, node: XotNode, buf: &mut String) {
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

    // /specs/tractor-parse/semantic-tree/transformations.md: Conditional shape
    /// Collapse a nested `else`/`if` chain under `if_node` into the flat
    /// conditional shape `<if>[condition][then][else_if*][else?]`.
    ///
    /// Applies after names have been rewritten, so children are already
    /// `<if>` / `<else>` / `<else_if>` / `<elsif>` (the raw Ruby kind).
    /// Handles two input shapes:
    ///
    /// - **C-like** (JS/TS, Java, C#, Go, Rust) — the `<else>` field
    ///   wrapper holds a renamed `else_clause` (itself `<else>`). If the
    ///   inner `<else>` contains a single `<if>`, that `<if>`'s
    ///   condition and then become a new `<else_if>` sibling; the
    ///   nested `<if>`'s own `<else>` chain continues. Final `<else>`
    ///   with a plain block stays.
    /// - **Ruby** — the grammar already emits `<elsif>` (nested) and
    ///   a final `<else>`. The `<elsif>` is renamed to `<else_if>` and
    ///   lifted out so it becomes a sibling of the outer `<if>`'s
    ///   condition/then; the same for any nested `<else>`.
    pub fn collapse_else_if_chain(xot: &mut Xot, if_node: XotNode) -> Result<(), xot::Error> {
        // Walk the `<else>` / `<elsif>` chain, lifting each level out
        // of the previous one so they become flat children of
        // `if_node`. `current` is the node we scan for a trailing
        // alternative (initially `if_node`; later each lifted
        // `<else_if>`). `anchor` is the child of `if_node` that the
        // next lifted alternative should be inserted *after* — None
        // means "append at the end" (modulo trailing text). Before
        // each step we normalize the C-like `<else>` wrapper around a
        // renamed `else_clause` (also `<else>`) into a single `<else>`
        // child.
        let mut current = if_node;
        let mut anchor: Option<XotNode> = None;
        loop {
            // Find the trailing alternative child (else / elsif / else_if)
            // on the current node.
            let alt = match find_trailing_alternative(xot, current) {
                Some(a) => a,
                None => break,
            };
            let alt_name = get_element_name(xot, alt).unwrap_or_default();

            match alt_name.as_str() {
                "else" => {
                    // Before finishing, check whether this `<else>` holds
                    // only a single `<if>` (else if in C-like shape).
                    // The `<else>` wraps an `<if>` directly (C-like else-if chain).
                    let inner_if = single_if_child(xot, alt);
                    if let Some(inner_if) = inner_if {
                        // Lift this `<if>`'s condition/then as a new
                        // `<else_if>` child of `if_node`, inserted
                        // immediately after `anchor` (so the chain stays
                        // in source order, even when the outer `<if>`
                        // has trailing text like Ruby's "end" keyword).
                        let else_if = lift_if_as_else_if(xot, if_node, anchor, inner_if)?;
                        xot.detach(alt)?; // drop the now-empty <else>
                        current = else_if;
                        anchor = Some(else_if);
                        continue;
                    }
                    // Terminal <else>: move it to be a child of if_node,
                    // positioned after `anchor` (Ruby nests else deep
                    // inside the elsif chain).
                    reparent_in(xot, alt, if_node, anchor)?;
                    break;
                }
                "elsif" | "else_if" => {
                    // Ruby's <elsif> (or any previously-renamed <else_if>).
                    // Rename and lift to be a child of if_node,
                    // positioned after `anchor`.
                    rename(xot, alt, "else_if");
                    reparent_in(xot, alt, if_node, anchor)?;
                    current = alt;
                    anchor = Some(alt);
                }
                _ => break,
            }
        }

        Ok(())
    }

    /// Return the last element child of `node` whose name is `else`,
    /// `elsif`, or `else_if` — the tail of the conditional chain.
    fn find_trailing_alternative(xot: &Xot, node: XotNode) -> Option<XotNode> {
        let children = get_element_children(xot, node);
        let last = *children.last()?;
        match get_element_name(xot, last).as_deref() {
            Some("else") | Some("elsif") | Some("else_if") => Some(last),
            _ => None,
        }
    }

    /// If `else_node` has exactly one element child and that child is
    /// `<if>`, return it. Used to detect the "else if" C-like shape.
    fn single_if_child(xot: &Xot, else_node: XotNode) -> Option<XotNode> {
        let children = get_element_children(xot, else_node);
        if children.len() != 1 {
            return None;
        }
        let only = children[0];
        if get_element_name(xot, only).as_deref() == Some("if") {
            Some(only)
        } else {
            None
        }
    }

    /// Build an `<else_if>` from `inner_if`'s condition/then and place
    /// it as a child of `outer_if`, positioned after `after` (or at
    /// the end of `outer_if` when `after` is `None`). The inner
    /// `<if>`'s own `<else>` / `<elsif>` chain is moved into the new
    /// `<else_if>` so the caller can continue iterating. Returns the
    /// new `<else_if>` node.
    fn lift_if_as_else_if(
        xot: &mut Xot,
        outer_if: XotNode,
        after: Option<XotNode>,
        inner_if: XotNode,
    ) -> Result<XotNode, xot::Error> {
        let else_if_name = xot.add_name("else_if");
        let else_if = xot.new_element(else_if_name);
        copy_source_location(xot, inner_if, else_if);

        // Insert the new `<else_if>` as a child of `outer_if`, placed
        // right after `after` so the chain reads in source order.
        match after {
            Some(a) => {
                let next = xot.next_sibling(a);
                match next {
                    Some(n) => xot.insert_before(n, else_if)?,
                    None => xot.append(outer_if, else_if)?,
                }
            }
            None => xot.append(outer_if, else_if)?,
        }

        // Move condition and then children from the inner <if> to the
        // new <else_if>.
        let inner_children = get_element_children(xot, inner_if);
        for child in inner_children {
            let name = get_element_name(xot, child).unwrap_or_default();
            match name.as_str() {
                "condition" | "then" => {
                    xot.detach(child)?;
                    xot.append(else_if, child)?;
                }
                _ => {}
            }
        }

        // The inner <if>'s remaining alternative children (its own
        // <else> / <elsif> chain) now belong semantically to the new
        // <else_if>'s tail. Move them under `else_if` so the caller
        // can continue iterating via `find_trailing_alternative`.
        let remaining = get_element_children(xot, inner_if);
        for child in remaining {
            let name = get_element_name(xot, child).unwrap_or_default();
            if matches!(name.as_str(), "else" | "elsif" | "else_if") {
                xot.detach(child)?;
                xot.append(else_if, child)?;
            }
        }

        Ok(else_if)
    }

    /// Detach `node` from its current parent and place it as a child
    /// of `parent`, positioned immediately after `after` when set, or
    /// at the end of `parent` when `after` is `None`. No-op if `node`
    /// is already positioned correctly.
    fn reparent_in(
        xot: &mut Xot,
        node: XotNode,
        parent: XotNode,
        after: Option<XotNode>,
    ) -> Result<(), xot::Error> {
        // Already in the right position?
        if xot.parent(node) == Some(parent) {
            match after {
                Some(a) if xot.next_sibling(a) == Some(node) => return Ok(()),
                None if xot.next_sibling(node).is_none() => return Ok(()),
                _ => {}
            }
        }
        xot.detach(node)?;
        match after {
            Some(a) => {
                let next = xot.next_sibling(a);
                match next {
                    Some(n) => xot.insert_before(n, node)?,
                    None => xot.append(parent, node)?,
                }
            }
            None => xot.append(parent, node)?,
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::helpers::*;

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
