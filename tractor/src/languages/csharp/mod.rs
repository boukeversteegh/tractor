//! C# language module.
//!
//! As of the IR migration, the imperative pipeline (`rules.rs` /
//! `transformations.rs` / `transform.rs` / `input.rs` (`CsKind`)) has
//! been retired — all C# transforms now flow through `crate::ir::csharp`
//! and are rendered via `crate::ir::render_to_xot`. What remains here:
//!
//!   - [`output`] — semantic-name vocabulary (`TractorNode` enum +
//!                  `NODES_TABLE`), shared by shape contracts, the
//!                  reverse renderer, and the IR's element naming.
//!   - [`post_transform`] — IR-pipeline post-passes that run after
//!                  XML rendering (chain inversion, `where`-clause
//!                  attachment, list distribution, …).

pub mod output;
pub mod post_transform;

pub use post_transform::csharp_post_transform;
pub use output::{ACCESS_MODIFIERS, OTHER_MODIFIERS, syntax_category};
