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

#![cfg(feature = "native")]

pub mod types;
pub mod to_xot;
pub mod to_json;
pub mod render_source;

pub use types::{ComparisonOp, CreateKind, DropKind, JoinKind, QuoteStyle, SortDirection, SqlTree};
