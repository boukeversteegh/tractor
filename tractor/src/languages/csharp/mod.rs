//! C# language transform pipeline.
//!
//!   - [`input`]    — generated `CsKind` enum (the input vocabulary).
//!   - [`output`]   — semantic-name constants and `NODES` (the output
//!                    vocabulary).
//!   - [`rules`]    — `rule(CsKind) -> Rule`, the input→output table.
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

pub use post_transform::csharp_post_transform;
pub use transform::{transform, syntax_category, ACCESS_MODIFIERS, OTHER_MODIFIERS};
