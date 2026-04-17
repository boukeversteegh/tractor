//! Configuration for language-specific semantic tree transformations

/// Configuration for a language's semantic tree transformation
#[derive(Debug, Clone)]
pub struct LanguageConfig {
    /// Map TreeSitter node kinds to semantic element names
    /// e.g., ("class_declaration", "class")
    pub element_mappings: &'static [(&'static str, &'static str)],

    /// Node kinds that represent modifiers (become empty elements)
    /// e.g., ["modifier"] for C#
    pub modifier_kinds: &'static [&'static str],

    /// Known modifier text values
    /// e.g., ["public", "private", "static", "async"]
    pub known_modifiers: &'static [&'static str],

    /// Node kinds that should be flattened (children promoted to parent)
    /// e.g., ["declaration_list"] - removes unnecessary nesting
    pub flatten_kinds: &'static [&'static str],

    /// Node kinds that should be skipped entirely
    pub skip_kinds: &'static [&'static str],
}

impl LanguageConfig {
    /// Look up the semantic element name for a TreeSitter node kind
    pub fn map_element_name<'a>(&self, kind: &'a str) -> &'a str {
        self.element_mappings
            .iter()
            .find(|(from, _)| *from == kind)
            .map(|(_, to)| *to as &'a str)
            .unwrap_or(kind)
    }

    /// Check if a node kind represents a modifier
    pub fn is_modifier_kind(&self, kind: &str) -> bool {
        self.modifier_kinds.contains(&kind)
    }

    /// Check if text is a known modifier value
    pub fn is_known_modifier(&self, text: &str) -> bool {
        self.known_modifiers.contains(&text)
    }

    /// Check if a node kind should be flattened
    pub fn should_flatten(&self, kind: &str) -> bool {
        self.flatten_kinds.contains(&kind)
    }

    /// Check if a node kind should be skipped
    pub fn should_skip(&self, kind: &str) -> bool {
        self.skip_kinds.contains(&kind)
    }
}

/// Default configuration for unsupported languages (minimal transformation)
pub static DEFAULT_CONFIG: LanguageConfig = LanguageConfig {
    element_mappings: &[
        // Common patterns across languages
        ("identifier", "name"),
    ],
    modifier_kinds: &[],
    known_modifiers: &[],
    flatten_kinds: &[],
    skip_kinds: &[],
};
