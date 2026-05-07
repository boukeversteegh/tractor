//! Architectural test: every language whose `post_transform` calls
//! a shared transform helper MUST declare each name that helper
//! emits programmatically in its `TractorNodeSpec` table.
//!
//! Why this exists at TEST level rather than relying on the runtime
//! `name-declared-in-semantic-module` shape rule (layer 2):
//!
//! - Layer 2 only trips when a blueprint actually exercises an
//!   undeclared name. A language with no `arr[i].field` chain in
//!   its blueprint won't trip on a missing `<subscript>` declaration
//!   — yet a future code change could add one and silently land.
//! - This test runs against the static enum tables, not against
//!   any runtime tree. Adding a shared transform to a new language
//!   (or adding a new emitted name to a shared helper without
//!   updating every enum) trips immediately.
//!
//! Two contracts are checked:
//!
//! 1. **Chain inversion** (iter 335): every language whose
//!    `post_transform` calls `chain_inversion::invert_chains_in_tree`
//!    or `wrap_flat_call_member` must declare each name in
//!    `chain_inversion::EMITTED_NAMES`. Caught Ruby missing
//!    `Property` on first run.
//!
//! 2. **Shared post-walk helpers** (iter 337): every language whose
//!    `post_transform` calls `wrap_expression_positions`,
//!    `wrap_body_value_children`, or `wrap_relationship_targets_in_type`
//!    must declare each name in `transform::EMITTED_NAMES`.
//!
//! Pattern-mirrors iter 328's ROLE_MIXED_PARENTS architectural guard:
//! both prevent regression classes from reintroducing themselves
//! rather than relying on belated audits.

use tractor::languages::is_declared_name;

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

/// The 8 languages whose `post_transform` calls
/// `wrap_expression_positions` (and/or `wrap_body_value_children`).
/// All chain-inverting languages happen to use these too — confirmed
/// by `grep -l wrap_expression_positions
/// tractor/src/languages/*/post_transform.rs`.
const EXPRESSION_HOST_LANGUAGES: &[&str] = &[
    "csharp",
    "go",
    "java",
    "php",
    "python",
    "ruby",
    "rust",
    "typescript",
];

/// The 4 languages whose `post_transform` calls
/// `wrap_relationship_targets_in_type`. Confirmed by
/// `grep -l wrap_relationship_targets_in_type tractor/src/languages/*/post_transform.rs`.
const RELATIONSHIP_TARGET_LANGUAGES: &[&str] = &[
    "csharp",
    "php",
    "python",
    "ruby",
];

#[test]
fn every_chain_inverting_language_declares_all_emitted_names() {
    use tractor::transform::chain_inversion::EMITTED_NAMES;
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

#[test]
fn every_language_using_shared_transforms_declares_their_emitted_names() {
    let mut violations = Vec::<String>::new();
    // <expression> is emitted by wrap_expression_positions and
    // wrap_body_value_children — every chain-inverting language uses
    // at least one of them.
    for &lang in EXPRESSION_HOST_LANGUAGES {
        if !is_declared_name(lang, "expression") {
            violations.push(format!(
                "{lang} uses wrap_expression_positions but doesn't declare <expression> in its TractorNodeSpec table"
            ));
        }
    }
    // <type> is emitted by wrap_relationship_targets_in_type when it
    // wraps a bare <name> child of a relationship slot.
    for &lang in RELATIONSHIP_TARGET_LANGUAGES {
        if !is_declared_name(lang, "type") {
            violations.push(format!(
                "{lang} uses wrap_relationship_targets_in_type but doesn't declare <type> in its TractorNodeSpec table"
            ));
        }
    }
    assert!(
        violations.is_empty(),
        "{} shared-transform enum-drift violation(s):\n  - {}",
        violations.len(),
        violations.join("\n  - ")
    );
}
