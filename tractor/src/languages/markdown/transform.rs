//! Markdown transform orchestrator.
//!
//! Look up the kind, fetch its `Rule` from `rules::rule`, execute via
//! the shared dispatcher. Reshape logic lives in
//! [`super::transformations`].

use xot::{Xot, Node as XotNode};
use crate::transform::{TransformAction, helpers::*};

use super::input::MdKind;

pub fn transform(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let name = match get_element_name(xot, node) {
        Some(n) => n,
        None => return Ok(TransformAction::Continue),
    };
    let kind = match MdKind::from_str(&name) {
        Some(k) => k,
        None => return Ok(TransformAction::Continue),
    };
    crate::languages::rule::dispatch(xot, node, super::rules::rule(kind))
}
