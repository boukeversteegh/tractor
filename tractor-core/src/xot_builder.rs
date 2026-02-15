//! Build xot XML documents from TreeSitter AST
//!
//! This module provides a unified pipeline:
//! TreeSitter AST -> xot::Xot document -> colored string output
//!
//! There are two builders:
//! - XotBuilder: Builds into a standalone xot::Xot instance
//! - XeeBuilder: Builds into xee-xpath's Documents for direct XPath querying
//!
//! Both use `TreeBuilder` internally for shared tree-building logic.

#[cfg(feature = "native")]
use tree_sitter::Node as TsNode;
use xot::{Xot, Node as XotNode, NameId};
use std::collections::HashMap;

#[cfg(feature = "wasm")]
use crate::wasm_ast::SerializedNode;

/// Normalize CRLF line endings to LF
/// This ensures consistent text content regardless of source file line endings
#[inline]
fn normalize_crlf(text: &str) -> std::borrow::Cow<'_, str> {
    if text.contains('\r') {
        std::borrow::Cow::Owned(text.replace('\r', ""))
    } else {
        std::borrow::Cow::Borrowed(text)
    }
}

// ============================================================================
// TreeBuilder: Shared tree-building logic
// ============================================================================

/// Helper struct for building xot trees from TreeSitter AST
///
/// This extracts the shared logic between XotBuilder and XeeBuilder,
/// avoiding code duplication while supporting the `ignore_whitespace` flag.
struct TreeBuilder<'a> {
    xot: &'a mut Xot,
    name_cache: &'a mut HashMap<String, NameId>,
    ignore_whitespace: bool,
}

impl<'a> TreeBuilder<'a> {
    /// Create a new TreeBuilder
    fn new(xot: &'a mut Xot, name_cache: &'a mut HashMap<String, NameId>, ignore_whitespace: bool) -> Self {
        TreeBuilder {
            xot,
            name_cache,
            ignore_whitespace,
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

    /// Create a text node, applying whitespace stripping if enabled
    /// Returns None if the text is empty after processing
    fn create_text(&mut self, text: &str) -> Option<XotNode> {
        // Normalize CRLF first
        let text = normalize_crlf(text);

        // Apply whitespace stripping if enabled
        let text = if self.ignore_whitespace {
            text.split_whitespace().collect::<Vec<_>>().join("")
        } else {
            text.to_string()
        };

        // Skip empty text nodes
        if text.is_empty() {
            return None;
        }

        Some(self.xot.new_text(&text))
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

    /// Recursively build xot nodes from TreeSitter node with optional depth limit
    #[cfg(feature = "native")]
    fn build_node(
        &mut self,
        ts_node: TsNode,
        source: &str,
        parent: XotNode,
        field_name: Option<&str>,
        current_depth: usize,
        max_depth: Option<usize>,
    ) -> Result<(), xot::Error> {
        // Check depth limit - stop recursion if exceeded
        if let Some(max) = max_depth {
            if current_depth >= max {
                return Ok(());
            }
        }

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
                if let Some(text_node) = self.create_text(text) {
                    self.xot.append(element, text_node)?;
                }
            }
        } else {
            // Non-leaf: iterate ALL children
            let mut cursor = ts_node.walk();
            cursor.goto_first_child();
            let mut last_end_byte = ts_node.start_byte();

            loop {
                let child = cursor.node();
                let child_start = child.start_byte();

                // Add source text between the last node and this one
                if child_start > last_end_byte {
                    let gap = &source[last_end_byte..child_start];
                    if let Some(text_node) = self.create_text(gap) {
                        self.xot.append(element, text_node)?;
                    }
                }

                if child.is_named() {
                    let child_field = cursor.field_name();
                    // Recurse with incremented depth
                    self.build_node(child, source, element, child_field, current_depth + 1, max_depth)?;
                } else {
                    // Anonymous node - add as text child
                    if let Ok(text) = child.utf8_text(source.as_bytes()) {
                        let trimmed = normalize_crlf(text.trim());
                        if !trimmed.is_empty() {
                            let text_node = self.xot.new_text(&trimmed);
                            self.xot.append(element, text_node)?;
                        }
                    }
                }

                last_end_byte = child.end_byte();

                if !cursor.goto_next_sibling() {
                    break;
                }
            }

            // Add any trailing text after the last child
            let parent_end_byte = ts_node.end_byte();
            if parent_end_byte > last_end_byte {
                let trailing = &source[last_end_byte..parent_end_byte];
                if let Some(text_node) = self.create_text(trailing) {
                    self.xot.append(element, text_node)?;
                }
            }
        }

        // Wrap in field element if needed, or just append directly
        if let Some(field) = field_name {
            if Self::should_wrap_field(field) {
                let wrapper_name = self.get_name(field);
                let wrapper = self.xot.new_element(wrapper_name);

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
                let field_attr = self.get_name("field");
                self.xot.attributes_mut(element).insert(field_attr, field.to_string());
                self.xot.append(parent, element)?;
            }
        } else {
            self.xot.append(parent, element)?;
        }

        Ok(())
    }

    /// Recursively build xot nodes from serialized AST node (WASM)
    #[cfg(feature = "wasm")]
    fn build_serialized_node(
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
            if let Some(text_node) = self.create_text(text) {
                self.xot.append(element, text_node)?;
            }
        } else {
            // Non-leaf: iterate ALL children to preserve order
            let mut last_end_byte = node.start_byte;

            for child in &node.children {
                let child_start = child.start_byte;

                // Add source text between the last node and this one
                if child_start > last_end_byte {
                    let gap = &source[last_end_byte..child_start];
                    if let Some(text_node) = self.create_text(gap) {
                        self.xot.append(element, text_node)?;
                    }
                }

                if child.is_named {
                    self.build_serialized_node(child, source, element, child.field_name.as_deref())?;
                } else {
                    // Anonymous node - add as text child (operators, keywords, punctuation)
                    let text = child.text(source);
                    let trimmed = normalize_crlf(text.trim());
                    if !trimmed.is_empty() {
                        let text_node = self.xot.new_text(&trimmed);
                        self.xot.append(element, text_node)?;
                    }
                }

                last_end_byte = child.end_byte;
            }

            // Add any trailing text after the last child
            let parent_end_byte = node.end_byte;
            if parent_end_byte > last_end_byte {
                let trailing = &source[last_end_byte..parent_end_byte];
                if let Some(text_node) = self.create_text(trailing) {
                    self.xot.append(element, text_node)?;
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
}

// ============================================================================
// XotBuilder: Build into standalone xot::Xot instance
// ============================================================================

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
        self.build_raw_with_options(ts_node, source, file_path, false)
    }

    /// Build an xot document from TreeSitter AST with options
    #[cfg(feature = "native")]
    pub fn build_raw_with_options(
        &mut self,
        ts_node: TsNode,
        source: &str,
        file_path: &str,
        ignore_whitespace: bool,
    ) -> Result<XotNode, xot::Error> {
        // Create Files root element
        let files_name = self.get_name("Files");
        let files_el = self.xot.new_element(files_name);

        // Create File element with path attribute
        let file_name = self.get_name("File");
        let file_el = self.xot.new_element(file_name);

        let path_attr = self.get_name("path");
        self.xot.attributes_mut(file_el).insert(path_attr, file_path.to_string());

        // Build the tree from TreeSitter using TreeBuilder
        {
            let mut builder = TreeBuilder::new(&mut self.xot, &mut self.name_cache, ignore_whitespace);
            builder.build_node(ts_node, source, file_el, None, 0, None)?;
        }

        // Assemble document
        self.xot.append(files_el, file_el)?;
        let doc = self.xot.new_document_with_element(files_el)?;

        Ok(doc)
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
        self.build_raw_from_serialized_with_options(node, source, file_path, false)
    }

    /// Build an xot document from a serialized AST with options (for WASM)
    #[cfg(feature = "wasm")]
    pub fn build_raw_from_serialized_with_options(
        &mut self,
        node: &SerializedNode,
        source: &str,
        file_path: &str,
        ignore_whitespace: bool,
    ) -> Result<XotNode, xot::Error> {
        // Create Files root element
        let files_name = self.get_name("Files");
        let files_el = self.xot.new_element(files_name);

        // Create File element with path attribute
        let file_name = self.get_name("File");
        let file_el = self.xot.new_element(file_name);

        let path_attr = self.get_name("path");
        self.xot.attributes_mut(file_el).insert(path_attr, file_path.to_string());

        // Build the tree from serialized AST using TreeBuilder
        {
            let mut builder = TreeBuilder::new(&mut self.xot, &mut self.name_cache, ignore_whitespace);
            builder.build_serialized_node(node, source, file_el, None)?;
        }

        // Assemble document
        self.xot.append(files_el, file_el)?;
        let doc = self.xot.new_document_with_element(files_el)?;

        Ok(doc)
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
        self.build_with_options(ts_node, source, file_path, lang, raw_mode, false, None)
    }

    /// Build a document from TreeSitter AST with options
    ///
    /// This is the fast path that avoids XML serialization/parsing.
    /// Use `raw_mode=true` to skip semantic transforms (faster but less normalized).
    /// Use `ignore_whitespace=true` to strip whitespace from text nodes.
    /// Use `max_depth` to limit tree building depth (skip deeper nodes for speed).
    #[cfg(feature = "native")]
    pub fn build_with_options(
        &mut self,
        ts_node: TsNode,
        source: &str,
        file_path: &str,
        lang: &str,
        raw_mode: bool,
        ignore_whitespace: bool,
        max_depth: Option<usize>,
    ) -> Result<DocumentHandle, xot::Error> {
        use std::time::Instant;
        use std::sync::atomic::{AtomicU64, Ordering};

        static TIMING_RAW_BUILD: AtomicU64 = AtomicU64::new(0);
        static TIMING_TRANSFORM: AtomicU64 = AtomicU64::new(0);
        static TIMING_BUILD_COUNT: AtomicU64 = AtomicU64::new(0);

        let t0 = Instant::now();
        // Build the raw tree
        let doc_handle = self.build_raw_with_options(ts_node, source, file_path, ignore_whitespace, max_depth)?;
        let t1 = Instant::now();

        // Apply semantic transforms if not in raw mode
        if !raw_mode {
            if let Some((format, syntax_fn, data_fn)) = crate::languages::get_data_transforms(lang) {
                // Data-aware language: build dual-branch (syntax + data)
                self.apply_data_transforms(doc_handle, format, syntax_fn, data_fn)?;
            } else {
                // Programming language: single transform
                let doc_node = self.documents.document_node(doc_handle)
                    .ok_or_else(|| xot::Error::Io("Failed to get document node".to_string()))?;
                let transform_fn = crate::languages::get_transform(lang);
                crate::xot_transform::walk_transform(self.documents.xot_mut(), doc_node, transform_fn)?;
            }
        }
        let t2 = Instant::now();

        TIMING_RAW_BUILD.fetch_add((t1 - t0).as_micros() as u64, Ordering::Relaxed);
        TIMING_TRANSFORM.fetch_add((t2 - t1).as_micros() as u64, Ordering::Relaxed);
        let count = TIMING_BUILD_COUNT.fetch_add(1, Ordering::Relaxed) + 1;

        // Print stats periodically
        if count % 5000 == 0 {
            let raw = TIMING_RAW_BUILD.load(Ordering::Relaxed);
            let transform = TIMING_TRANSFORM.load(Ordering::Relaxed);
            eprintln!("\n=== Xot Build Stats ({} files) ===", count);
            eprintln!("Raw build:    {:>8.2}ms ({:.2}ms/file)", raw as f64 / 1000.0, raw as f64 / 1000.0 / count as f64);
            eprintln!("Transform:    {:>8.2}ms ({:.2}ms/file)", transform as f64 / 1000.0, transform as f64 / 1000.0 / count as f64);
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
        self.build_raw_with_options(ts_node, source, file_path, false, None)
    }

    /// Build a document from TreeSitter AST with options (raw mode only)
    ///
    /// Use `max_depth` to limit tree building depth (skip deeper nodes for speed).
    #[cfg(feature = "native")]
    pub fn build_raw_with_options(
        &mut self,
        ts_node: TsNode,
        source: &str,
        file_path: &str,
        ignore_whitespace: bool,
        max_depth: Option<usize>,
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

        // Build the tree from TreeSitter using TreeBuilder
        {
            let xot = self.documents.xot_mut();
            let mut builder = TreeBuilder::new(xot, &mut self.name_cache, ignore_whitespace);
            builder.build_node(ts_node, source, file_el, None, 0, max_depth)?;
        }

        // Append File to root
        let xot = self.documents.xot_mut();
        xot.append(root, file_el)?;

        Ok(doc_handle)
    }

    /// Build dual-branch (syntax + data) tree for data-aware languages.
    ///
    /// Takes the raw tree under <File>, clones it, applies the syntax transform
    /// to one copy and the data transform to the other, then wraps each in
    /// <syntax> and <data> elements under <File>.
    ///
    /// For the data branch, single-document files have <document> flattened
    /// so content sits directly under <data>. Multi-document files keep
    /// <document> wrappers for positional queries.
    #[cfg(feature = "native")]
    fn apply_data_transforms(
        &mut self,
        doc_handle: DocumentHandle,
        format: &str,
        syntax_transform: crate::languages::TransformFn,
        data_transform: crate::languages::TransformFn,
    ) -> Result<(), xot::Error> {
        let doc_node = self.documents.document_node(doc_handle)
            .ok_or_else(|| xot::Error::Io("Failed to get document node".to_string()))?;

        let xot = self.documents.xot_mut();

        // Find <Files> -> <File>
        let files_el = xot.document_element(doc_node)?;
        let file_el = xot.children(files_el)
            .find(|&c| xot.element(c).is_some())
            .ok_or_else(|| xot::Error::Io("No File element found".to_string()))?;

        // Set kind="data" and format="json|yaml" on <File>
        let kind_attr = xot.add_name("kind");
        xot.attributes_mut(file_el).insert(kind_attr, "data".to_string());
        let format_attr = xot.add_name("format");
        xot.attributes_mut(file_el).insert(format_attr, format.to_string());

        // Find the content root (first element child of <File>)
        let content_root = xot.children(file_el)
            .find(|&c| xot.element(c).is_some())
            .ok_or_else(|| xot::Error::Io("No content root under File".to_string()))?;

        // Clone the content subtree for the data branch
        let data_content = xot.clone_node(content_root);

        // Create <syntax> and <data> wrapper elements FIRST, then attach content
        // to them before transforming. Transforms may Flatten the root node,
        // which requires a parent to insert children into.
        let syntax_name = xot.add_name("syntax");
        let syntax_el = xot.new_element(syntax_name);
        let data_name = xot.add_name("data");
        let data_el = xot.new_element(data_name);

        // Move original content from <File> into <syntax>
        xot.detach(content_root)?;
        xot.append(syntax_el, content_root)?;

        // Attach cloned content into <data>
        xot.append(data_el, data_content)?;

        // Apply syntax transform (content_root is now child of <syntax>)
        crate::xot_transform::walk_transform_node(xot, content_root, syntax_transform)?;

        // Apply data transform (data_content is now child of <data>)
        crate::xot_transform::walk_transform_node(xot, data_content, data_transform)?;

        // Flatten single <document> in data branch.
        // For single-doc files (JSON, single YAML), <document> is unnecessary
        // nesting — content should sit directly under <data>.
        // For multi-doc YAML, keep <document> wrappers for positional queries.
        let doc_element_name = xot.add_name("document");
        let doc_children: Vec<XotNode> = xot.children(data_el)
            .filter(|&c| {
                xot.element(c)
                    .map(|e| e.name() == doc_element_name)
                    .unwrap_or(false)
            })
            .collect();

        if doc_children.len() == 1 {
            let single_doc = doc_children[0];
            // Move all children of <document> up to <data>
            let children: Vec<XotNode> = xot.children(single_doc).collect();
            for child in children {
                xot.detach(child)?;
                xot.insert_before(single_doc, child)?;
            }
            // Remove the now-empty <document>
            xot.detach(single_doc)?;
        }

        // Append <syntax> and <data> to <File>
        xot.append(file_el, syntax_el)?;
        xot.append(file_el, data_el)?;

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
