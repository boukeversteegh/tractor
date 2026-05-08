//! Java language module.
//!
//! Java runs through `crate::ir::java` end-to-end.
//!
//!   - [`input`]   — generated `JavaKind` enum, kept as a kind-coverage
//!                   catalogue for `tests/kind_catalogue.rs`.
//!   - [`output`]  — semantic-name vocabulary (`TractorNode` enum).

pub mod input;
pub mod output;

pub use output::syntax_category;
