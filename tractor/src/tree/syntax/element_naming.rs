//! Per-language element-name resolution.
//!
//! The variant-blind renderers default to the variant tag
//! (`snake_case` of the Rust variant) as the XML element name. This
//! module layers per-language overrides on top so users see their
//! own language's vocabulary — e.g. a Python `Namespace` may render
//! as `<package>` while C# uses `<namespace>`, and the legacy
//! per-language root names (`<unit>` / `<program>` / `<file>`) are
//! restored on top of the unified `Module` variant.
//!
//! Two modes:
//! - **Native (default)** — call [`element_name_for_lang`] with
//!   `Some(lang)`. The function consults a per-language override
//!   table and falls back to the variant tag.
//! - **Universal** — call with `None`. Returns the variant tag
//!   verbatim, regardless of language. Useful for cross-language
//!   XPath, JSON round-tripping, and tests that want a single
//!   stable vocabulary.
//!
//! New overrides are added to [`OVERRIDES`] below. The table is a
//! flat `(lang, variant_tag, native_name)` list — easy to scan and
//! diff in PRs, no special build-time generation.

#![cfg(feature = "native")]

/// Per-language native names for variants whose default
/// snake_case-of-variant-tag rendering doesn't match the language's
/// existing vocabulary.
///
/// Format: `(language, variant_tag, native_name)`.
const OVERRIDES: &[(&str, &str, &str)] = &[
    // Root-document naming restored after the C8 Module unification.
    ("csharp", "module", "unit"),
    ("java", "module", "program"),
    ("go", "module", "file"),
    ("rust", "module", "file"),
    ("typescript", "module", "program"),
    ("php", "module", "program"),
    ("ruby", "module", "program"),
    // Future: ("python", "namespace", "package"), etc.
];

/// Resolve the user-facing element name for `variant_tag` in the
/// context of `lang`. With `None`, returns the variant tag verbatim
/// (universal mode).
pub fn element_name_for_lang<'a>(variant_tag: &'a str, lang: Option<&str>) -> &'a str {
    let Some(lang) = lang else { return variant_tag };
    for (l, tag, native) in OVERRIDES {
        if *l == lang && *tag == variant_tag {
            return *native;
        }
    }
    variant_tag
}
