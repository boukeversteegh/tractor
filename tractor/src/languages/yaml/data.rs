//! YAML data-branch orchestrator.
//!
//! Dispatches by element name (not just `kind=`) so the
//! builder-inserted `<value>` wrapper, which has no `kind=` attribute,
//! still gets cleanup. Tree-sitter kinds are matched against the
//! typed `YamlKind` enum and dispatched via the shared rule executor.

use xot::{Xot, Node as XotNode};
use crate::transform::{TransformAction, helpers::*};

use super::input::YamlKind;
use super::transformations::strip_punct_flatten;

pub fn data_transform(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let name = match get_element_name(xot, node) {
        Some(n) => n,
        None => return Ok(TransformAction::Continue),
    };

    // Builder-inserted field wrapper: dispatch by element name
    // (no `kind=` attribute).
    if name == "value" {
        return strip_punct_flatten(xot, node);
    }

    // Tree-sitter kind: dispatch via the rule table.
    let kind = match YamlKind::from_str(&name) {
        Some(k) => k,
        None => return Ok(TransformAction::Continue),
    };
    crate::languages::rule::dispatch(xot, node, super::rules::data_rule(kind))
}
