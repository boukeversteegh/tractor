//! YAML syntax-branch orchestrator.
//!
//! Look up the kind, fetch its `Rule` from `rules::syntax_rule`,
//! execute via the shared dispatcher. Reshape logic lives in
//! [`super::transformations`].

use xot::{Xot, Node as XotNode};
use crate::transform::{TransformAction, helpers::*};

use super::input::YamlKind;

pub fn syntax_transform(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let kind_str = match get_kind(xot, node) {
        Some(k) => k,
        None => return Ok(TransformAction::Continue),
    };
    let kind = match YamlKind::from_str(&kind_str) {
        Some(k) => k,
        None => return Ok(TransformAction::Continue),
    };
    crate::languages::rule::dispatch(xot, node, super::rules::syntax_rule(kind))
}
