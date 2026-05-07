//! Python language module.
//!
//! Python runs through `crate::ir::python` end-to-end. The legacy
//! imperative `rules.rs` / `transformations.rs` / `transform.rs`
//! modules have been retired.
//!
//!   - [`input`]   — generated `PyKind` enum, kept as a kind-coverage
//!                   catalogue for `tests/kind_catalogue.rs`.
//!   - [`output`]  — semantic-name vocabulary (`TractorNode` enum +
//!                   `NODES_TABLE`) shared by shape contracts and the
//!                   IR's element naming.
//!   - [`post_transform`] — IR-pipeline post-passes (chain inversion,
//!                   visibility-marker injection, list-tagging).

pub mod input;
pub mod output;
pub mod post_transform;

pub use output::syntax_category;
pub use post_transform::python_post_transform;
