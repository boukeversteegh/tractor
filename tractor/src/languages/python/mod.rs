//! Python language module.
//!
//! Python runs through `crate::ir::python` end-to-end.
//!
//!   - [`input`]   — generated `PyKind` enum, kept as a kind-coverage
//!                   catalogue for `tests/kind_catalogue.rs`.
//!   - [`output`]  — semantic-name vocabulary (`TractorNode` enum +
//!                   `NODES_TABLE`) shared by shape contracts and the
//!                   IR's element naming.

pub mod input;
pub mod output;

pub use output::syntax_category;
