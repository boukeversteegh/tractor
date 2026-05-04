//! Architectural test (iter 335): every language whose
//! `post_transform` calls `chain_inversion::invert_chains_in_tree`
//! (or `wrap_flat_call_member`) MUST declare each name in
//! [`tractor::transform::chain_inversion::EMITTED_NAMES`] in its
//! `TractorNodeSpec` table.
//!
//! Why this exists at TEST level rather than relying on the runtime
//! `name-declared-in-semantic-module` shape rule (layer 2):
//!
//! - Layer 2 only trips when a blueprint actually exercises an
//!   undeclared name. A language with no `arr[i].field` chain in
//!   its blueprint won't trip on a missing `<subscript>` declaration
//!   — yet a future code change could add one and silently land.
//! - This test runs against the static enum tables, not against
//!   any runtime tree. Adding chain inversion to a new language
//!   (or adding a new emitted name to chain_inversion.rs without
//!   updating every enum) trips immediately.
//!
//! See iter 321/325 for the regression class this guards: chain
//! inverter started emitting `<subscript>` for `arr[i].field` chains;
//! several languages had `<call>` and `<member>` declared but not
//! `<subscript>`, only caught when fixture coverage caught up.
//! Iter 328's ROLE_MIXED_PARENTS guard eliminated a sibling
//! regression class architecturally; this test does the same for
//! enum-declaration drift.

use tractor::languages::is_declared_name;
use tractor::transform::chain_inversion::EMITTED_NAMES;

/// The 8 languages whose `post_transform` calls
/// `chain_inversion::invert_chains_in_tree`. Confirmed by
/// `grep -l invert_chains_in_tree tractor/src/languages/*/post_transform.rs`.
const CHAIN_INVERTING_LANGUAGES: &[&str] = &[
    "csharp",
    "go",
    "java",
    "php",
    "python",
    "ruby",
    "rust",
    "typescript",
];

#[test]
fn every_chain_inverting_language_declares_all_emitted_names() {
    let mut violations = Vec::<String>::new();
    for &lang in CHAIN_INVERTING_LANGUAGES {
        for &name in EMITTED_NAMES {
            if !is_declared_name(lang, name) {
                violations.push(format!(
                    "{lang} runs chain inversion but doesn't declare <{name}> in its TractorNodeSpec table"
                ));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "{} chain-inversion enum-drift violation(s):\n  - {}",
        violations.len(),
        violations.join("\n  - ")
    );
}
