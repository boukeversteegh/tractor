//! T-SQL language transform pipeline.
//!
//!   - [`lower`]    — CST → `SqlTree` lowering (moved from `tree/sql_lower.rs`
//!                    in S10D).
//!   - [`kinds`]    — generated `TsqlKind` enum (the input vocabulary).
//!   - [`output`]   — semantic-name constants and `NODES`.
//!   - [`rules`]    — `rule(TsqlKind) -> Rule`, the input→output table.
//!   - [`transformations`] — named functions for Rule::Custom + wrappers.
//!   - [`transform`]      — orchestrator.

pub mod lower;
pub mod kinds;
pub mod output;
pub mod rules;
pub mod transform;
pub mod transformations;

pub use lower::lower_sql_root;
pub use transform::{transform, syntax_category};
