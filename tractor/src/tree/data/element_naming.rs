//! Per-format element-name resolution for `DataTree`.
//!
//! Mirrors `tree/syntax/element_naming.rs`: the variant-blind walker
//! defaults to the variant tag (snake_case) as the element name, and
//! a per-context overlay maps abstract names onto each format's
//! vocabulary:
//!
//! | DataTree variant | JSON         | YAML         | TOML / INI   |
//! |------------------|--------------|--------------|--------------|
//! | Mapping          | `<object>`   | `<mapping>`  | `<table>`    |
//! | Sequence         | `<array>`    | `<sequence>` | `<array>`    |
//! | Pair             | `<property>` | `<property>` | `<property>` |
//!
//! Most variants need no override (`<string>`, `<number>`, `<bool>`,
//! `<null>`, `<comment>`, ...).
//!
//! Keyed-mode walkers (YAML / TOML / INI / Markdown) lift the Pair
//! key into the element name and need a structurally different
//! lowering or a custom walker — that's tracked separately. This
//! table covers naming only.

#![cfg(feature = "native")]

/// Per-format override table. Format: `(format, variant_tag, native_name)`.
/// Looked up by the [`WalkerTree::display_name_for`](crate::tree::walker::WalkerTree::display_name_for)
/// impl on `DataTree`. Missing entries fall back to the variant tag
/// (the abstract snake_case form, matching `metadata_generated`).
const OVERRIDES: &[(&str, &str, &str)] = &[
    // JSON-style (JSON / JSON5) — keys per CodeSearch convention.
    ("json", "mapping", "object"),
    ("json", "sequence", "array"),
    ("json", "pair", "property"),
    // YAML keeps the abstract `<mapping>` / `<sequence>` names; only
    // the Pair → `<property>` rename applies.
    ("yaml", "pair", "property"),
    // TOML uses `<table>` for `[section]` mappings, `<array>` for
    // arrays-of-tables. Pair becomes `<property>` for parity.
    ("toml", "mapping", "table"),
    ("toml", "sequence", "array"),
    ("toml", "pair", "property"),
    // INI is structurally simpler — sections only. Mapping rename
    // keeps queries consistent with TOML.
    ("ini", "mapping", "table"),
    ("ini", "pair", "property"),
];

/// Resolve the user-facing element name for `variant_tag` in the
/// context of `format`. With `None`, the per-format layer is
/// skipped and the abstract variant tag passes through verbatim.
pub fn element_name_for_format<'a>(variant_tag: &'a str, format: Option<&str>) -> &'a str {
    if let Some(fmt) = format {
        for (f, tag, native) in OVERRIDES {
            if *f == fmt && *tag == variant_tag {
                return *native;
            }
        }
    }
    variant_tag
}
