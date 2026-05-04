//! Shape contracts — declarative invariants over the *transformed* tree
//! shape, validated at the predicate-engine boundary so that both the
//! cargo integration test (`tractor/tests/shape_contracts.rs`) and the
//! debug-build end-of-`post_transform` assertion call the same code.
//!
//! See `docs/transform-validation-architecture.md` for design rationale
//! and the layered-defense ordering. Phase 1 ships layers 1+2:
//!
//! - **Layer 1** — cargo test iterates blueprint fixtures, runs every
//!   rule, fails the build on `Severity::Error` violations and reports
//!   `Severity::Advisory` ones without failing.
//! - **Layer 2** — `assert_shape_contracts` is invoked in debug builds
//!   from `transform::builder.rs` immediately after a language's
//!   `post_transform` returns. Catches regressions at first transform
//!   invocation, not just when the cargo test runs.
//!
//! Predicates are plain Rust walkers (matches the style of
//! `tractor/tests/tree_invariants.rs`). XPath is overkill for the
//! current rule set; a hybrid `RulePredicate::XPath(_)` variant can be
//! added later if a contract needs it.

use xot::{Node as XotNode, Xot};

use crate::transform::helpers::{get_attr, get_element_name};

/// Severity of a shape-contract violation.
///
/// `Error` violations panic in debug builds (layer 2) and fail the cargo
/// integration test (layer 1). `Advisory` violations report diagnostics
/// without failing — used for rules whose target population is not yet
/// at zero (e.g. `no-children-overflow` while the cross-language
/// design-call class is still open).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Advisory,
    Error,
}

/// One rule-violation occurrence.
#[derive(Debug, Clone)]
pub struct Violation {
    /// Stable identifier of the rule that fired.
    pub rule_id: &'static str,
    /// One-line human-readable summary of *this* occurrence (location,
    /// element name, etc.). The rule's general description is on the
    /// `ShapeRule` itself.
    pub message: String,
    /// Severity inherited from the rule.
    pub severity: Severity,
}

/// Type signature for a rule's predicate.
///
/// Walks the tree rooted at `root` and appends one `Violation` per
/// occurrence to `out`. `lang` is supplied so a rule can scope itself
/// to specific languages if needed.
pub type CheckFn = fn(xot: &Xot, root: XotNode, lang: &str, out: &mut Vec<Violation>);

/// A single declarative shape contract.
///
/// Static-only fields so the `RULES` table can live as a `&'static`
/// slice and rules can be referenced by id from anywhere.
pub struct ShapeRule {
    pub id: &'static str,
    pub description: &'static str,
    pub severity: Severity,
    pub check: CheckFn,
}

// ---------------------------------------------------------------------------
// Helper — recursive element walk.
// ---------------------------------------------------------------------------

fn walk_elements<F: FnMut(&Xot, XotNode)>(xot: &Xot, node: XotNode, f: &mut F) {
    if xot.element(node).is_some() {
        f(xot, node);
    }
    for child in xot.children(node) {
        walk_elements(xot, child, f);
    }
}

/// True iff `node` is an element with no element children and no
/// non-whitespace text children. Used by the marker/wrapper-collision
/// check to distinguish empty markers (`<X/>`) from structural
/// wrappers (`<X>...</X>`).
fn is_empty_element(xot: &Xot, node: XotNode) -> bool {
    if xot.element(node).is_none() {
        return false;
    }
    let has_element_child = xot.children(node).any(|c| xot.element(c).is_some());
    if has_element_child {
        return false;
    }
    let has_real_text = xot
        .children(node)
        .any(|c| xot.text_str(c).map_or(false, |t| !t.trim().is_empty()));
    !has_real_text
}

// ---------------------------------------------------------------------------
// Rule predicates.
// ---------------------------------------------------------------------------

/// Rule `no-children-overflow` — when JSON-serializing a parent, the
/// renderer's behavior on same-named element children is:
///
/// - children carrying `list="X"` are grouped into one JSON array;
/// - children without `list=` lift one as the singleton JSON key for
///   that name, and remaining untagged siblings fall through to an
///   anonymous `$children:` overflow array (sigil-prefixed so a real
///   `<children>` element name doesn't collide).
///
/// Therefore the actual overflow condition is **2+ untagged children
/// sharing a name** under the same parent. (Mixed "1 untagged + N
/// tagged" is fine — the tagged ones form their array, the untagged
/// one is the singleton key.)
///
/// Severity: `Advisory` for now (9 known sites in the cross-language
/// design-call class as of iter 291). Promote to `Error` once the
/// audit reaches zero.
fn check_no_children_overflow(
    xot: &Xot,
    root: XotNode,
    _lang: &str,
    out: &mut Vec<Violation>,
) {
    walk_elements(xot, root, &mut |xot, node| {
        // Group element children by name, counting only those WITHOUT
        // a `list=` attribute. A name with 2+ untagged children
        // overflows on JSON serialization.
        use std::collections::HashMap;
        let mut untagged_per_name: HashMap<String, u32> = HashMap::new();
        for child in xot.children(node) {
            let Some(child_name) = get_element_name(xot, child) else { continue };
            if get_attr(xot, child, "list").is_some() {
                continue;
            }
            *untagged_per_name.entry(child_name).or_insert(0) += 1;
        }
        for (name, count) in &untagged_per_name {
            if *count < 2 {
                continue;
            }
            let parent_name = get_element_name(xot, node).unwrap_or_default();
            let line = get_attr(xot, node, "line").unwrap_or_default();
            out.push(Violation {
                rule_id: "no-children-overflow",
                message: format!(
                    "<{parent_name}> (line {line}) has {count} untagged <{name}> children — JSON $children: overflow",
                ),
                severity: Severity::Advisory,
            });
        }
    });
}

/// Rule `no-marker-wrapper-collision` — a parent must not have both an
/// empty marker child `<X/>` AND a structural wrapper child
/// `<X>...</X>` with the same name. JSON serializer collides on the
/// singleton key `X`. iter 184 archetype.
///
/// Excludes element names that legitimately appear in both shapes
/// (dual-use). For now, the dual-use exclusions are derived from
/// per-language `TractorNodeSpec` declarations: a name with both
/// `marker: true` AND `container: true` is exempt.
///
/// Severity: `Error` — the archetype is at zero across blueprints as
/// of iter 184/275/283.
fn check_no_marker_wrapper_collision(
    xot: &Xot,
    root: XotNode,
    lang: &str,
    out: &mut Vec<Violation>,
) {
    let lookup = crate::languages::get_language(lang).and_then(|l| l.node_spec);
    walk_elements(xot, root, &mut |xot, node| {
        // Group element children by name; record empty-vs-non-empty
        // counts per name on this parent.
        use std::collections::HashMap;
        struct Counts {
            empty: u32,
            wrapper: u32,
        }
        let mut by_name: HashMap<String, Counts> = HashMap::new();
        for child in xot.children(node) {
            let Some(child_name) = get_element_name(xot, child) else { continue };
            let entry = by_name.entry(child_name).or_insert(Counts { empty: 0, wrapper: 0 });
            if is_empty_element(xot, child) {
                entry.empty += 1;
            } else {
                entry.wrapper += 1;
            }
        }
        for (name, c) in &by_name {
            if c.empty == 0 || c.wrapper == 0 {
                continue; // not a collision: only one shape present
            }
            // Dual-use names are exempt — the language declares both
            // shapes valid (e.g. `<new/>` marker AND `<new>...</new>`
            // wrapper coexist on construction sites).
            if let Some(lookup) = lookup {
                if let Some(spec) = lookup(name) {
                    if spec.marker && spec.container {
                        continue;
                    }
                }
            }
            let parent_name = get_element_name(xot, node).unwrap_or_default();
            let line = get_attr(xot, node, "line").unwrap_or_default();
            out.push(Violation {
                rule_id: "no-marker-wrapper-collision",
                message: format!(
                    "<{parent_name}> (line {line}) has both empty <{name}/> marker and <{name}>…</{name}> wrapper sibling — JSON key collision"
                ),
                severity: Severity::Error,
            });
        }
    });
}

/// Rule `marker-stays-empty` — first phase-2 migration: a name
/// declared `NodeRole::MarkerOnly` in its language's
/// `TractorNodeSpec` must, when emitted, be empty (no text, no
/// element children).
///
/// Mirrors the existing `markers_stay_empty` invariant in
/// `tree_invariants.rs` (asserted at zero since iter 67), but driven
/// by the unified `NodeRole` derivation rather than the bespoke
/// `is_marker_only_name` lookup. Phase 2 keeps both during migration;
/// the hand-coded version retires once the role-driven walker has
/// proven parity across cycles.
///
/// Severity: `Error`. Pinned at 0.
fn check_marker_stays_empty(
    xot: &Xot,
    root: XotNode,
    lang: &str,
    out: &mut Vec<Violation>,
) {
    let lookup = crate::languages::get_language(lang).and_then(|l| l.node_spec);
    let Some(lookup) = lookup else { return };
    walk_elements(xot, root, &mut |xot, node| {
        let Some(name) = get_element_name(xot, node) else { return };
        let Some(spec) = lookup(&name) else { return };
        if spec.role() != crate::languages::NodeRole::MarkerOnly {
            return;
        }
        // MarkerOnly must be empty. Mirrors `is_empty_element` but
        // with explicit reporting of which contamination was found.
        if let Some(child) = xot.children(node).find(|&c| xot.element(c).is_some()) {
            let child_name = get_element_name(xot, child).unwrap_or_default();
            let line = get_attr(xot, node, "line").unwrap_or_default();
            out.push(Violation {
                rule_id: "marker-stays-empty",
                message: format!(
                    "<{name}/> (line {line}) is MarkerOnly but has element child <{child_name}>"
                ),
                severity: Severity::Error,
            });
            return;
        }
        if let Some(child) = xot.children(node).find(|&c| {
            xot.text_str(c).map_or(false, |s| !s.trim().is_empty())
        }) {
            let text = xot
                .text_str(child)
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            let shown = if text.len() > 40 {
                format!("{}…", &text[..40])
            } else {
                text
            };
            let line = get_attr(xot, node, "line").unwrap_or_default();
            out.push(Violation {
                rule_id: "marker-stays-empty",
                message: format!(
                    "<{name}/> (line {line}) is MarkerOnly but has text {shown:?}"
                ),
                severity: Severity::Error,
            });
        }
    });
}

/// Rule `name-is-text-leaf` — `<name>` is reserved for identifier text
/// leaves. Element children inside `<name>` indicate a wrapper that
/// didn't get inlined (e.g. `<name><type>Foo</type></name>`).
///
/// Severity: `Error`. Mirrors the existing `name_element_is_text_leaf`
/// invariant in `tree_invariants.rs`. Phase 2 retires the hand-coded
/// version once the spec-conformance walker reaches parity.
fn check_name_is_text_leaf(
    xot: &Xot,
    root: XotNode,
    _lang: &str,
    out: &mut Vec<Violation>,
) {
    walk_elements(xot, root, &mut |xot, node| {
        let Some(name) = get_element_name(xot, node) else { return };
        if name != "name" {
            return;
        }
        let Some(child) = xot.children(node).find(|&c| xot.element(c).is_some()) else {
            return;
        };
        let child_name = get_element_name(xot, child).unwrap_or_default();
        let line = get_attr(xot, node, "line").unwrap_or_default();
        out.push(Violation {
            rule_id: "name-is-text-leaf",
            message: format!(
                "<name> (line {line}) has element child <{child_name}> — identifiers are text-only leaves"
            ),
            severity: Severity::Error,
        });
    });
}

// ---------------------------------------------------------------------------
// Rule table — single source of truth, consumed by both layers.
// ---------------------------------------------------------------------------

pub static RULES: &[ShapeRule] = &[
    ShapeRule {
        id: "no-children-overflow",
        description: "JSON children-overflow — same-name siblings collided on a singleton key without role-named slot wrappers or list= tagging.",
        severity: Severity::Advisory,
        check: check_no_children_overflow,
    },
    ShapeRule {
        id: "no-marker-wrapper-collision",
        description: "Parent has both empty <X/> marker and <X>…</X> wrapper sibling — JSON key collision (iter 184 archetype).",
        severity: Severity::Error,
        check: check_no_marker_wrapper_collision,
    },
    ShapeRule {
        id: "marker-stays-empty",
        description: "A NodeRole::MarkerOnly element must have no text and no element children.",
        severity: Severity::Error,
        check: check_marker_stays_empty,
    },
    ShapeRule {
        id: "name-is-text-leaf",
        description: "<name> is reserved for identifier text leaves; element children indicate an un-inlined wrapper.",
        severity: Severity::Error,
        check: check_name_is_text_leaf,
    },
];

// ---------------------------------------------------------------------------
// Public entry points.
// ---------------------------------------------------------------------------

/// Run every rule against the tree rooted at `root` and return the
/// flat list of violations. Empty result = clean.
pub fn validate_shape_contracts(xot: &Xot, root: XotNode, lang: &str) -> Vec<Violation> {
    let mut out = Vec::new();
    for rule in RULES {
        (rule.check)(xot, root, lang, &mut out);
    }
    out
}

/// Debug-build entry point invoked from `transform::builder` after each
/// language's `post_transform` completes. Panics on `Severity::Error`
/// violations with a precise per-violation message; advisories are
/// silent here (the cargo test reports them).
///
/// Release builds make this a no-op via the `cfg(debug_assertions)`
/// gate at the call site, so this body never runs in production
/// transforms.
pub fn assert_shape_contracts(xot: &Xot, root: XotNode, lang: &str) {
    let violations = validate_shape_contracts(xot, root, lang);
    let errors: Vec<&Violation> = violations
        .iter()
        .filter(|v| v.severity == Severity::Error)
        .collect();
    if errors.is_empty() {
        return;
    }
    let mut msg = format!(
        "shape-contract violation in {lang} transform — {} error(s):\n",
        errors.len()
    );
    for v in errors {
        msg.push_str(&format!("  [{}] {}\n", v.rule_id, v.message));
    }
    panic!("{msg}");
}
