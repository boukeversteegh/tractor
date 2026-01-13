//! Semantic tree transformation toolkit
//!
//! Provides composable transform functions that operate on XML node concepts.
//! Transforms are applied during AST traversal for performance.
//!
//! ## Architecture
//! - General transforms: Apply to all languages (rename, wrap_fields, flatten, skip)
//! - Language-specific transforms: Handle language-specific patterns (text_to_modifier, operator extraction)
//!
//! ## Data Flow
//! ```text
//! TreeSitter AST → collect_node_info() → apply transforms → emit XML
//! ```

/// Collected information about a tree-sitter node in XML terms
#[derive(Debug, Clone)]
pub struct NodeInfo<'a> {
    /// Original node kind from tree-sitter (e.g., "binary_expression")
    pub kind: &'a str,
    /// Field name from parent, if any (e.g., "left", "right", "name")
    pub field: Option<&'a str>,
    /// Anonymous text children (operators, keywords, punctuation)
    pub text_content: Vec<&'a str>,
    /// Source location
    pub start_line: usize,
    pub start_col: usize,
    pub end_line: usize,
    pub end_col: usize,
    /// Source text for leaf nodes
    pub text: Option<&'a str>,
    /// Number of named children
    pub named_child_count: usize,
}

/// Result of applying transforms to a node
#[derive(Debug, Clone)]
pub struct TransformedNode {
    /// Output element name (after rename)
    pub element_name: String,
    /// Additional attributes to emit (e.g., op="+" for operators)
    pub extra_attrs: Vec<(String, String)>,
    /// Modifier elements to emit as empty children (e.g., <let/>, <public/>)
    pub modifiers: Vec<String>,
    /// Whether to skip this node entirely (children promoted to parent)
    pub skip: bool,
    /// Whether to flatten (emit children at current indent, no wrapper)
    pub flatten: bool,
}

impl TransformedNode {
    pub fn new(element_name: &str) -> Self {
        Self {
            element_name: element_name.to_string(),
            extra_attrs: Vec::new(),
            modifiers: Vec::new(),
            skip: false,
            flatten: false,
        }
    }

    pub fn with_attr(mut self, name: &str, value: &str) -> Self {
        self.extra_attrs.push((name.to_string(), value.to_string()));
        self
    }

    pub fn with_modifier(mut self, modifier: &str) -> Self {
        self.modifiers.push(modifier.to_string());
        self
    }

    pub fn skipped() -> Self {
        Self {
            element_name: String::new(),
            extra_attrs: Vec::new(),
            modifiers: Vec::new(),
            skip: true,
            flatten: false,
        }
    }

    pub fn flattened() -> Self {
        Self {
            element_name: String::new(),
            extra_attrs: Vec::new(),
            modifiers: Vec::new(),
            skip: false,
            flatten: true,
        }
    }
}

// =============================================================================
// GENERAL TRANSFORMS (apply to all languages)
// =============================================================================

/// Rename an element based on a mapping
/// Returns the mapped name or original if not found
pub fn rename<'a>(kind: &'a str, mappings: &'static [(&'static str, &'static str)]) -> &'a str {
    mappings
        .iter()
        .find(|(from, _)| *from == kind)
        .map(|(_, to)| *to)
        .unwrap_or(kind)
}

/// Check if a field should be wrapped in a semantic element
pub fn should_wrap_field(field: &str) -> bool {
    matches!(field,
        "name" | "value" | "left" | "right" | "body" |
        "parameters" | "condition" | "consequence" | "alternative" |
        "returns" | "arguments"
    )
}

/// Check if a field represents a type context (identifiers should become <type>)
pub fn is_type_context_field(field: &str) -> bool {
    matches!(field, "returns" | "type")
}

// =============================================================================
// LANGUAGE-SPECIFIC TRANSFORMS
// These are called from language config handlers
// =============================================================================

/// Extract first text content as an attribute (for operators)
/// General pattern: binary_expression has text "+" -> add op="+" attribute
pub fn text_to_attr(text_content: &[&str], attr_name: &str) -> Option<(String, String)> {
    text_content.first().map(|t| (attr_name.to_string(), t.to_string()))
}

/// Convert text content items to modifier elements
/// General pattern: "let" text -> <let/> modifier element
pub fn text_to_modifiers(text_content: &[&str], known_modifiers: &[&str]) -> Vec<String> {
    text_content
        .iter()
        .filter(|t| known_modifiers.contains(t))
        .map(|t| t.to_string())
        .collect()
}

/// Filter text content to extract operator-like tokens
/// Skips parentheses, commas, semicolons - keeps operators
pub fn extract_operator<'a>(text_content: &[&'a str]) -> Option<&'a str> {
    text_content.iter().find(|t| {
        !t.chars().all(|c| matches!(c, '(' | ')' | ',' | ';' | '{' | '}' | '[' | ']'))
    }).copied()
}

// =============================================================================
// IDENTIFIER CLASSIFICATION (Language-specific rules)
// =============================================================================

/// Context for classifying identifiers as name vs type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IdentifierKind {
    Name,
    Type,
}

/// Classify an identifier based on field context
/// This is the primary classification method - uses tree-sitter field info
pub fn classify_by_field(field: Option<&str>) -> IdentifierKind {
    match field {
        Some("name") => IdentifierKind::Name,
        Some("type") | Some("returns") => IdentifierKind::Type,
        _ => IdentifierKind::Type, // Default to type for unspecified
    }
}


// =============================================================================
// LANGUAGE-SPECIFIC TRANSFORM CONFIGURATIONS
// =============================================================================

/// Complete transform configuration for a language
#[derive(Clone)]
pub struct LangTransforms {
    /// Element name mappings (tree-sitter kind -> semantic name)
    pub element_mappings: &'static [(&'static str, &'static str)],
    /// Node kinds to flatten (promote children to parent level)
    pub flatten_kinds: &'static [&'static str],
    /// Node kinds to skip entirely
    pub skip_kinds: &'static [&'static str],
    /// Node kinds that should extract operator from text content
    pub operator_kinds: &'static [&'static str],
    /// Node kinds that should extract keyword modifiers (let/const/var)
    pub keyword_modifier_kinds: &'static [&'static str],
    /// Known modifier keywords
    pub known_modifiers: &'static [&'static str],
    /// Node kinds that represent modifiers (C# has wrapper nodes)
    pub modifier_wrapper_kinds: &'static [&'static str],
    /// Node kinds that should extract a "name" attribute from their name field (e.g., namespace)
    pub extract_name_attr_kinds: &'static [&'static str],
    /// Language-specific identifier classification function
    /// Args: (parent_kind, has_param_sibling, in_special_context) -> IdentifierKind
    pub classify_identifier: fn(parent_kind: &str, has_param_sibling: bool, in_special_context: bool) -> IdentifierKind,
    /// Language-specific context computation for identifier classification
    /// Walks up the tree to compute context (e.g., C# checks if in namespace declaration)
    /// Default languages should use `default_compute_context` which returns false
    pub compute_identifier_context: fn(parent_chain: &[&str]) -> bool,
}

/// Default context computation - returns false (no special context)
pub fn default_compute_context(_parent_chain: &[&str]) -> bool {
    false
}

// Manual Debug impl since function pointers don't derive Debug nicely
impl std::fmt::Debug for LangTransforms {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LangTransforms")
            .field("element_mappings", &self.element_mappings)
            .field("flatten_kinds", &self.flatten_kinds)
            .field("skip_kinds", &self.skip_kinds)
            .field("operator_kinds", &self.operator_kinds)
            .field("keyword_modifier_kinds", &self.keyword_modifier_kinds)
            .field("known_modifiers", &self.known_modifiers)
            .field("modifier_wrapper_kinds", &self.modifier_wrapper_kinds)
            .field("extract_name_attr_kinds", &self.extract_name_attr_kinds)
            .field("classify_identifier", &"<fn>")
            .field("compute_identifier_context", &"<fn>")
            .finish()
    }
}

impl LangTransforms {
    /// Compute special context for identifier classification by walking the parent chain
    pub fn compute_context(&self, parent_chain: &[&str]) -> bool {
        (self.compute_identifier_context)(parent_chain)
    }

    /// Check if node kind should be flattened
    pub fn should_flatten(&self, kind: &str) -> bool {
        self.flatten_kinds.contains(&kind)
    }

    /// Check if node kind should be skipped
    pub fn should_skip(&self, kind: &str) -> bool {
        self.skip_kinds.contains(&kind)
    }

    /// Check if node kind should extract operator
    pub fn should_extract_operator(&self, kind: &str) -> bool {
        self.operator_kinds.contains(&kind)
    }

    /// Check if node kind should extract keyword modifiers
    pub fn should_extract_keyword_modifier(&self, kind: &str) -> bool {
        self.keyword_modifier_kinds.contains(&kind)
    }

    /// Check if text is a known modifier
    pub fn is_known_modifier(&self, text: &str) -> bool {
        self.known_modifiers.contains(&text)
    }

    /// Check if node kind is a modifier wrapper
    pub fn is_modifier_wrapper(&self, kind: &str) -> bool {
        self.modifier_wrapper_kinds.contains(&kind)
    }

    /// Get renamed element name
    pub fn rename_element<'a>(&self, kind: &'a str) -> &'a str {
        rename(kind, self.element_mappings)
    }
}

// =============================================================================
// RE-EXPORT LANGUAGE CONFIGS (from sibling languages module)
// =============================================================================

// Re-export the get_transforms function from the languages module
// The actual language configs are defined in parser/languages/
pub use super::languages::get_transforms;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rename() {
        static MAPPINGS: &[(&str, &str)] = &[
            ("binary_expression", "binary"),
            ("class_declaration", "class"),
        ];

        assert_eq!(rename("binary_expression", MAPPINGS), "binary");
        assert_eq!(rename("class_declaration", MAPPINGS), "class");
        assert_eq!(rename("unknown_kind", MAPPINGS), "unknown_kind");
    }

    #[test]
    fn test_should_wrap_field() {
        assert!(should_wrap_field("left"));
        assert!(should_wrap_field("right"));
        assert!(should_wrap_field("name"));
        assert!(should_wrap_field("value"));
        assert!(!should_wrap_field("unknown"));
    }

    #[test]
    fn test_extract_operator() {
        assert_eq!(extract_operator(&["+", "(", ")"]), Some("+"));
        assert_eq!(extract_operator(&["&&"]), Some("&&"));
        assert_eq!(extract_operator(&["(", ")", ","]), None);
    }

    #[test]
    fn test_text_to_attr() {
        let text = vec!["+", "2"];
        let attr = text_to_attr(&text, "op");
        assert_eq!(attr, Some(("op".to_string(), "+".to_string())));
    }

    #[test]
    fn test_text_to_modifiers() {
        let text = vec!["let", "x", "="];
        let known = &["let", "const", "var"];
        let mods = text_to_modifiers(&text, known);
        assert_eq!(mods, vec!["let"]);
    }

    #[test]
    fn test_classify_by_field() {
        assert_eq!(classify_by_field(Some("name")), IdentifierKind::Name);
        assert_eq!(classify_by_field(Some("type")), IdentifierKind::Type);
        assert_eq!(classify_by_field(Some("returns")), IdentifierKind::Type);
        assert_eq!(classify_by_field(None), IdentifierKind::Type);
    }

    // Language transform config tests

    #[test]
    fn test_typescript_transforms() {
        let ts = get_transforms("typescript");

        // Element renaming
        assert_eq!(ts.rename_element("binary_expression"), "binary");
        assert_eq!(ts.rename_element("class_declaration"), "class");
        assert_eq!(ts.rename_element("lexical_declaration"), "variable");

        // Operator extraction
        assert!(ts.should_extract_operator("binary_expression"));
        assert!(ts.should_extract_operator("unary_expression"));
        assert!(!ts.should_extract_operator("if_statement"));

        // Keyword modifiers (let/const/var)
        assert!(ts.should_extract_keyword_modifier("lexical_declaration"));
        assert!(!ts.should_extract_keyword_modifier("if_statement"));

        // Known modifiers
        assert!(ts.is_known_modifier("let"));
        assert!(ts.is_known_modifier("const"));
        assert!(ts.is_known_modifier("async"));

        // Skip/flatten
        assert!(ts.should_skip("expression_statement"));
        assert!(ts.should_flatten("variable_declarator"));
    }

    #[test]
    fn test_csharp_transforms() {
        let cs = get_transforms("csharp");

        // Element renaming
        assert_eq!(cs.rename_element("method_declaration"), "method");
        assert_eq!(cs.rename_element("class_declaration"), "class");
        assert_eq!(cs.rename_element("namespace_declaration"), "namespace");

        // Operator extraction
        assert!(cs.should_extract_operator("binary_expression"));

        // Modifier wrapper (C# uses wrapper nodes)
        assert!(cs.is_modifier_wrapper("modifier"));

        // Known modifiers
        assert!(cs.is_known_modifier("public"));
        assert!(cs.is_known_modifier("static"));
        assert!(cs.is_known_modifier("async"));

        // Flatten
        assert!(cs.should_flatten("declaration_list"));
    }

    #[test]
    fn test_python_transforms() {
        let py = get_transforms("python");

        // Element renaming
        assert_eq!(py.rename_element("function_definition"), "function");
        assert_eq!(py.rename_element("class_definition"), "class");
        assert_eq!(py.rename_element("list_comprehension"), "listcomp");

        // Operator extraction
        assert!(py.should_extract_operator("binary_operator"));
        assert!(py.should_extract_operator("comparison_operator"));

        // Skip
        assert!(py.should_skip("expression_statement"));
    }

    #[test]
    fn test_go_transforms() {
        let go = get_transforms("go");

        // Element renaming
        assert_eq!(go.rename_element("function_declaration"), "function");
        assert_eq!(go.rename_element("struct_type"), "struct");
        assert_eq!(go.rename_element("defer_statement"), "defer");

        // Operator extraction
        assert!(go.should_extract_operator("binary_expression"));
    }

    #[test]
    fn test_rust_transforms() {
        let rs = get_transforms("rust");

        // Element renaming
        assert_eq!(rs.rename_element("function_item"), "function");
        assert_eq!(rs.rename_element("struct_item"), "struct");
        assert_eq!(rs.rename_element("impl_item"), "impl");
        assert_eq!(rs.rename_element("match_expression"), "match");

        // Operator extraction
        assert!(rs.should_extract_operator("binary_expression"));

        // Modifier wrapper (Rust uses visibility_modifier)
        assert!(rs.is_modifier_wrapper("visibility_modifier"));

        // Known modifiers
        assert!(rs.is_known_modifier("pub"));
        assert!(rs.is_known_modifier("mut"));
    }

    #[test]
    fn test_java_transforms() {
        let java = get_transforms("java");

        // Element renaming
        assert_eq!(java.rename_element("method_declaration"), "method");
        assert_eq!(java.rename_element("class_declaration"), "class");
        assert_eq!(java.rename_element("method_invocation"), "call");

        // Modifier wrapper
        assert!(java.is_modifier_wrapper("modifiers"));

        // Known modifiers
        assert!(java.is_known_modifier("public"));
        assert!(java.is_known_modifier("static"));
        assert!(java.is_known_modifier("final"));

        // Flatten
        assert!(java.should_flatten("class_body"));
    }

    #[test]
    fn test_get_transforms_aliases() {
        // Test language aliases resolve to correct config
        assert_eq!(get_transforms("ts").rename_element("binary_expression"), "binary");
        assert_eq!(get_transforms("tsx").rename_element("binary_expression"), "binary");
        assert_eq!(get_transforms("js").rename_element("binary_expression"), "binary");
        assert_eq!(get_transforms("jsx").rename_element("binary_expression"), "binary");
        assert_eq!(get_transforms("cs").rename_element("class_declaration"), "class");
        assert_eq!(get_transforms("py").rename_element("function_definition"), "function");
        assert_eq!(get_transforms("rs").rename_element("function_item"), "function");
    }
}
