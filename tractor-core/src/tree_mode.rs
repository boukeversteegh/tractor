//! Tree mode controls which tree representation is produced by the parser.

/// Controls which tree representation the parser produces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeMode {
    /// Raw tree-sitter AST, no semantic transforms applied.
    Raw,
    /// Semantic syntax tree (structure). Default for non-data languages.
    Structure,
    /// Data projection (keys become elements, scalars become text).
    /// Only available for data-aware languages (JSON, YAML).
    Data,
}

impl TreeMode {
    /// Resolve an optional user-specified tree mode for a given language.
    ///
    /// When `None`, auto-selects: `Data` for data-aware languages, `Structure` for others.
    /// Returns `Err` if `Data` is requested for a non-data language.
    pub fn resolve(mode: Option<TreeMode>, lang: &str) -> Result<TreeMode, String> {
        match mode {
            Some(TreeMode::Data) => {
                if crate::languages::supports_data_tree(lang) {
                    Ok(TreeMode::Data)
                } else {
                    Err(format!(
                        "data tree is not available for language '{}'; use -t structure or -t raw",
                        lang
                    ))
                }
            }
            Some(m) => Ok(m),
            None => {
                if crate::languages::supports_data_tree(lang) {
                    Ok(TreeMode::Data)
                } else {
                    Ok(TreeMode::Structure)
                }
            }
        }
    }
}
