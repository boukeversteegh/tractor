//! Output element names — TOML's vocabulary after transform.
//!
//! Table / pair keys become user-driven element names, so the output
//! vocabulary is open. Only `<item>` (used for array elements and
//! `[[table]]` array entries) is a closed structural name.
//!
//! No `TractorNodeSpec` table: data-language convention shared with JSON,
//! YAML, and INI — `node_spec` stays `None` in the language registry.

pub const ITEM: &str = "item";
