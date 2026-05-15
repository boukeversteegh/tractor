//! Data-language tree — typed shape for JSON / YAML / TOML / INI / .env / Markdown.
//!
//! Format-agnostic: a single [`DataTree`] tree can be rendered to XML
//! ([`to_xot`]) or JSON ([`to_json`]) regardless of which source format
//! it was lowered from.
//!
//! Module layout (post S10C):
//!   - [`types`]         — the `DataTree` enum and supporting types.
//!   - [`to_xot`]        — `DataTree` → Xot XML rendering.
//!   - [`to_json`]       — `DataTree` → `serde_json::Value` rendering.
//!   - [`lower_json`]    — JSON CST → `DataTree`.
//!   - [`lower_yaml`]    — YAML CST → `DataTree`.
//!   - [`lower_toml`]    — TOML CST → `DataTree`.
//!   - [`lower_ini`]     — INI CST → `DataTree`.
//!   - [`lower_markdown`] — Markdown CST → `DataTree`.

pub mod types;
pub mod to_xot;
pub mod to_json;
pub mod lower_json;
pub mod lower_yaml;
pub mod lower_toml;
pub mod lower_ini;
pub mod lower_markdown;

pub use types::{DataTree, ScalarKind};
