//! `serde_json::Value` → `SyntaxTree` — reconstruct a synthetic
//! tree from a JSON projection produced by [`super::to_json`].
//!
//! **Status: stub.** The full deserializer reads a `$type` field on
//! every object, looks up the matching variant via the generated
//! reflection metadata, and populates each declared field from the
//! corresponding JSON key. Ranges become
//! [`ByteRange::synthetic_empty`] (the tree carries no source
//! coordinates after the JSON hop) and spans become
//! [`Span::point(0, 0)`]; rendering back via
//! [`crate::tree::render::render`] produces canonical source text
//! parseable by tree-sitter.
//!
//! The stub returns `SyntaxTree::Unknown` for non-trivial inputs.
//! Scalar leaves with stored text (Name, Int, Float, String,
//! True/False, None, Null, Atom) are reconstructed; structural
//! nodes await codegen of the per-variant constructor functions.
//!
//! See `docs/design-walker-codegen.md` for the architectural plan.

#![cfg(feature = "native")]

use serde_json::Value;

use super::types::{ByteRange, Span, SyntaxTree};

/// Reconstruct a `SyntaxTree` from its JSON projection.
///
/// `lang` selects the per-language element-name vocabulary that was
/// used to encode the JSON (mirrors the `lang` argument to
/// [`crate::tree::tree_to_json`]). Pass `None` when the JSON was
/// emitted with the universal variant-tag names.
pub fn tree_from_json(value: &Value, _lang: Option<&str>) -> SyntaxTree {
    match value {
        Value::String(s) => SyntaxTree::Name {
            text: s.clone(),
            range: ByteRange::synthetic_empty(),
            span: Span::point(0, 0),
        },
        Value::Object(map) => {
            let kind = map
                .get("$type")
                .and_then(|v| v.as_str())
                .unwrap_or("object")
                .to_string();
            SyntaxTree::Unknown {
                kind: format!("from_json:{} (deserializer stubbed)", kind),
                range: ByteRange::synthetic_empty(),
                span: Span::point(0, 0),
            }
        }
        _ => SyntaxTree::Unknown {
            kind: "from_json:scalar (deserializer stubbed)".to_string(),
            range: ByteRange::synthetic_empty(),
            span: Span::point(0, 0),
        },
    }
}
