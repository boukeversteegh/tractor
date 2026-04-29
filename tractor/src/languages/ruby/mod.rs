//! Ruby language transform pipeline.
//!
//!   - [`input`]    — generated `RubyKind` enum (the input vocabulary).
//!   - [`output`]   — semantic-name constants and `NODES`.
//!   - [`rules`]    — `rule(RubyKind) -> Rule`, the input→output table.
//!   - [`transformations`] — named functions for Rule::Custom + wrappers.
//!   - [`transform`]      — orchestrator.

pub mod input;
pub mod output;
pub mod rules;
pub mod transform;
pub mod transformations;

pub use transform::{transform, syntax_category};
