//! Python language module.
//!
//! As of the IR migration, the imperative pipeline (`rules.rs` /
//! `transformations.rs` / `transform.rs` / `input.rs` (`PyKind`)) has
//! been retired — Python now flows through `crate::ir::python` and
//! is rendered via `crate::ir::render_to_xot`. What remains here:
//!
//!   - [`output`] — semantic-name vocabulary (`TractorNode` enum +
//!                  `NODES_TABLE`) shared by shape contracts and the
//!                  IR's element naming.
//!   - [`post_transform`] — IR-pipeline post-passes (chain inversion,
//!                  visibility-marker injection, list-tagging).

pub mod output;
pub mod post_transform;

pub use output::syntax_category;
pub use post_transform::python_post_transform;
