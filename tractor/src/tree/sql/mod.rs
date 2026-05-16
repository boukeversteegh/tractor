//! SQL-language tree — typed variants per construct, parallel to
//! `SyntaxTree` (programming languages) and `DataTree` (data languages).
//!
//! T-SQL lowering lives at `crate::languages::tsql::lower` (relocated
//! in S10D, matching the S10A pattern for programming languages).
//!
//! Module layout (post S10D):
//!   - [`types`]         — the `SqlTree` enum and supporting types.
//!   - [`to_xot`]        — `SqlTree` → Xot XML rendering.
//!   - [`to_json`]       — `SqlTree` → `serde_json::Value` rendering.
//!   - [`render_source`] — `SqlTree` → SQL source text (canonical mode).

pub mod types;
pub mod to_xot;
#[cfg(feature = "native")]
pub mod to_json;
#[cfg(feature = "native")]
pub mod render_source;

// Auto-generated reflection metadata, parallel to
// `tree/syntax/metadata.generated.rs` and `tree/data/metadata.generated.rs`.
#[cfg(feature = "native")]
#[path = "metadata.generated.rs"]
pub mod metadata_generated;

pub use types::{ComparisonOp, CreateKind, DropKind, JoinKind, SortDirection, SqlTree};
// QuoteStyle is the shared cross-tree enum (since the Tier 1 SqlTree
// unification). Re-exported here so `tree::sql::QuoteStyle` keeps
// working for existing callers.
pub use crate::tree::types::QuoteStyle;
