//! Build xot XML documents from TreeSitter AST
//!
//! This module provides a unified pipeline:
//! TreeSitter AST -> xot::Xot document -> colored string output
//!
//! There are two builders:
//! - XotBuilder: Builds into a standalone xot::Xot instance
//! - XeeBuilder: Builds into xee-xpath's Documents for direct XPath querying

use tree_sitter::Node as TsNode;
use xot::{Xot, Node as XotNode, NameId};
use std::collections::HashMap;

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

    /// Recursively build xot nodes from TreeSitter node (raw mode)
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

        // Add field name if present
        if let Some(field) = field_name {
            let field_attr = self.get_name("field");
            self.xot.attributes_mut(element).insert(field_attr, field.to_string());
        }

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
            let mut cursor = ts_node.walk();
            cursor.goto_first_child();
            loop {
                let child = cursor.node();
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
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }

        self.xot.append(parent, element)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_simple_document() {
        // This would require a TreeSitter parser, so just test XotBuilder creation
        let builder = XotBuilder::new();
        assert!(builder.name_cache.is_empty());
    }
}
