//! Semantic transformation for xot XML trees
//!
//! This module provides xot → xot transformations that convert raw AST XML
//! into semantic XML with cleaner element names, extracted attributes, etc.
//!
//! ## Architecture
//! ```text
//! AST → build_raw() → xot tree → transform_semantic() → XPath/render
//! ```
//!
//! The transformation walks the xot tree and applies language-specific rules:
//! - Rename elements (binary_expression → binary)
//! - Extract operators as attributes (op="+")
//! - Flatten nodes (promote children to parent)
//! - Skip nodes entirely
//! - Convert modifier wrappers to empty elements (<public/>)
//! - Classify identifiers as <name> or <type>

use xot::{Xot, Node as XotNode, NameId};
use std::collections::HashMap;
use crate::parser::transform::{LangTransforms, IdentifierKind};

/// Context passed through the transformation
pub struct TransformContext<'a> {
    /// Language-specific transformation rules
    pub transforms: &'a LangTransforms,
    /// Cache of name strings to NameIds (for efficient lookups)
    name_cache: HashMap<String, NameId>,
}

impl<'a> TransformContext<'a> {
    pub fn new(transforms: &'a LangTransforms) -> Self {
        Self {
            transforms,
            name_cache: HashMap::new(),
        }
    }

    /// Get or create a NameId for a name string
    fn get_or_create_name(&mut self, xot: &mut Xot, name: &str) -> NameId {
        if let Some(&id) = self.name_cache.get(name) {
            id
        } else {
            let id = xot.add_name(name);
            self.name_cache.insert(name.to_string(), id);
            id
        }
    }
}

// =============================================================================
// HELPER FUNCTIONS - Small reusable operations on xot nodes
// =============================================================================

/// Get the local name of an element node
pub fn get_element_name(xot: &Xot, node: XotNode) -> Option<String> {
    if let Some(element) = xot.element(node) {
        let name_id = element.name();
        Some(xot.local_name_str(name_id).to_string())
    } else {
        None
    }
}

/// Rename an element node
pub fn rename_element(xot: &mut Xot, node: XotNode, new_name: NameId) {
    if let Some(element) = xot.element_mut(node) {
        element.set_name(new_name);
    }
}

/// Get text content of a node (for leaf nodes)
pub fn get_text_content(xot: &Xot, node: XotNode) -> Option<String> {
    let mut text = String::new();
    for child in xot.children(node) {
        if let Some(t) = xot.text_str(child) {
            text.push_str(t);
        }
    }
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

/// Set an attribute on an element
pub fn set_attribute(xot: &mut Xot, node: XotNode, attr_name: NameId, value: &str) {
    xot.attributes_mut(node).insert(attr_name, value.to_string());
}

/// Get an attribute value from an element
pub fn get_attribute(xot: &Xot, node: XotNode, attr_name: &str) -> Option<String> {
    let attrs = xot.attributes(node);
    for (name_id, value) in attrs.iter() {
        let local = xot.local_name_str(name_id);
        if local == attr_name {
            return Some(value.to_string());
        }
    }
    None
}

/// Remove an attribute from an element
pub fn remove_attribute(xot: &mut Xot, node: XotNode, attr_name: &str) {
    let mut to_remove = None;
    {
        let attrs = xot.attributes(node);
        for (name_id, _) in attrs.iter() {
            let local = xot.local_name_str(name_id);
            if local == attr_name {
                to_remove = Some(name_id);
                break;
            }
        }
    }
    if let Some(name_id) = to_remove {
        xot.attributes_mut(node).remove(name_id);
    }
}

/// Check if a node has any element children
pub fn has_element_children(xot: &Xot, node: XotNode) -> bool {
    xot.children(node).any(|child| xot.element(child).is_some())
}

/// Get all element children of a node
pub fn element_children(xot: &Xot, node: XotNode) -> Vec<XotNode> {
    xot.children(node)
        .filter(|&child| xot.element(child).is_some())
        .collect()
}

/// Create a new empty element and insert it as first child
pub fn prepend_empty_element(xot: &mut Xot, parent: XotNode, name: NameId) -> Result<XotNode, xot::Error> {
    let element = xot.new_element(name);
    xot.prepend(parent, element)?;
    Ok(element)
}

/// Create a new empty element and insert it before a sibling
pub fn insert_empty_element_before(xot: &mut Xot, sibling: XotNode, name: NameId) -> Result<XotNode, xot::Error> {
    let element = xot.new_element(name);
    xot.insert_before(sibling, element)?;
    Ok(element)
}

/// Remove a node from the tree (does not delete, just detaches)
pub fn detach_node(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    xot.detach(node)
}

/// Move all children of a node to its parent (flatten)
pub fn flatten_node(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    // Collect children first to avoid mutation during iteration
    let children: Vec<XotNode> = xot.children(node).collect();

    // Move each child to before the node
    for child in children {
        xot.detach(child)?;
        xot.insert_before(node, child)?;
    }

    // Remove the now-empty node
    xot.detach(node)?;
    Ok(())
}

/// Delete a node and all its descendants
pub fn delete_node(xot: &mut Xot, node: XotNode) -> Result<(), xot::Error> {
    xot.detach(node)?;
    // Node will be garbage collected by xot
    Ok(())
}

// =============================================================================
// SEMANTIC TRANSFORM - Main transformation logic
// =============================================================================

/// Transform a raw xot tree into semantic form
///
/// This is the main entry point. It walks the tree and applies language-specific
/// transformation rules from `LangTransforms`.
pub fn transform_semantic(xot: &mut Xot, root: XotNode, transforms: &LangTransforms) -> Result<(), xot::Error> {
    let mut ctx = TransformContext::new(transforms);

    // Find the actual content root (skip Files/File wrapper if present)
    let content_root = find_content_root(xot, root);

    // Transform recursively
    transform_node(xot, content_root, &mut ctx)?;

    Ok(())
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
    if let Some(name) = get_element_name(xot, node) {
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

/// Transform a single node and its descendants
fn transform_node(xot: &mut Xot, node: XotNode, ctx: &mut TransformContext) -> Result<(), xot::Error> {
    // Skip non-element nodes
    if xot.element(node).is_none() {
        return Ok(());
    }

    let kind = match get_element_name(xot, node) {
        Some(k) => k,
        None => return Ok(()),
    };

    // Rule 1: Skip nodes entirely (remove from tree)
    if ctx.transforms.should_skip(&kind) {
        // Move children to parent, then remove this node
        let children: Vec<XotNode> = xot.children(node).collect();
        for child in children {
            xot.detach(child)?;
            xot.insert_before(node, child)?;
            // Transform the promoted child
            transform_node(xot, child, ctx)?;
        }
        xot.detach(node)?;
        return Ok(());
    }

    // Rule 2: Flatten nodes (promote children to parent, remove this node)
    // IMPORTANT: We must transform children BEFORE flattening, otherwise they won't be processed
    if ctx.transforms.should_flatten(&kind) {
        // First, transform all element children
        let children: Vec<XotNode> = xot.children(node)
            .filter(|&c| xot.element(c).is_some())
            .collect();
        for child in children {
            transform_node(xot, child, ctx)?;
        }
        // Then flatten (move children to parent, remove node)
        flatten_node(xot, node)?;
        return Ok(());
    }

    // Rule 3: Handle modifier wrappers (C# "modifier" nodes become empty elements)
    if ctx.transforms.is_modifier_wrapper(&kind) {
        // Get the text content which should be the modifier name
        if let Some(text) = get_text_content(xot, node) {
            let text = text.trim();
            if ctx.transforms.is_known_modifier(text) {
                // Rename this element to the modifier name and clear its children
                let new_name = ctx.get_or_create_name(xot, text);
                rename_element(xot, node, new_name);

                // Remove all children (text content)
                let children: Vec<XotNode> = xot.children(node).collect();
                for child in children {
                    xot.detach(child)?;
                }

                // Remove location attributes for cleaner output
                remove_attribute(xot, node, "start");
                remove_attribute(xot, node, "end");
                remove_attribute(xot, node, "field");

                return Ok(());
            }
        }
    }

    // Rule 4: Extract operators from expression nodes
    if ctx.transforms.should_extract_operator(&kind) {
        extract_operator_attr(xot, node, ctx)?;
    }

    // Rule 5: Extract keyword modifiers (let/const/var for JS/TS)
    if ctx.transforms.should_extract_keyword_modifier(&kind) {
        extract_keyword_modifiers(xot, node, ctx)?;
    }

    // Rule 6: Rename element based on mappings
    let new_name = ctx.transforms.rename_element(&kind);
    if new_name != kind {
        let name_id = ctx.get_or_create_name(xot, new_name);
        rename_element(xot, node, name_id);
    }

    // Rule 7: Classify identifiers as name or type
    // This happens after rename, so we check for "identifier", "type_identifier", "property_identifier"
    if matches!(kind.as_str(), "identifier" | "type_identifier" | "property_identifier") {
        classify_identifier(xot, node, ctx)?;
    }

    // Process children recursively
    // Collect children first to avoid issues with tree modification during iteration
    let children: Vec<XotNode> = xot.children(node)
        .filter(|&c| xot.element(c).is_some())
        .collect();

    for child in children {
        transform_node(xot, child, ctx)?;
    }

    Ok(())
}

/// Collect text node children as tokens
fn collect_text_tokens(xot: &Xot, node: XotNode) -> Vec<String> {
    xot.children(node)
        .filter_map(|child| xot.text_str(child).map(|s| s.to_string()))
        .collect()
}

/// Extract operator from expression node's text children and add as op attribute
fn extract_operator_attr(xot: &mut Xot, node: XotNode, ctx: &mut TransformContext) -> Result<(), xot::Error> {
    // Collect text tokens (operators, punctuation) from children
    let tokens = collect_text_tokens(xot, node);

    // Find the operator token (skip punctuation like parentheses, commas)
    let operator = tokens.iter()
        .find(|token| {
            !token.chars().all(|c| matches!(c, '(' | ')' | ',' | ';' | '{' | '}' | '[' | ']'))
        });

    if let Some(op) = operator {
        let op_attr = ctx.get_or_create_name(xot, "op");
        set_attribute(xot, node, op_attr, op);
    }

    Ok(())
}

/// Extract keyword modifiers (let/const/var) from text children and add as empty child elements
fn extract_keyword_modifiers(xot: &mut Xot, node: XotNode, ctx: &mut TransformContext) -> Result<(), xot::Error> {
    // Collect text tokens from children
    let tokens = collect_text_tokens(xot, node);

    // Find known modifiers in the text tokens
    let modifiers: Vec<&str> = tokens.iter()
        .map(|s| s.as_str())
        .filter(|token| ctx.transforms.is_known_modifier(token))
        .collect();

    // Insert modifier elements as first children
    // We insert in reverse order so they appear in the original order
    for modifier in modifiers.into_iter().rev() {
        let name = ctx.get_or_create_name(xot, modifier);
        prepend_empty_element(xot, node, name)?;
    }

    Ok(())
}

/// Classify an identifier node as <name> or <type> based on context
fn classify_identifier(xot: &mut Xot, node: XotNode, ctx: &mut TransformContext) -> Result<(), xot::Error> {
    // Get parent element to determine context
    let parent = match xot.parent(node) {
        Some(p) if xot.element(p).is_some() => p,
        _ => return Ok(()),
    };

    let parent_kind = get_element_name(xot, parent).unwrap_or_default();

    // Check if next sibling is a parameter list (indicates function/method name)
    let has_param_sibling = xot.following_siblings(node).any(|sib| {
        if let Some(name) = get_element_name(xot, sib) {
            matches!(name.as_str(), "parameter_list" | "parameters" | "formal_parameters" | "params")
        } else {
            false
        }
    });

    // Compute special context (e.g., namespace declaration in C#)
    let parent_chain: Vec<&str> = collect_parent_chain_names(xot, node);
    let special_context = ctx.transforms.compute_context(&parent_chain);

    // Use language-specific classifier
    let id_kind = (ctx.transforms.classify_identifier)(&parent_kind, has_param_sibling, special_context);

    // Rename based on classification
    let new_name = match id_kind {
        IdentifierKind::Name => "name",
        IdentifierKind::Type => "type",
    };

    let name_id = ctx.get_or_create_name(xot, new_name);
    rename_element(xot, node, name_id);

    Ok(())
}

/// Collect parent element names for context computation
fn collect_parent_chain_names(xot: &Xot, node: XotNode) -> Vec<&'static str> {
    // This is a bit awkward because we need static strings for the LangTransforms API
    // For now, return an empty vec - the context computation will need adjustment
    // to work with owned strings
    let _ = (xot, node);
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_rename_element() {
        let (mut xot, doc) = create_test_xot();
        let root = xot.document_element(doc).unwrap();
        let new_name = xot.add_name("renamed");
        rename_element(&mut xot, root, new_name);
        assert_eq!(get_element_name(&xot, root), Some("renamed".to_string()));
    }

    #[test]
    fn test_set_and_get_attribute() {
        let (mut xot, doc) = create_test_xot();
        let root = xot.document_element(doc).unwrap();
        let attr_name = xot.add_name("op");
        set_attribute(&mut xot, root, attr_name, "+");
        assert_eq!(get_attribute(&xot, root, "op"), Some("+".to_string()));
    }

    // Integration test using the actual xot pipeline
    #[test]
    fn test_xot_pipeline_typescript() {
        use crate::parser::parse_string_to_xot;
        use crate::output::{render_document, RenderOptions};

        let source = "let x = 1 + 2;";
        let result = parse_string_to_xot(source, "typescript", "<test>".to_string(), false).unwrap();

        // Render to XML string
        let options = RenderOptions::default();
        let xml = render_document(&result.xot, result.root, &options);

        // Check that transforms were applied:
        // 1. Element renamed: binary_expression -> binary
        assert!(xml.contains("<binary"), "binary_expression should be renamed to binary: {}", xml);
        // 2. Operator extracted as attribute
        assert!(xml.contains(r#"op="+""#), "operator should be extracted as op attribute: {}", xml);
        // 3. let modifier extracted
        assert!(xml.contains("<let"), "let should be extracted as modifier element: {}", xml);
    }

    #[test]
    fn test_xot_pipeline_raw_mode() {
        use crate::parser::parse_string_to_xot;
        use crate::output::{render_document, RenderOptions};

        let source = "let x = 1 + 2;";
        let result = parse_string_to_xot(source, "typescript", "<test>".to_string(), true).unwrap();

        // Render to XML string
        let options = RenderOptions::default();
        let xml = render_document(&result.xot, result.root, &options);

        // In raw mode, transforms should NOT be applied:
        // Element should still be binary_expression, not binary
        assert!(xml.contains("<binary_expression") || xml.contains("binary_expression"),
            "raw mode should keep binary_expression: {}", xml);
    }
}
