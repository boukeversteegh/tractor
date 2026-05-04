//! TypeScript / JavaScript language transform pipeline.
//!
//!   - [`input`]    — generated `TsKind` enum (the input vocabulary,
//!                    union of typescript + tsx grammars).
//!   - [`output`]   — semantic-name constants and `NODES`.
//!   - [`rules`]    — `rule(TsKind) -> Rule`, the input→output table.
//!   - [`transformations`] — named functions for Rule::Custom + wrappers.
//!   - [`transform`]      — orchestrator.

pub mod input;
pub mod output;
pub mod post_transform;
pub mod rules;
pub mod transform;
pub mod transformations;

pub use post_transform::typescript_post_transform;
pub use transform::{transform, syntax_category};
