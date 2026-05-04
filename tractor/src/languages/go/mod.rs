//! Go language transform pipeline.
//!
//!   - [`input`]    — generated `GoKind` enum (the input vocabulary).
//!   - [`output`]   — semantic-name constants and `NODES` (the output
//!                    vocabulary).
//!   - [`rules`]    — `rule(GoKind) -> Rule`, the input→output table.
//!   - [`transformations`] — named functions referenced by
//!                    `Rule::Custom` arms in `rules`.
//!   - [`transform`]      — orchestrator: dispatch each AST node
//!                    through `rules::rule` (or by element name for
//!                    builder-inserted wrappers) to the shared
//!                    [`crate::languages::rule::dispatch`] helper.

pub mod input;
pub mod output;
pub mod post_transform;
pub mod rules;
pub mod transform;
pub mod transformations;

pub use post_transform::go_post_transform;
pub use transform::{transform, syntax_category};
