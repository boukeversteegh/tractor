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
/// Format: `(language, variant_tag, native_name)`. Looked up first;
/// falls through to [`UNIVERSAL_OVERRIDES`] if no language-specific
/// entry matches.
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

/// Universal element-name renames applied regardless of language.
/// Used for variant tags whose snake_case Rust form differs from
/// the user-facing vocabulary every language shares for that concept.
///
/// Format: `(variant_tag, display_name)`. Applied after the
/// per-language lookup misses.
const UNIVERSAL_OVERRIDES: &[(&str, &str)] = &[
    // EnumMember renders as <constant> universally: every code
    // language calls these "constants" / "members" rather than
    // "enum_member" in user-facing tools.
    ("enum_member", "constant"),
];

/// Resolve the user-facing element name for `variant_tag` in the
/// context of `lang`. With `None`, the per-language layer is skipped
/// but the universal overrides still apply (so JSON round-trips and
/// XPath share the same vocabulary as XML).
pub fn element_name_for_lang<'a>(variant_tag: &'a str, lang: Option<&str>) -> &'a str {
    if let Some(lang) = lang {
        for (l, tag, native) in OVERRIDES {
            if *l == lang && *tag == variant_tag {
                return *native;
            }
        }
    }
    for (tag, display) in UNIVERSAL_OVERRIDES {
        if *tag == variant_tag {
            return *display;
        }
    }
    variant_tag
}

/// Inverse of [`element_name_for_lang`]: given a user-facing
/// `display_name` (e.g. `"program"` for TypeScript), return the
/// canonical variant tag (`"module"`) used by `from_json` dispatch.
/// Falls through to the input unchanged when no override matches —
/// canonical names pass through, unknown names are left for the
/// caller to handle.
pub fn canonical_name_for_lang<'a>(display_name: &'a str, lang: Option<&str>) -> &'a str {
    if let Some(lang) = lang {
        for (l, tag, native) in OVERRIDES {
            if *l == lang && *native == display_name {
                return *tag;
            }
        }
    }
    for (tag, display) in UNIVERSAL_OVERRIDES {
        if *display == display_name {
            return *tag;
        }
    }
    display_name
}
