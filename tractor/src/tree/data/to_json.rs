//! [`DataTree`] → `serde_json::Value` — variant-blind tree view.
//!
//! Thin wrapper around the generic walker in [`crate::tree::walker`].
//! Same role as [`crate::tree::syntax::to_json::tree_to_json`] /
//! [`crate::tree::sql::to_json::sql_to_json`]: a `$type`-discriminated
//! projection of the tree's *structure* (variant tags, marker
//! children, flat child lists). One uniform shape across all three
//! tree families — XPath result reporting, tree-shape inspection,
//! and round-trip-friendly representations consume this view.
//!
//! Counterpart to the content-shape JSON renderer in
//! [`crate::languages::json::render_source::render_json_value`]:
//! that one treats a `DataTree` as JSON content (`{"name": "x"}`);
//! this one treats it as a typed tree (`{"$type": "mapping", ...}`).
//! Two different jobs — file output vs structural projection.

#![cfg(feature = "native")]

use serde_json::Value;

use crate::tree::DataTree;
use crate::tree::walker::render_walker_to_json;

/// Render a [`DataTree`] tree to its variant-blind JSON projection.
///
/// `format` selects per-format element-name overrides (e.g. JSON's
/// `<object>`/`<array>`/`<property>` vs YAML's `<mapping>`/
/// `<sequence>`). Pass `None` for the abstract variant-tag names —
/// useful for cross-format diffing and round-trip-friendly output.
pub fn data_to_json(tree: &DataTree) -> Value {
    // Default to no per-format overlay — the variant-tag names are
    // already the canonical "abstract" vocabulary. Specific callers
    // that want JSON-flavoured names can call
    // [`data_to_json_for_format`] with `Some("json")`.
    render_walker_to_json(tree, "", None)
}

/// Same as [`data_to_json`] but threads `format` through the
/// per-format element-name overlay (`"json"`, `"yaml"`, `"toml"`,
/// `"ini"`). Source is unused — `DataTree` carries its own scalar
/// text — but kept on the signature for parity with `tree_to_json`.
pub fn data_to_json_for_format(tree: &DataTree, format: Option<&str>) -> Value {
    render_walker_to_json(tree, "", format)
}
