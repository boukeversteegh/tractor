//! Env transform orchestrator.
//!
//! Look up the kind, fetch its `Rule` from `rules::rule`, execute via
//! the shared dispatcher. Reshape logic lives in
//! [`super::transformations`].
//!
//! The `EnvKind` enum is a hand-curated subset of bash kinds; bash
//! constructs that don't appear in well-formed .env files fall
//! through the `from_str`-returns-`None` branch as no-ops, which
//! matches the old `_ => Continue` default.

use xot::{Xot, Node as XotNode};
use crate::transform::{TransformAction, helpers::*};

use super::input::EnvKind;

pub fn transform(xot: &mut Xot, node: XotNode) -> Result<TransformAction, xot::Error> {
    let name = match get_element_name(xot, node) {
        Some(n) => n,
        None => return Ok(TransformAction::Continue),
    };
    let kind = match EnvKind::from_str(&name) {
        Some(k) => k,
        None => return Ok(TransformAction::Continue),
    };
    crate::languages::rule::dispatch(xot, node, super::rules::rule(kind))
}
