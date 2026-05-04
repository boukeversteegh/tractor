//! Rust language transform pipeline.
//!
//!   - [`input`]    — generated `RustKind` enum (the input vocabulary).
//!   - [`output`]   — semantic-name constants and `NODES`.
//!   - [`rules`]    — `rule(RustKind) -> Rule`, the input→output table.
//!   - [`transformations`] — named functions for Rule::Custom + wrappers.
//!   - [`transform`]      — orchestrator.
//!   - [`post_transform`] — post-walk pipeline (chain inversion, use
//!     restructure, lifetime-name normalize, list distribution).

pub mod input;
pub mod output;
pub mod post_transform;
pub mod rules;
pub mod transform;
pub mod transformations;

pub use post_transform::rust_post_transform;
pub use transform::{transform, syntax_category};
