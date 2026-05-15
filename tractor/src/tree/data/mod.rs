//! Data-language tree — typed shape for JSON / YAML / TOML / INI / .env / Markdown.
//!
//! Format-agnostic: a single [`DataTree`] tree can be rendered to XML
//! ([`to_xot`]) or JSON ([`to_json`]) regardless of which source format
//! it was lowered from.
//!
//! ## Layout
//!
//! - [`types`]         — the `DataTree` enum and supporting types.
//! - [`to_xot`]        — `DataTree` → Xot XML rendering.
//! - [`to_json`]       — `DataTree` → `serde_json::Value` rendering.
//! - [`render_common`] — shared helpers for the per-language
//!                       `render_source.rs` files (DataTree → source
//!                       text).
//!
//! Per-language lowering (CST → DataTree) and source-text rendering
//! (DataTree → JSON / YAML / … text) live under
//! `tractor/src/languages/{json,yaml,toml,ini,markdown}/{lower,render_source}.rs`,
//! mirroring the layout used by code languages.

pub mod types;
pub mod to_xot;
pub mod to_json;

// Shared helpers consumed by the per-language DataTree → source-text
// renderers at `languages/{json,yaml}/render_source.rs`.
#[cfg(feature = "native")]
pub mod render_common;

pub use types::{DataTree, ScalarKind};
