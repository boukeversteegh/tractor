//! Tree → XML projection — **stub state**.
//!
//! The per-variant `render_tree_*` arms have been removed. The
//! upcoming renderer is fully generated from the `SyntaxTree` enum
//! per `docs/design-walker-codegen.md` — a single mechanical walk
//! with no per-variant knowledge. This file keeps the public
//! `render_to_xot` API alive so callers compile while the codegen
//! is being built.
//!
//! Until the generated renderer lands, every parsed program
//! renders as a single `<xml/>` element. That is deliberate: it
//! deletes the surface area where a "just this one variant" rule
//! could re-enter the renderer. Every snapshot / XPath test will
//! fail; we'll bring them back online once the generated path
//! exists.
//!
//! See `tractor/src/bin/gen_walker.rs` (forthcoming) for the
//! generator. See the design doc for the `#[shape(...)]` attribute
//! contract.

#![cfg(feature = "native")]

use xot::{Node as XotNode, Xot};

use super::types::SyntaxTree;

/// Stub renderer. Emits a single `<xml/>` element under `parent`.
///
/// The generated walker will replace this entirely; this stub
/// exists only to keep the call sites in `parser/mod.rs` compiling.
pub fn render_to_xot(
    xot: &mut Xot,
    parent: XotNode,
    _tree: &SyntaxTree,
    _source: &str,
) -> Result<XotNode, xot::Error> {
    let name = xot.add_name("xml");
    let node = xot.new_element(name);
    xot.append(parent, node)?;
    Ok(node)
}
