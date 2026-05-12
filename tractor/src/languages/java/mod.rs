//! Java language module.
//!
//! Java runs through `crate::tree::java` end-to-end.
//!
//!   - [`input`]   — generated `JavaKind` enum, kept as a kind-coverage
//!                   catalogue for `tests/kind_catalogue.rs`.
//!   - [`output`]  — semantic-name vocabulary (`TractorNode` enum).

#[cfg(feature = "native")]
pub mod lower;
#[cfg(feature = "native")]
pub mod render_source;
pub mod input;
pub mod output;

#[cfg(feature = "native")]
pub use lower::{lower_java_root, lower_java_node};
pub use output::syntax_category;
