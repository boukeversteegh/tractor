//! `serde_json::Value` → `SyntaxTree` — reconstruct a synthetic
//! tree from a JSON projection produced by [`super::to_json`].
//!
//! Implementation is **mechanical and generated**: see
//! [`super::metadata_generated::tree_from_json`] (emitted by
//! `tractor/build_codegen.rs::render_from_json`). The generator
//! reads the `SyntaxTree` enum and produces a `$type` dispatch with
//! one constructor per variant, populating each field by type:
//!
//! - **Tree children** (`Box<SyntaxTree>`, `Option<Box<SyntaxTree>>`,
//!   `Vec<SyntaxTree>`, `Expression`, `LambdaBody`, `AccessReceiver`)
//!   pull from `map.get(field_name)` first, falling back to a
//!   flattened "kids" list drained in stable JSON key order.
//! - **Flag / Modifiers** are reconstructed from truthy boolean keys
//!   in the JSON object via `Modifiers::from_marker_names` and the
//!   field-by-field `Flag` lookups.
//! - **Scalar leaves** with stored text (`Name`, `Int`, `Float`,
//!   `String`, `True`, `False`, `None`, `Null`, `Atom`) round-trip
//!   their text value.
//! - **Shape metadata** (`&'static str` element-name fields, etc.)
//!   are derived from the `$type` tag via `Box::leak`; ranges and
//!   spans become synthetic placeholders.
//!
//! Lossy by design — source positions, `Vec<AccessSegment>` chains,
//! and ordering of duplicate-named children are not preserved.
//! Re-rendering through `render_source` should still produce
//! parseable code for typical shapes.
//!
//! See `docs/design-walker-codegen.md` for the architectural plan.

#![cfg(feature = "native")]

use serde_json::{Map, Value};

use super::element_naming::canonical_name_for_lang;
use super::types::SyntaxTree;

/// Reconstruct a `SyntaxTree` from its JSON projection.
///
/// `lang` selects the per-language element-name vocabulary that was
/// used to encode the JSON (mirrors the `lang` argument to
/// [`crate::tree::tree_to_json`]). When provided, this layer
/// reverse-maps per-language display names (`program` for TypeScript,
/// `unit` for C#, `file` for Go/Rust) back to canonical variant tags
/// (`module`) before dispatch — so JSON produced by
/// `tree_to_json(tree, lang)` round-trips cleanly through
/// `tree_from_json(json, lang)` without callers having to translate
/// `$type` themselves. Pass `None` when the JSON already uses
/// canonical variant-tag names (cross-language XPath round-trips).
pub fn tree_from_json(value: &Value, lang: Option<&str>) -> SyntaxTree {
    let normalised = match lang {
        Some(lang) => normalise_display_names(value, lang),
        None => value.clone(),
    };
    super::metadata_generated::tree_from_json(&normalised)
}

/// Deep-walk the JSON tree, rewriting per-language display names
/// (the values of `$type` plus the keys of every object) back to
/// canonical variant tags. The codegen's `$type` dispatch matches
/// canonical names directly; without this normalisation, JSON
/// produced under `lang=typescript` would carry tags like `"program"`
/// that don't appear in the match table.
///
/// Non-variant keys (`text`, `kind`, `op_text`, `$type`, `$children`,
/// ...) aren't in the override map, so `canonical_name_for_lang`
/// passes them through unchanged.
fn normalise_display_names(value: &Value, lang: &str) -> Value {
    match value {
        Value::Object(map) => {
            let mut out = Map::with_capacity(map.len());
            for (key, val) in map {
                let canonical_key = if key == "$type" || key == "$children" {
                    key.clone()
                } else {
                    canonical_name_for_lang(key, Some(lang)).to_string()
                };
                let canonical_val = if key == "$type" {
                    if let Some(s) = val.as_str() {
                        Value::String(canonical_name_for_lang(s, Some(lang)).to_string())
                    } else {
                        val.clone()
                    }
                } else {
                    normalise_display_names(val, lang)
                };
                out.insert(canonical_key, canonical_val);
            }
            Value::Object(out)
        }
        Value::Array(arr) => {
            Value::Array(arr.iter().map(|v| normalise_display_names(v, lang)).collect())
        }
        _ => value.clone(),
    }
}
