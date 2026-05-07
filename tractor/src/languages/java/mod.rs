//! Java language module.
//!
//! Java runs through `crate::ir::java` end-to-end. The legacy
//! imperative `rules.rs` / `transformations.rs` / `transform.rs`
//! modules have been retired.
//!
//!   - [`input`]   — generated `JavaKind` enum, kept as a kind-coverage
//!                   catalogue for `tests/kind_catalogue.rs`.
//!   - [`output`]  — semantic-name vocabulary (`TractorNode` enum).
//!   - [`post_transform`] — IR-pipeline post-passes.

pub mod input;
pub mod output;
pub mod post_transform;

pub use output::syntax_category;
pub use post_transform::java_post_transform;
