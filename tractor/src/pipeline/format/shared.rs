//! Shared helpers used across multiple format renderers.

use std::path::Path;
use tractor_core::normalize_path;
use tractor_core::report::ReportMatch;
use super::options::{ViewField, ViewSet};

pub fn to_absolute_path(path: &str) -> String {
    let p = Path::new(path);
    let absolute = if p.is_absolute() {
        p.to_path_buf()
    } else if let Ok(cwd) = std::env::current_dir() {
        cwd.join(p)
    } else {
        p.to_path_buf()
    };
    normalize_path(&absolute.to_string_lossy())
}

/// Determine whether a field should be rendered for this match.
/// A field is shown when:
///   - it is selected in the view (for view-gated fields)
///   - it is not hoisted to a parent group (not in skip_dims)
///   - the match actually has a value for it
pub fn should_emit_file(rm: &ReportMatch, skip_dims: &[&str]) -> bool {
    !skip_dims.contains(&"file") && !rm.file.is_empty()
}

pub fn should_emit_command(rm: &ReportMatch, view: &ViewSet, skip_dims: &[&str]) -> bool {
    view.has(ViewField::Command) && !skip_dims.contains(&"command") && !rm.command.is_empty()
}

pub fn should_emit_rule_id(rm: &ReportMatch, skip_dims: &[&str]) -> bool {
    !skip_dims.contains(&"rule_id") && rm.rule_id.is_some()
}
