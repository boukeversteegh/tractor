//! PHP language transform pipeline.
//!
//!   - [`input`]    — generated `PhpKind` enum (the input vocabulary).
//!   - [`output`]   — semantic-name constants and `NODES`.
//!   - [`rules`]    — `rule(PhpKind) -> Rule`, the input→output table.
//!   - [`transformations`] — named functions for Rule::Custom + wrappers.
//!   - [`transform`]      — orchestrator.

pub mod input;
pub mod output;
pub mod rules;
pub mod transform;
pub mod transformations;

pub use transform::{transform, syntax_category};
