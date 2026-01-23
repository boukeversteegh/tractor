//! Build xot XML documents from TreeSitter AST
//!
//! This module provides a unified pipeline:
//! TreeSitter AST -> xot::Xot document -> colored string output
//!
//! There are two builders:
//! - XotBuilder: Builds into a standalone xot::Xot instance
//! - XeeBuilder: Builds into xee-xpath's Documents for direct XPath querying

#[cfg(feature = "native")]
use tree_sitter::Node as TsNode;
use xot::{Xot, Node as XotNode, NameId};
use std::collections::HashMap;

#[cfg(feature = "wasm")]
use crate::wasm_ast::SerializedNode;

/// Builder for creating xot documents from TreeSitter AST
pub struct XotBuilder {
    xot: Xot,
    /// Cache of name strings to NameIds
    name_cache: HashMap<String, NameId>,
}

impl XotBuilder {
    /// Create a new XotBuilder
    pub fn new() -> Self {
        XotBuilder {
            xot: Xot::new(),
            name_cache: HashMap::new(),
        }
    }

    /// Get or create a NameId for the given name
    fn get_name(&mut self, name: &str) -> NameId {
        if let Some(&id) = self.name_cache.get(name) {
            id
        } else {
            let id = self.xot.add_name(name);
            self.name_cache.insert(name.to_string(), id);
            id
        }
    }

    /// Build an xot document from TreeSitter AST (raw mode)
    #[cfg(feature = "native")]
    pub fn build_raw(
        &mut self,
        ts_node: TsNode,
        source: &str,
        file_path: &str,
    ) -> Result<XotNode, xot::Error> {
        // Create Files root element
        let files_name = self.get_name("Files");
        let files_el = self.xot.new_element(files_name);

        // Create File element with path attribute
        let file_name = self.get_name("File");
        let file_el = self.xot.new_element(file_name);

        let path_attr = self.get_name("path");
        self.xot.attributes_mut(file_el).insert(path_attr, file_path.to_string());

        // Build the tree from TreeSitter
        self.build_raw_node(ts_node, source, file_el, None)?;

        // Assemble document
        self.xot.append(files_el, file_el)?;
        let doc = self.xot.new_document_with_element(files_el)?;

        Ok(doc)
    }

    /// Fields that should be wrapped in semantic elements
    const WRAPPED_FIELDS: &'static [&'static str] = &[
        "name",        // variable/function/class name
        "value",       // assigned/initial value
        "left",        // binary expression left operand
        "right",       // binary expression right operand
        "body",        // function/class/loop body
        "parameters",  // function parameters
        "condition",   // if/while/for condition
        "consequence", // if true branch
        "alternative", // if else branch
        "returns",     // return type
        "arguments",   // call arguments
    ];

    /// Check if a field should be wrapped in a semantic element
    fn should_wrap_field(field: &str) -> bool {
        Self::WRAPPED_FIELDS.contains(&field)
    }

    /// Recursively build xot nodes from TreeSitter node (raw mode)
    #[cfg(feature = "native")]
    fn build_raw_node(
        &mut self,
        ts_node: TsNode,
        source: &str,
        parent: XotNode,
        field_name: Option<&str>,
    ) -> Result<(), xot::Error> {
        // Skip anonymous nodes (punctuation, etc.)
        if !ts_node.is_named() {
            return Ok(());
        }

        let kind = ts_node.kind();
        let elem_name = self.get_name(kind);
        let element = self.xot.new_element(elem_name);

        // Add kind attribute (original TreeSitter kind) for robust transform detection
        let kind_attr = self.get_name("kind");
        self.xot.attributes_mut(element).insert(kind_attr, kind.to_string());

        // Add location attributes
        let start = ts_node.start_position();
        let end = ts_node.end_position();

        let start_attr = self.get_name("start");
        let end_attr = self.get_name("end");

        self.xot.attributes_mut(element).insert(
            start_attr,
            format!("{}:{}", start.row + 1, start.column + 1),
        );
        self.xot.attributes_mut(element).insert(
            end_attr,
            format!("{}:{}", end.row + 1, end.column + 1),
        );

        // Check if leaf node (no named children)
        let named_child_count = ts_node.named_child_count();

        if named_child_count == 0 {
            // Leaf node - add text content
            if let Ok(text) = ts_node.utf8_text(source.as_bytes()) {
                let text_node = self.xot.new_text(text);
                self.xot.append(element, text_node)?;
            }
        } else {
            // Non-leaf: iterate ALL children (named and anonymous) to preserve order
            // Anonymous nodes become text children, named nodes become element children
            // We also capture inter-node whitespace from the source to preserve spacing
            let mut cursor = ts_node.walk();
            cursor.goto_first_child();
            let mut last_end_byte = ts_node.start_byte();

            loop {
                let child = cursor.node();
                let child_start = child.start_byte();

                // Add any whitespace/content between the last node and this one
                if child_start > last_end_byte {
                    let gap = &source[last_end_byte..child_start];
                    if !gap.is_empty() && gap.chars().any(|c| c.is_whitespace()) {
                        // Normalize the gap to a single space if it contains any whitespace
                        let text_node = self.xot.new_text(" ");
                        self.xot.append(element, text_node)?;
                    }
                }

                if child.is_named() {
                    let child_field = cursor.field_name();
                    self.build_raw_node(child, source, element, child_field)?;
                } else {
                    // Anonymous node - add as text child (operators, keywords, punctuation)
                    if let Ok(text) = child.utf8_text(source.as_bytes()) {
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            let text_node = self.xot.new_text(trimmed);
                            self.xot.append(element, text_node)?;
                        }
                    }
                }

                last_end_byte = child.end_byte();

                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }

        // Wrap in field element if needed, or just append directly
        if let Some(field) = field_name {
            if Self::should_wrap_field(field) {
                // Create wrapper element with field name
                let wrapper_name = self.get_name(field);
                let wrapper = self.xot.new_element(wrapper_name);

                // Copy location attributes from child to wrapper
                let start_attr = self.get_name("start");
                let end_attr = self.get_name("end");
                if let Some(start_val) = self.xot.attributes(element).get(start_attr).cloned() {
                    self.xot.attributes_mut(wrapper).insert(start_attr, start_val);
                }
                if let Some(end_val) = self.xot.attributes(element).get(end_attr).cloned() {
                    self.xot.attributes_mut(wrapper).insert(end_attr, end_val);
                }

                self.xot.append(wrapper, element)?;
                self.xot.append(parent, wrapper)?;
            } else {
                // Add field as attribute instead
                let field_attr = self.get_name("field");
                self.xot.attributes_mut(element).insert(field_attr, field.to_string());
                self.xot.append(parent, element)?;
            }
        } else {
            self.xot.append(parent, element)?;
        }

        Ok(())
    }

    // =========================================================================
    // WASM support - build from serialized AST
    // =========================================================================

    /// Build an xot document from a serialized AST (for WASM)
    #[cfg(feature = "wasm")]
    pub fn build_raw_from_serialized(
        &mut self,
        node: &SerializedNode,
        source: &str,
        file_path: &str,
    ) -> Result<XotNode, xot::Error> {
        // Create Files root element
        let files_name = self.get_name("Files");
        let files_el = self.xot.new_element(files_name);

        // Create File element with path attribute
        let file_name = self.get_name("File");
        let file_el = self.xot.new_element(file_name);

        let path_attr = self.get_name("path");
        self.xot.attributes_mut(file_el).insert(path_attr, file_path.to_string());

        // Build the tree from serialized AST
        self.build_raw_serialized_node(node, source, file_el, None)?;

        // Assemble document
        self.xot.append(files_el, file_el)?;
        let doc = self.xot.new_document_with_element(files_el)?;

        Ok(doc)
    }

    /// Recursively build xot nodes from serialized AST node
    #[cfg(feature = "wasm")]
    fn build_raw_serialized_node(
        &mut self,
        node: &SerializedNode,
        source: &str,
        parent: XotNode,
        field_name: Option<&str>,
    ) -> Result<(), xot::Error> {
        // Skip anonymous nodes (punctuation, etc.)
        if !node.is_named {
            return Ok(());
        }

        let kind = &node.kind;
        let elem_name = self.get_name(kind);
        let element = self.xot.new_element(elem_name);

        // Add kind attribute (original TreeSitter kind) for robust transform detection
        let kind_attr = self.get_name("kind");
        self.xot.attributes_mut(element).insert(kind_attr, kind.to_string());

        // Add location attributes (convert from 0-indexed to 1-indexed)
        let start_attr = self.get_name("start");
        let end_attr = self.get_name("end");

        self.xot.attributes_mut(element).insert(
            start_attr,
            format!("{}:{}", node.start_row + 1, node.start_col + 1),
        );
        self.xot.attributes_mut(element).insert(
            end_attr,
            format!("{}:{}", node.end_row + 1, node.end_col + 1),
        );

        // Check if leaf node (no named children)
        let named_child_count = node.named_child_count();

        if named_child_count == 0 {
            // Leaf node - add text content
            let text = node.text(source);
            let text_node = self.xot.new_text(text);
            self.xot.append(element, text_node)?;
        } else {
            // Non-leaf: iterate ALL children to preserve order
            let mut last_end_byte = node.start_byte;

            for child in &node.children {
                let child_start = child.start_byte;

                // Add any whitespace between the last node and this one
                if child_start > last_end_byte {
                    let gap = &source[last_end_byte..child_start];
                    if !gap.is_empty() && gap.chars().any(|c| c.is_whitespace()) {
                        let text_node = self.xot.new_text(" ");
                        self.xot.append(element, text_node)?;
                    }
                }

                if child.is_named {
                    self.build_raw_serialized_node(child, source, element, child.field_name.as_deref())?;
                } else {
                    // Anonymous node - add as text child (operators, keywords, punctuation)
                    let text = child.text(source);
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        let text_node = self.xot.new_text(trimmed);
                        self.xot.append(element, text_node)?;
                    }
                }

                last_end_byte = child.end_byte;
            }
        }

        // Wrap in field element if needed, or just append directly
        if let Some(field) = field_name {
            if Self::should_wrap_field(field) {
                // Create wrapper element with field name
                let wrapper_name = self.get_name(field);
                let wrapper = self.xot.new_element(wrapper_name);

                // Copy location attributes from child to wrapper
                let start_attr = self.get_name("start");
                let end_attr = self.get_name("end");
                if let Some(start_val) = self.xot.attributes(element).get(start_attr).cloned() {
                    self.xot.attributes_mut(wrapper).insert(start_attr, start_val);
                }
                if let Some(end_val) = self.xot.attributes(element).get(end_attr).cloned() {
                    self.xot.attributes_mut(wrapper).insert(end_attr, end_val);
                }

                self.xot.append(wrapper, element)?;
                self.xot.append(parent, wrapper)?;
            } else {
                // Add field as attribute instead
                let field_attr = self.get_name("field");
                self.xot.attributes_mut(element).insert(field_attr, field.to_string());
                self.xot.append(parent, element)?;
            }
        } else {
            self.xot.append(parent, element)?;
        }

        Ok(())
    }

    /// Consume the builder and return the xot instance
    pub fn into_xot(self) -> Xot {
        self.xot
    }

    /// Get a reference to the xot instance
    pub fn xot(&self) -> &Xot {
        &self.xot
    }

    /// Get a mutable reference to the xot instance
    pub fn xot_mut(&mut self) -> &mut Xot {
        &mut self.xot
    }

    /// Render the document to a string with optional colors
    pub fn render(&self, node: XotNode, options: &crate::output::RenderOptions) -> String {
        crate::output::render_document(&self.xot, node, options)
    }
}

impl Default for XotBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// XeeBuilder: Build directly into xee-xpath's Documents (no serialization)
// ============================================================================

use xee_xpath::{Documents, DocumentHandle};

/// Builder for creating documents directly into xee-xpath's Documents
///
/// This avoids the serialize/parse roundtrip by building directly into
/// the Documents' internal Xot arena.
pub struct XeeBuilder {
    documents: Documents,
    /// Cache of name strings to NameIds
    name_cache: HashMap<String, NameId>,
}

impl XeeBuilder {
    /// Create a new XeeBuilder
    pub fn new() -> Self {
        XeeBuilder {
            documents: Documents::new(),
            name_cache: HashMap::new(),
        }
    }

    /// Get or create a NameId for the given name in the Documents' Xot
    fn get_name(&mut self, name: &str) -> NameId {
        if let Some(&id) = self.name_cache.get(name) {
            id
        } else {
            let xot = self.documents.xot_mut();
            let id = xot.add_name(name);
            self.name_cache.insert(name.to_string(), id);
            id
        }
    }

    /// Build a document from TreeSitter AST directly into Documents
    ///
    /// This is the fast path that avoids XML serialization/parsing.
    /// Use `raw_mode=true` to skip semantic transforms (faster but less normalized).
    #[cfg(feature = "native")]
    pub fn build(
        &mut self,
        ts_node: TsNode,
        source: &str,
        file_path: &str,
        lang: &str,
        raw_mode: bool,
    ) -> Result<DocumentHandle, xot::Error> {
        // Build the raw tree
        let doc_handle = self.build_raw(ts_node, source, file_path)?;

        // Apply semantic transforms if not in raw mode
        if !raw_mode {
            // Get the document node for transformation
            let doc_node = self.documents.document_node(doc_handle)
                .ok_or_else(|| xot::Error::Io("Failed to get document node".to_string()))?;

            // Apply language-specific transforms
            let transform_fn = crate::languages::get_transform(lang);
            crate::xot_transform::walk_transform(self.documents.xot_mut(), doc_node, transform_fn)?;
        }

        Ok(doc_handle)
    }

    /// Build a document from TreeSitter AST directly into Documents (raw mode only)
    #[cfg(feature = "native")]
    pub fn build_raw(
        &mut self,
        ts_node: TsNode,
        source: &str,
        file_path: &str,
    ) -> Result<DocumentHandle, xot::Error> {
        // Create a shell document with Files root
        let doc_handle = self.documents.add_string(
            "file:///source".try_into().unwrap(),
            "<Files/>",
        ).map_err(|e| xot::Error::Io(e.to_string()))?;

        // Get the document node and root element
        let doc_node = self.documents.document_node(doc_handle)
            .ok_or_else(|| xot::Error::Io("Failed to get document node".to_string()))?;

        let xot = self.documents.xot_mut();
        let root = xot.document_element(doc_node)?;

        // Create File element with path attribute
        let file_name = {
            let id = xot.add_name("File");
            self.name_cache.insert("File".to_string(), id);
            id
        };
        let file_el = xot.new_element(file_name);

        let path_attr = {
            let id = xot.add_name("path");
            self.name_cache.insert("path".to_string(), id);
            id
        };
        xot.attributes_mut(file_el).insert(path_attr, file_path.to_string());

        // Build the tree from TreeSitter
        self.build_raw_node_into_documents(ts_node, source, file_el, None)?;

        // Append File to root
        let xot = self.documents.xot_mut();
        xot.append(root, file_el)?;

        Ok(doc_handle)
    }

    /// Recursively build xot nodes from TreeSitter node into Documents
    #[cfg(feature = "native")]
    fn build_raw_node_into_documents(
        &mut self,
        ts_node: TsNode,
        source: &str,
        parent: XotNode,
        field_name: Option<&str>,
    ) -> Result<(), xot::Error> {
        // Skip anonymous nodes (punctuation, etc.)
        if !ts_node.is_named() {
            return Ok(());
        }

        let kind = ts_node.kind();
        let elem_name = self.get_name(kind);
        let xot = self.documents.xot_mut();
        let element = xot.new_element(elem_name);

        // Add kind attribute (original TreeSitter kind) for robust transform detection
        let kind_attr = self.get_name("kind");
        self.documents.xot_mut().attributes_mut(element).insert(kind_attr, kind.to_string());

        // Add location attributes
        let start = ts_node.start_position();
        let end = ts_node.end_position();

        let start_attr = self.get_name("start");
        let end_attr = self.get_name("end");

        self.documents.xot_mut().attributes_mut(element).insert(
            start_attr,
            format!("{}:{}", start.row + 1, start.column + 1),
        );
        self.documents.xot_mut().attributes_mut(element).insert(
            end_attr,
            format!("{}:{}", end.row + 1, end.column + 1),
        );

        // Check if leaf node (no named children)
        let named_child_count = ts_node.named_child_count();

        if named_child_count == 0 {
            // Leaf node - add text content
            if let Ok(text) = ts_node.utf8_text(source.as_bytes()) {
                let text_node = self.documents.xot_mut().new_text(text);
                self.documents.xot_mut().append(element, text_node)?;
            }
        } else {
            // Non-leaf: iterate ALL children (named and anonymous) to preserve order
            let mut cursor = ts_node.walk();
            cursor.goto_first_child();
            let mut last_end_byte = ts_node.start_byte();

            loop {
                let child = cursor.node();
                let child_start = child.start_byte();

                // Add any whitespace/content between the last node and this one
                if child_start > last_end_byte {
                    let gap = &source[last_end_byte..child_start];
                    if !gap.is_empty() && gap.chars().any(|c| c.is_whitespace()) {
                        let text_node = self.documents.xot_mut().new_text(" ");
                        self.documents.xot_mut().append(element, text_node)?;
                    }
                }

                if child.is_named() {
                    let child_field = cursor.field_name();
                    self.build_raw_node_into_documents(child, source, element, child_field)?;
                } else {
                    // Anonymous node - add as text child (operators, keywords, punctuation)
                    if let Ok(text) = child.utf8_text(source.as_bytes()) {
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            let text_node = self.documents.xot_mut().new_text(trimmed);
                            self.documents.xot_mut().append(element, text_node)?;
                        }
                    }
                }

                last_end_byte = child.end_byte();

                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }

        // Wrap in field element if needed, or just append directly
        if let Some(field) = field_name {
            if XotBuilder::should_wrap_field(field) {
                // Create wrapper element with field name
                let wrapper_name = self.get_name(field);
                let xot = self.documents.xot_mut();
                let wrapper = xot.new_element(wrapper_name);

                // Copy location attributes from child to wrapper
                let start_attr = self.get_name("start");
                let end_attr = self.get_name("end");
                let xot = self.documents.xot_mut();
                if let Some(start_val) = xot.attributes(element).get(start_attr).cloned() {
                    xot.attributes_mut(wrapper).insert(start_attr, start_val);
                }
                if let Some(end_val) = xot.attributes(element).get(end_attr).cloned() {
                    xot.attributes_mut(wrapper).insert(end_attr, end_val);
                }

                xot.append(wrapper, element)?;
                xot.append(parent, wrapper)?;
            } else {
                // Add field as attribute instead
                let field_attr = self.get_name("field");
                let xot = self.documents.xot_mut();
                xot.attributes_mut(element).insert(field_attr, field.to_string());
                xot.append(parent, element)?;
            }
        } else {
            self.documents.xot_mut().append(parent, element)?;
        }

        Ok(())
    }

    /// Consume the builder and return the Documents instance
    pub fn into_documents(self) -> Documents {
        self.documents
    }

    /// Get a reference to the documents instance
    pub fn documents(&self) -> &Documents {
        &self.documents
    }

    /// Get a mutable reference to the documents instance
    pub fn documents_mut(&mut self) -> &mut Documents {
        &mut self.documents
    }
}

impl Default for XeeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_simple_document() {
        // This would require a TreeSitter parser, so just test XotBuilder creation
        let builder = XotBuilder::new();
        assert!(builder.name_cache.is_empty());
    }

    #[test]
    fn test_xee_builder_creation() {
        let builder = XeeBuilder::new();
        assert!(builder.name_cache.is_empty());
    }
}
