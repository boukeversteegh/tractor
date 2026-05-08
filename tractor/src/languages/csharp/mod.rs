//! C# language module.
//!
//! C# runs through `crate::ir::csharp` end-to-end. What remains here:
//!
//!   - [`input`]    — generated `CsKind` enum, kept as a kind-coverage
//!                    catalogue for `tests/kind_catalogue.rs`.
//!   - [`output`]   — semantic-name vocabulary (`TractorNode` enum +
//!                    `NODES_TABLE`), shared by shape contracts, the
//!                    reverse renderer, and the IR's element naming.

pub mod input;
pub mod output;

pub use output::{ACCESS_MODIFIERS, OTHER_MODIFIERS, syntax_category};
