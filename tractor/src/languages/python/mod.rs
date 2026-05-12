//! Python language module.
//!
//!   - [`lower`]   — CST → `SyntaxTree` lowering (moved from `tree/python.rs`
//!                   in S10A-Z2).
//!   - [`input`]   — generated `PyKind` enum, kept as a kind-coverage
//!                   catalogue for `tests/kind_catalogue.rs`.
//!   - [`output`]  — semantic-name vocabulary (`TractorNode` enum +
//!                   `NODES_TABLE`) shared by shape contracts and the
//!                   tree's element naming.

#[cfg(feature = "native")]
pub mod lower;
pub mod input;
pub mod output;

#[cfg(feature = "native")]
pub use lower::lower_python_root;
pub use output::syntax_category;
