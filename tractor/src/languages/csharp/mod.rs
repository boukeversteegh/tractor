//! C# language module.
//!
//!   - [`lower`]    — CST → `SyntaxTree` lowering (moved from `tree/csharp.rs`
//!                    in S10A-Z1).
//!   - [`input`]    — generated `CsKind` enum, kept as a kind-coverage
//!                    catalogue for `tests/kind_catalogue.rs`.
//!   - [`output`]   — semantic-name vocabulary (`TractorNode` enum +
//!                    `NODES_TABLE`), shared by shape contracts, the
//!                    reverse renderer, and the tree's element naming.

pub mod lower;
#[cfg(feature = "native")]
pub mod render_source;
pub mod kinds;
pub mod output;

pub use lower::{lower_csharp_root, lower_csharp_node};
pub use output::{ACCESS_MODIFIERS, OTHER_MODIFIERS, syntax_category};
