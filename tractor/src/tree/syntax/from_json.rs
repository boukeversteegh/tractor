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

use serde_json::Value;

use super::types::SyntaxTree;

/// Reconstruct a `SyntaxTree` from its JSON projection.
///
/// `lang` selects the per-language element-name vocabulary that was
/// used to encode the JSON (mirrors the `lang` argument to
/// [`crate::tree::tree_to_json`]). Pass `None` when the JSON was
/// emitted with the universal variant-tag names. The deserializer
/// is currently `lang`-agnostic — the `$type` tag is matched against
/// the universal variant name set; per-language native names
/// (`unit` / `program` / `file` for Module) are not yet inverted.
pub fn tree_from_json(value: &Value, _lang: Option<&str>) -> SyntaxTree {
    super::metadata_generated::tree_from_json(value)
}
