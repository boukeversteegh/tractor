//! Serializable AST types for WASM interop
//!
//! This module defines types that can be serialized from JavaScript's
//! web-tree-sitter AST and deserialized in Rust for XML building.

use serde::{Deserialize, Serialize};

/// A serialized TreeSitter syntax node
///
/// This mirrors the structure of web-tree-sitter's SyntaxNode but in a
/// format that can be passed across the WASM boundary as JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SerializedNode {
    /// The node type/kind (e.g., "function_declaration", "identifier")
    pub kind: String,

    /// Whether this is a named node (vs anonymous like punctuation)
    pub is_named: bool,

    /// Start position - row (0-indexed)
    pub start_row: usize,

    /// Start position - column (0-indexed)
    pub start_col: usize,

    /// End position - row (0-indexed)
    pub end_row: usize,

    /// End position - column (0-indexed)
    pub end_col: usize,

    /// Start byte offset in source
    pub start_byte: usize,

    /// End byte offset in source
    pub end_byte: usize,

    /// Field name if this node is a field child (e.g., "name", "body")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field_name: Option<String>,

    /// Child nodes (both named and anonymous)
    #[serde(default)]
    pub children: Vec<SerializedNode>,
}

impl SerializedNode {
    /// Check if this is a leaf node (no children)
    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    /// Count named children
    pub fn named_child_count(&self) -> usize {
        self.children.iter().filter(|c| c.is_named).count()
    }

    /// Get text content from source (for leaf nodes)
    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        &source[self.start_byte..self.end_byte]
    }
}

/// Request to parse source code to XML
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseRequest {
    /// The serialized AST from web-tree-sitter
    pub ast: SerializedNode,

    /// The original source code
    pub source: String,

    /// Language identifier (e.g., "csharp", "typescript")
    pub language: String,

    /// File path for the XML output
    #[serde(default = "default_file_path")]
    pub file_path: String,

    /// Whether to output raw (untransformed) XML
    #[serde(default)]
    pub raw_mode: bool,

    /// Whether to include location/kind attributes (start, end, kind)
    #[serde(default = "default_include_locations")]
    pub include_locations: bool,

    /// Whether to format with indentation and newlines
    #[serde(default = "default_pretty_print")]
    pub pretty_print: bool,
}

fn default_include_locations() -> bool {
    true
}

fn default_pretty_print() -> bool {
    true
}

fn default_file_path() -> String {
    "input".to_string()
}

/// Response from parsing
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseResponse {
    /// The generated XML
    pub xml: String,

    /// Any warnings generated during parsing
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_node() {
        let json = r#"{
            "kind": "identifier",
            "isNamed": true,
            "startRow": 0,
            "startCol": 0,
            "endRow": 0,
            "endCol": 3,
            "startByte": 0,
            "endByte": 3,
            "children": []
        }"#;

        let node: SerializedNode = serde_json::from_str(json).unwrap();
        assert_eq!(node.kind, "identifier");
        assert!(node.is_named);
        assert!(node.is_leaf());
    }

    #[test]
    fn test_deserialize_with_field_name() {
        let json = r#"{
            "kind": "identifier",
            "isNamed": true,
            "startRow": 0,
            "startCol": 0,
            "endRow": 0,
            "endCol": 3,
            "startByte": 0,
            "endByte": 3,
            "fieldName": "name",
            "children": []
        }"#;

        let node: SerializedNode = serde_json::from_str(json).unwrap();
        assert_eq!(node.field_name, Some("name".to_string()));
    }
}
