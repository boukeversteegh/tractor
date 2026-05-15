//! Shared plumbing for the [`DataTree`] source emitters
//! ([`super::data_json`], [`super::data_yaml`]).
//!
//! The per-format walks diverge — JSON's `{ "k": v }` block layout
//! has no useful overlap with YAML's significant-indent `k: v` form —
//! so the engines stay separate. What *is* shared lives here:
//!
//! - [`DataSpanMap`]: `(line, col) → (rendered_start, rendered_end)`
//!   used by [`crate::mutation::xpath_upsert`] to splice byte regions
//!   back into source.
//! - [`DataRenderOptions`]: indent + newline + level. Both formats
//!   accept the same shape; previously each carried its own
//!   nominally-identical struct.
//! - [`scalar_text`]: read the leaf text for any [`DataTree`] scalar,
//!   falling back to canonical spellings for empty / non-scalar
//!   variants.

use std::collections::HashMap;

use crate::tree::data::DataTree;

/// `(line, col) → (rendered_start, rendered_end)` byte range map.
///
/// Same shape both data emitters produce; consumers
/// ([`crate::mutation::xpath_upsert`]) treat them interchangeably.
pub type DataSpanMap = HashMap<(u32, u32), (usize, usize)>;

/// Output-formatting options for a [`DataTree`] emitter. Indent unit,
/// newline form, and current depth. Both JSON and YAML accept the
/// same shape.
#[derive(Debug, Clone)]
pub struct DataRenderOptions {
    pub indent: String,
    pub newline: String,
    pub indent_level: usize,
}

impl Default for DataRenderOptions {
    fn default() -> Self {
        Self {
            indent: "  ".to_string(),
            newline: "\n".to_string(),
            indent_level: 0,
        }
    }
}

impl DataRenderOptions {
    pub fn indented(&self) -> Self {
        Self {
            indent: self.indent.clone(),
            newline: self.newline.clone(),
            indent_level: self.indent_level + 1,
        }
    }

    pub fn current_indent(&self) -> String {
        self.indent.repeat(self.indent_level)
    }
}

/// Leaf text for any [`DataTree`] scalar; empty string for compound
/// or unrecognised variants. Mirrors `DataTree::scalar_text()` but
/// owns the result and supplies canonical fallbacks (`"null"` for
/// Null with empty text) so renderers can route the result straight
/// to their output buffer.
pub fn scalar_text(tree: &DataTree) -> String {
    match tree {
        DataTree::String { value, .. } => value.clone(),
        DataTree::Number { text, .. } => text.clone(),
        DataTree::Bool { value, text, .. } => {
            if text.is_empty() {
                if *value { "true".to_string() } else { "false".to_string() }
            } else {
                text.clone()
            }
        }
        DataTree::Null { text, .. } => {
            if text.is_empty() { "null".to_string() } else { text.clone() }
        }
        _ => String::new(),
    }
}
