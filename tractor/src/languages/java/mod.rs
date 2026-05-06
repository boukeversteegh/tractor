//! Java language module.
//!
//! As of the IR migration, the imperative pipeline (`rules.rs` /
//! `transformations.rs` / `transform.rs` / `input.rs` (`JavaKind`))
//! has been retired — Java now flows through `crate::ir::java` and
//! is rendered via `crate::ir::render_to_xot`.
//!
//!   - [`output`] — semantic-name vocabulary (`TractorNode` enum).
//!   - [`post_transform`] — IR-pipeline post-passes.

pub mod output;
pub mod post_transform;

pub use output::syntax_category;
pub use post_transform::java_post_transform;
