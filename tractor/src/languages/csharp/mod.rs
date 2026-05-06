//! C# language module.
//!
//! C# runs through `crate::ir::csharp` end-to-end. The legacy
//! imperative `rules.rs` / `transformations.rs` / `transform.rs`
//! modules have been retired. What remains here:
//!
//!   - [`input`]    — generated `CsKind` enum, kept as a kind-coverage
//!                    catalogue for `tests/kind_catalogue.rs`.
//!   - [`output`]   — semantic-name vocabulary (`TractorNode` enum +
//!                    `NODES_TABLE`), shared by shape contracts, the
//!                    reverse renderer, and the IR's element naming.
//!   - [`post_transform`] — IR-pipeline post-passes that run after
//!                    XML rendering (chain inversion, `where`-clause
//!                    attachment, list distribution, …).

pub mod input;
pub mod output;
pub mod post_transform;

pub use post_transform::csharp_post_transform;
pub use output::{ACCESS_MODIFIERS, OTHER_MODIFIERS, syntax_category};
