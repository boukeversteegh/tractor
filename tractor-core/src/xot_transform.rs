//! Xot tree transformation infrastructure
//!
//! Provides a generic tree walker and low-level helpers for xot manipulation.
//! No assumptions about AST structure - each language defines its own transform logic.
//!
//! ## Architecture
//! ```text
//! AST → build_raw() → xot tree → walk_transform(lang_fn) → transformed tree
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
    // Find the actual content root (skip document/Files/File wrappers)
    let content_root = find_content_root(xot, root);
    walk_node(xot, content_root, &mut transform_fn)
}

/// Walk and transform starting from a specific node (no wrapper skipping).
///
/// Use this when transforming a detached subtree that isn't wrapped in
/// Files/File elements (e.g., a cloned content root for dual-branch assembly).
pub fn walk_transform_node<F>(xot: &mut Xot, node: XotNode, mut transform_fn: F) -> Result<(), xot::Error>
where
    F: FnMut(&mut Xot, XotNode) -> Result<TransformAction, xot::Error>,
{
    walk_node(xot, node, &mut transform_fn)
}

/// Find the actual content root, skipping Files/File wrappers
fn find_content_root(xot: &Xot, node: XotNode) -> XotNode {
    // If this is a document node, get the document element
    if xot.is_document(node) {
        if let Ok(elem) = xot.document_element(node) {
            return find_content_root(xot, elem);
        }
    }

    // Check if this is a Files or File wrapper
    if let Some(name) = helpers::get_element_name(xot, node) {
        if name == "Files" || name == "File" {
            // Return the first element child
            for child in xot.children(node) {
                if xot.element(child).is_some() {
                    return find_content_root(xot, child);
                }
            }
        }
    }

    node
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

    /// Rename an element node
    /// Also removes redundant `field` attribute if it matches the new name
    pub fn rename(xot: &mut Xot, node: XotNode, new_name: &str) {
        let name_id = xot.add_name(new_name);
        if let Some(element) = xot.element_mut(node) {
            element.set_name(name_id);
        }
        // Remove redundant field attribute if it matches the new element name
        if let Some(field_value) = get_attr(xot, node, "field") {
            if field_value == new_name {
                remove_attr(xot, node, "field");
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

    /// Get text content of a node (concatenated text children)
    pub fn get_text_content(xot: &Xot, node: XotNode) -> Option<String> {
        let text: String = xot.children(node)
            .filter_map(|child| xot.text_str(child))
            .collect();
        if text.is_empty() { None } else { Some(text) }
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
                        // Skip if key was sanitized (has <key> child element)
                        let has_key_child = xot.children(current)
                            .any(|c| get_element_name(xot, c).as_deref() == Some("key"));
                        if has_key_child {
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
}
