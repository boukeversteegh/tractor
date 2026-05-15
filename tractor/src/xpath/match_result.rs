//! Match result types for XPath queries

use std::sync::Arc;

// ---------------------------------------------------------------------------
// XmlNode — native tree for matched XML fragments and XPath data types
// ---------------------------------------------------------------------------

/// A native representation of an XML node tree or XPath value.
///
/// For XML nodes this avoids the serialize-then-reparse roundtrip: instead of
/// calling `xot.to_string(node)` and later parsing the string back, we walk
/// the xot tree once and build an `XmlNode` that downstream renderers can
/// consume directly.
///
/// For XPath maps and arrays, the structured data is represented natively
/// rather than stored as a JSON string — enabling renderers to work with
/// real typed data without deferred parsing.
#[derive(Debug, Clone, PartialEq)]
pub enum XmlNode {
    // --- XML node variants ---

    /// An XML element with tag name, attributes, and children.
    Element {
        name: String,
        attributes: Vec<(String, String)>,
        children: Vec<XmlNode>,
    },
    /// A text node.
    Text(String),
    /// A comment node.
    Comment(String),
    /// A processing instruction.
    ProcessingInstruction {
        target: String,
        data: Option<String>,
    },

    // --- XPath data variants (maps, arrays, scalars) ---

    /// An XPath map: ordered sequence of key–value pairs.
    /// Keys are always strings (from XPath map constructors).
    Map { entries: Vec<(String, XmlNode)> },
    /// An XPath array: ordered sequence of values.
    Array { items: Vec<XmlNode> },
    /// A numeric value (integer or float).
    Number(f64),
    /// A boolean value.
    Boolean(bool),
    /// An explicit null / empty-sequence value.
    Null,
}

// ---------------------------------------------------------------------------
// Match
// ---------------------------------------------------------------------------

/// A single match from an XPath query
#[derive(Debug, Clone)]
pub struct Match {
    /// File path where the match was found
    pub file: String,
    /// Start line (1-based)
    pub line: u32,
    /// Start column (1-based)
    pub column: u32,
    /// End line (1-based)
    pub end_line: u32,
    /// End column (1-based)
    pub end_column: u32,
    /// The matched value (text content or source snippet)
    pub value: String,
    /// Original source lines for location-based output (Arc for cheap cloning)
    pub source_lines: Arc<Vec<String>>,
    /// The matched tree — typed tree for root-document matches, raw
    /// XML for partial matches and XPath atomic / map / array
    /// results. Format renderers (JSON / YAML / XML / source-text)
    /// dispatch on the variant.
    pub tree: Option<Tree>,
    /// Editable-trees [`NodeId`](crate::tree::NodeId) recovered from
    /// the matched xot element's `@id` attribute (S15-Z2). `Some`
    /// when the match came from the typed-tree pipeline and the
    /// element carries a stamped id; `None` for legacy XML matches,
    /// XPath atomic / map / array results, and synthetic xot
    /// elements that were not stamped. The mutation pipeline reads
    /// this to look up the typed-tree node via
    /// [`find_by_id_*`](crate::tree::find_by_id_syntax) (S15-Z3).
    pub node_id: Option<u32>,
}

/// The matched subtree representation. The architectural target is
/// for every match to carry `Tree::SyntaxTree` (so all downstream rendering
/// is a function from tree), but partial XPath matches need a
/// xot↔tree mapping that's not yet built — for those cases the
/// `Tree::Xml` variant remains as a transitional fallback.
///
/// Both tree variants carry an `xml` snapshot of the post-transformed
/// xot subtree alongside the typed tree. The `xml` field is the same
/// representation `Tree::Xml` uses, captured at parse time from the
/// xot tree the XPath engine queries against. JSON / YAML rendering
/// goes through the typed tree (`tree_to_json` / `data_to_json`); XML
/// and text rendering walk the captured `xml`. Both views describe
/// the same document; once the legacy XML shape catches up to the
/// tree's typed shape the `xml` field can retire.
#[derive(Debug, Clone)]
pub enum Tree {
    /// Syntax tree (root-document match).
    /// Native-only: the `crate::tree` module is gated behind the
    /// `native` feature, so WASM builds skip this variant.
    #[cfg(feature = "native")]
    SyntaxTree {
        tree: Arc<crate::tree::SyntaxTree>,
        source: Arc<String>,
        xml: XmlNode,
    },
    /// Data-language tree (root-document match). Native-only.
    #[cfg(feature = "native")]
    DataTree {
        tree: Arc<crate::tree::DataTree>,
        source: Arc<String>,
        xml: XmlNode,
    },
    /// SQL-language tree (root-document match). Native-only.
    /// Renders via `sql_to_xot` for XML and `sql_to_json` for JSON.
    #[cfg(feature = "native")]
    Sql {
        tree: Arc<crate::tree::sql::SqlTree>,
        source: Arc<String>,
        xml: XmlNode,
    },
    /// Raw XML / XPath structured data — used for partial matches
    /// (XPath returning an inner subtree) and for XPath
    /// atomic/map/array results that have no direct tree analogue.
    /// Transitional: once xot↔tree mapping is wired, partial matches
    /// can carry tree too and this variant retires to just the
    /// XPath-structured-data case.
    Xml(XmlNode),
}

impl Tree {
    /// Render the matched tree to a JSON value.
    ///
    /// `SyntaxTree` flows through the variant-blind walker in
    /// `tree::to_json` (same metadata accessors as the XML walker;
    /// emits `serde_json::Value` directly). `DataTree` / `Sql` keep
    /// their existing typed renderers. `Xml` falls back to the
    /// XML → JSON projection for partial-match subtrees.
    pub fn to_json(&self, max_depth: Option<usize>) -> serde_json::Value {
        match self {
            #[cfg(feature = "native")]
            Tree::SyntaxTree { tree, source, .. } => {
                crate::tree::tree_to_json(tree, source)
            }
            #[cfg(feature = "native")]
            Tree::DataTree { tree, .. } => crate::tree::data_to_json(tree),
            #[cfg(feature = "native")]
            Tree::Sql { tree, source, .. } => crate::tree::sql::to_json::sql_to_json(tree, source),
            Tree::Xml(node) => crate::output::xml_node_to_json(node, max_depth),
        }
    }

    /// Borrow the matched tree as an `XmlNode`. Every variant has
    /// one available: the tree variants stash the post-transformed xot
    /// subtree at parse time so legacy XML / text renderers can keep
    /// using it without re-rendering the tree.
    pub fn as_xml_node(&self) -> &XmlNode {
        match self {
            Tree::Xml(node) => node,
            #[cfg(feature = "native")]
            Tree::SyntaxTree { xml, .. } => xml,
            #[cfg(feature = "native")]
            Tree::DataTree { xml, .. } => xml,
            #[cfg(feature = "native")]
            Tree::Sql { xml, .. } => xml,
        }
    }
}

impl Match {
    /// Create a new match with minimal information
    pub fn new(file: String, value: String) -> Self {
        Match {
            file,
            line: 1,
            column: 1,
            end_line: 1,
            end_column: 1,
            value,
            source_lines: Arc::new(Vec::new()),
            tree: None,
            node_id: None,
        }
    }

    /// Create a match with full location information
    pub fn with_location(
        file: String,
        line: u32,
        column: u32,
        end_line: u32,
        end_column: u32,
        value: String,
        source_lines: Arc<Vec<String>>,
    ) -> Self {
        Match {
            file,
            line,
            column,
            end_line,
            end_column,
            value,
            source_lines,
            tree: None,
            node_id: None,
        }
    }

    /// Attach the matched tree (`Tree::SyntaxTree`, `Tree::DataTree`, or
    /// `Tree::Xml` for partial matches / XPath atomic results).
    pub fn with_tree(mut self, tree: Tree) -> Self {
        self.tree = Some(tree);
        self
    }

    /// Attach the editable-trees [`NodeId`](crate::tree::NodeId)
    /// recovered from the matched xot element's `@id` attribute
    /// (S15-Z3 mutation pipeline).
    pub fn with_node_id(mut self, id: u32) -> Self {
        self.node_id = Some(id);
        self
    }

    /// Borrow the matched tree's underlying `XmlNode`. Returns
    /// `None` only when the match has no tree at all (e.g. an
    /// XPath atomic value like `count(...)`); for tree variants the
    /// `XmlNode` is always available — tree variants stash a snapshot
    /// at parse time.
    pub fn xml_node(&self) -> Option<&XmlNode> {
        self.tree.as_ref().map(|t| t.as_xml_node())
    }

    /// Returns `true` when this match's file is the pathless sentinel —
    /// i.e. the match came from inline input (`-s`/stdin) with no
    /// meaningful path to display or write back to.
    pub fn is_pathless(&self) -> bool {
        crate::model::report::is_pathless_file(&self.file)
    }

    /// Extract source snippet from source lines based on location
    pub fn extract_source_snippet(&self) -> String {
        if self.source_lines.is_empty() || self.line == 0 {
            return self.value.clone();
        }

        let start_line = (self.line as usize).saturating_sub(1);
        let end_line = (self.end_line as usize).min(self.source_lines.len());

        if start_line >= self.source_lines.len() {
            return self.value.clone();
        }

        let mut result = String::new();

        for (i, line_idx) in (start_line..end_line).enumerate() {
            let line = &self.source_lines[line_idx];
            let trimmed = line.trim_end_matches('\r');

            let start_col = if i == 0 {
                (self.column as usize).saturating_sub(1).min(trimmed.len())
            } else {
                0
            };

            let end_col = if line_idx == end_line - 1 {
                (self.end_column as usize).saturating_sub(1).min(trimmed.len())
            } else {
                trimmed.len()
            };

            if end_col > start_col {
                result.push_str(&trimmed[start_col..end_col]);
            }

            if line_idx < end_line - 1 {
                result.push('\n');
            }
        }

        result
    }

    /// Get the full source lines for the match range
    pub fn get_source_lines_range(&self) -> Vec<&str> {
        if self.source_lines.is_empty() || self.line == 0 {
            return Vec::new();
        }

        let start = (self.line as usize).saturating_sub(1);
        let end = (self.end_line as usize).min(self.source_lines.len());

        self.source_lines[start..end]
            .iter()
            .map(|s| s.as_str())
            .collect()
    }
}
