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
    /// Grandfather ratchet: when `Some(n)`, layer 1 (cargo test) fails
    /// if the violation count exceeds `n`. The current population
    /// stays grandfathered while the ratchet prevents *new*
    /// regressions from sneaking in. Decrement as the population
    /// shrinks; promote rule to `Severity::Error` once at zero.
    /// `None` for rules that don't ratchet (Error rules are pinned
    /// at zero by definition; Advisory rules without a ratchet just
    /// report).
    pub grandfathered_max: Option<usize>,
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

/// Rule `container-has-content` — phase 2 migration of the
/// `containers_have_content_or_are_absent` invariant in
/// `tree_invariants.rs` (asserted at zero since iter 49). Driven by
/// `NodeRole::ContainerOnly` rather than the bespoke
/// `(spec.container && !spec.marker)` check.
///
/// A name declared `ContainerOnly` must, when emitted, have at least
/// one child (text or element). An empty `<X/>` declared as
/// `ContainerOnly` indicates the transform detached the inner content
/// but left the wrapper behind.
///
/// Operator markers (from the shared `OPERATOR_MARKERS` table) and
/// field wrappers (per-language builder-inserted slot wrappers) are
/// exempt — they're not in the language's `TractorNodeSpec` table and
/// have their own invariants. `Unspecified` (neither marker nor
/// container declared) is also exempt; tighten via explicit
/// declaration in phase 2.
///
/// Severity: `Error`. Pinned at 0.
fn check_container_has_content(
    xot: &Xot,
    root: XotNode,
    lang: &str,
    out: &mut Vec<Violation>,
) {
    use crate::transform::operators::is_operator_marker_name;
    let lookup = crate::languages::get_language(lang).and_then(|l| l.node_spec);
    let Some(lookup) = lookup else { return };
    walk_elements(xot, root, &mut |xot, node| {
        let Some(name) = get_element_name(xot, node) else { return };
        // Skip operator markers and field wrappers — outside the
        // per-language NodeSpec table; covered by other invariants.
        if is_operator_marker_name(&name)
            || crate::languages::is_field_wrapper_name(lang, &name)
        {
            return;
        }
        let Some(spec) = lookup(&name) else { return };
        if spec.role() != crate::languages::NodeRole::ContainerOnly {
            return;
        }
        // ContainerOnly must have content (text or element children).
        let has_element_child = xot.children(node).any(|c| xot.element(c).is_some());
        let has_real_text = xot
            .children(node)
            .any(|c| xot.text_str(c).map_or(false, |t| !t.trim().is_empty()));
        if has_element_child || has_real_text {
            return;
        }
        let line = get_attr(xot, node, "line").unwrap_or_default();
        out.push(Violation {
            rule_id: "container-has-content",
            message: format!(
                "<{name}/> (line {line}) is ContainerOnly but empty — content expected"
            ),
            severity: Severity::Error,
        });
    });
}

/// Rule `name-is-text-leaf` — `<name>` is reserved for identifier text
/// leaves. Element children inside `<name>` indicate a wrapper that
/// didn't get inlined (e.g. `<name><type>Foo</type></name>`).
///
/// Severity: `Error`. Replaces the retired `name_element_is_text_leaf`
/// invariant from `tree_invariants.rs` (retired iter 299). Currently
/// hardcodes `name` as the only TextLeaf; phase 2 will generalize via
/// a `NodeRole::TextLeaf` variant + per-language declaration.
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

/// Rule `kind-attribute-non-empty` — every node the builder emits
/// from a tree-sitter parse carries a `kind` attribute recording the
/// original grammar kind. Per-language transforms dispatch on the
/// kind (via `get_kind`) in preference to the element name so that
/// walk-order doesn't matter — a parent handler inspecting a child's
/// role should see the same kind it had at parse time, regardless of
/// whether the child's element name has already been renamed.
///
/// Invariant: if a node has a `kind` attribute at all, its value is
/// non-empty. A transform that accidentally cleared the attribute
/// (e.g. by re-creating the element from scratch rather than renaming
/// it) would surface here.
///
/// Builder-synthesised wrappers (field wrappers, comment wrappers,
/// op markers, etc.) legitimately have NO `kind` attribute — only
/// nodes WITH the attribute are checked.
///
/// Severity: `Error`. Replaces the retired
/// `kind_attribute_is_non_empty` invariant from `tree_invariants.rs`
/// (retired iter 312).
fn check_kind_attribute_non_empty(
    xot: &Xot,
    root: XotNode,
    _lang: &str,
    out: &mut Vec<Violation>,
) {
    walk_elements(xot, root, &mut |xot, node| {
        let Some(name) = get_element_name(xot, node) else { return };
        let Some(kind) = get_attr(xot, node, "kind") else { return };
        if !kind.is_empty() {
            return;
        }
        let line = get_attr(xot, node, "line").unwrap_or_default();
        out.push(Violation {
            rule_id: "kind-attribute-non-empty",
            message: format!(
                "<{name}> (line {line}) has empty kind=\"\" — tree-sitter origin attribute was wiped"
            ),
            severity: Severity::Error,
        });
    });
}

/// Rule `op-marker-matches-text` — every `<op>` element whose trimmed
/// text value matches an entry in the canonical cross-language
/// `OPERATOR_MARKERS` table must carry that entry's `primary` marker
/// (when one is declared) as a direct element child.
///
/// Catches:
///   - A language that extracts operators but forgets to call the
///     shared `prepend_op_element` helper.
///   - A transform that attaches the wrong marker for a canonical op.
///
/// Graceful: unknown operator text (language-specific operators) is
/// accepted without requirements; canonical entries with no `primary`
/// marker (e.g. bare `=`) are exempt.
///
/// Severity: `Error`. Replaces the retired `op_marker_matches_text`
/// invariant from `tree_invariants.rs` (retired iter 313).
fn check_op_marker_matches_text(
    xot: &Xot,
    root: XotNode,
    _lang: &str,
    out: &mut Vec<Violation>,
) {
    use crate::transform::operators::lookup_operator_spec;
    walk_elements(xot, root, &mut |xot, node| {
        let Some(name) = get_element_name(xot, node) else { return };
        if name != "op" {
            return;
        }
        let text: String = xot
            .children(node)
            .filter_map(|c| xot.text_str(c).map(|s| s.to_string()))
            .collect::<Vec<_>>()
            .join("");
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return;
        }
        let Some(spec) = lookup_operator_spec(trimmed) else { return };
        let Some(primary) = spec.primary else { return };
        let has_primary = xot
            .children(node)
            .any(|c| get_element_name(xot, c).as_deref() == Some(primary));
        if has_primary {
            return;
        }
        let line = get_attr(xot, node, "line").unwrap_or_default();
        out.push(Violation {
            rule_id: "op-marker-matches-text",
            message: format!(
                "<op> (line {line}) text {trimmed:?} missing <{primary}/> primary marker"
            ),
            severity: Severity::Error,
        });
    });
}

/// Rule `node-name-lowercase` — Principle #3 ("Always Lowercase").
/// Also supports the "identifiers are never element names" decision:
/// user-defined identifiers almost always have uppercase chars, so an
/// element name with an uppercase char is a symptom of an identifier
/// accidentally being promoted.
///
/// Severity: `Error`. Replaces the retired `all_node_names_are_lowercase`
/// invariant from `tree_invariants.rs` (retired iter 314).
fn check_node_name_lowercase(
    xot: &Xot,
    root: XotNode,
    _lang: &str,
    out: &mut Vec<Violation>,
) {
    walk_elements(xot, root, &mut |xot, node| {
        let Some(name) = get_element_name(xot, node) else { return };
        if !name.chars().any(|c| c.is_ascii_uppercase()) {
            return;
        }
        let line = get_attr(xot, node, "line").unwrap_or_default();
        out.push(Violation {
            rule_id: "node-name-lowercase",
            message: format!(
                "<{name}> (line {line}) has uppercase chars — Principle #3 violation"
            ),
            severity: Severity::Error,
        });
    });
}

/// Rule `no-dash-in-node-name` — project convention is single-word
/// element names, falling back to snake_case for multi-word concepts.
/// Dashes invite ambiguity with arithmetic-minus when authoring
/// queries, and they don't match the strum `serialize_all =
/// "snake_case"` default that every output enum declares.
///
/// Severity: `Error`. Replaces the retired `no_dash_in_node_names`
/// invariant from `tree_invariants.rs` (retired iter 314).
fn check_no_dash_in_node_name(
    xot: &Xot,
    root: XotNode,
    _lang: &str,
    out: &mut Vec<Violation>,
) {
    walk_elements(xot, root, &mut |xot, node| {
        let Some(name) = get_element_name(xot, node) else { return };
        if !name.contains('-') {
            return;
        }
        let line = get_attr(xot, node, "line").unwrap_or_default();
        out.push(Violation {
            rule_id: "no-dash-in-node-name",
            message: format!(
                "<{name}> (line {line}) contains a dash — use snake_case for multi-word names"
            ),
            severity: Severity::Error,
        });
    });
}

/// Element names exempt from the `no-grammar-kind-suffix` rule.
/// Each entry should have a justification.
const GRAMMAR_SUFFIX_EXEMPT: &[&str] = &[
    // Markdown's `<code_block>` is the canonical name for fenced /
    // indented code blocks. The trailing `_block` looks like a
    // grammar suffix, but in markdown's vocabulary "code block" IS
    // the user-facing concept. Renaming would break the existing
    // CLI test `markdown_round_trip_extracts_javascript_block`
    // (queries `//code_block`) and lose user-facing clarity. Iter
    // 315 surfaced this; iter 317 ships the rule with this exemption
    // after PHP empty-modifier nodes (the other iter-315 finding)
    // got detached in iter 316.
    "code_block",
];

/// Grammar-kind suffixes that indicate tree-sitter bleed-through.
const GRAMMAR_SUFFIXES: &[&str] = &[
    "_statement",
    "_declaration",
    "_expression",
    "_clause",
    "_specifier",
    "_list",
    "_literal",
    "_modifier",
    "_identifier",
    "_block",
    "_body",
    "_parameter",
    "_parameters",
    "_argument",
    "_arguments",
    "_type",
    "_definition",
];

/// Rule `no-grammar-kind-suffix` — no element name should end in a
/// known tree-sitter grammar suffix (e.g. `if_statement`,
/// `conditional_expression`). Catches kinds that didn't get a
/// semantic rename.
///
/// Severity: `Error`. Replaces the retired `no_grammar_kind_suffixes`
/// invariant from `tree_invariants.rs` (retired iter 317). Markdown's
/// `<code_block>` is exempt per the GRAMMAR_SUFFIX_EXEMPT list.
fn check_no_grammar_kind_suffix(
    xot: &Xot,
    root: XotNode,
    _lang: &str,
    out: &mut Vec<Violation>,
) {
    walk_elements(xot, root, &mut |xot, node| {
        let Some(name) = get_element_name(xot, node) else { return };
        if GRAMMAR_SUFFIX_EXEMPT.contains(&name.as_str()) {
            return;
        }
        for suffix in GRAMMAR_SUFFIXES {
            if name.ends_with(suffix) {
                let line = get_attr(xot, node, "line").unwrap_or_default();
                out.push(Violation {
                    rule_id: "no-grammar-kind-suffix",
                    message: format!(
                        "<{name}> (line {line}) ends in `{suffix}` — likely an unrenamed tree-sitter kind"
                    ),
                    severity: Severity::Error,
                });
                return;
            }
        }
    });
}

/// Element names where parent/child nesting of the same name is
/// genuinely meaningful (recursive structures, not grammar-leak
/// double-wraps). Each entry has a defensible reason; keep the list
/// short.
const REPEATED_NAME_WHITELIST: &[&str] = &[
    // Nested module / namespace paths: `java.util.function` is a
    // path-of-paths in tree-sitter's grammar.
    "path",
    // Pattern combinators (Ruby `case in`, Python `match`) compose:
    // `pattern[alternative]/pattern[type]/...` is genuinely nested.
    "pattern",
    // Member-access chains `a.b.c`.
    "member",
    // Nested type expressions: `List<Map<String, T>>`.
    "type",
    // Composed function calls `f(g(x))`; method-chain shapes nest.
    "call",
    // Chained comparisons (`a < b < c`).
    "compare",
    // Binary expressions: `a + b * c`.
    "binary",
    // Ternaries can nest.
    "ternary",
    // Nested collection literals `[[1,2],[3,4]]`.
    "list",
    "dict",
    "tuple",
    // String concatenation / nested f-strings.
    "string",
    // Variable declarators with multiple bindings; some nesting cases.
    "variable",
    // TypeScript chained type assertions `<number>(<unknown>"42")`.
    "as",
    // Grouped imports/uses: `<use[group]>/<use>` / `<import[group]>/<import>`.
    "use",
    "import",
    // Markdown headings nest by level: a `## Section` under `# Doc`
    // produces `<section>/<section>`. Genuinely recursive; surfaced
    // by iter 318's layer-2 coverage on the markdown CLI tests
    // (the blueprint-scoped invariant didn't see markdown).
    "section",
    // TSX/JSX nested elements: `<div><span /></div>` produces
    // `<element>/<element>`. Genuinely recursive.
    "element",
];

/// Rule `no-repeated-parent-child-name` — flags `<X><X>...</X></X>`
/// patterns where parent and immediate child share the same element
/// name. Almost always a sign that two different rules independently
/// produced the same wrapper name and one should Flatten or rename.
///
/// `REPEATED_NAME_WHITELIST` exempts genuinely-recursive structures.
///
/// Severity: `Error`. Replaces the retired `no_repeated_parent_child_name`
/// invariant from `tree_invariants.rs` (retired iter 318).
fn check_no_repeated_parent_child_name(
    xot: &Xot,
    root: XotNode,
    _lang: &str,
    out: &mut Vec<Violation>,
) {
    walk_elements(xot, root, &mut |xot, node| {
        let Some(name) = get_element_name(xot, node) else { return };
        if REPEATED_NAME_WHITELIST.contains(&name.as_str()) {
            return;
        }
        let Some(parent) = xot.parent(node) else { return };
        let Some(parent_name) = get_element_name(xot, parent) else { return };
        if parent_name != name {
            return;
        }
        let line = get_attr(xot, node, "line").unwrap_or_default();
        out.push(Violation {
            rule_id: "no-repeated-parent-child-name",
            message: format!(
                "<{name}> (line {line}) nested directly under <{name}> — likely a Flatten/rename gap"
            ),
            severity: Severity::Error,
        });
    });
    // Note: walk_elements visits root as well; root has no parent
    // element, so the early return on `xot.parent` covers it.
    let _ = root;
}

/// Element names where text content is intentionally free-form and
/// not subject to keyword-leak detection (comment text, identifier
/// text, string content, etc.).
const KEYWORD_LEAK_FREE_FORM_PARENTS: &[&str] = &[
    "comment", "name", "string", "symbol", "regex", "interpolation",
    "template", "char", "escape",
    // `<op>` text is a known operator; not a keyword leak.
    "op",
];

/// Rule `no-anonymous-keyword-leak` — for every text leaf inside an
/// output element whose trimmed content tokenizes into MARKER_ONLY
/// keywords, every keyword token must have a sibling marker element of
/// the same name. Catches Principle #2 / #7 regressions where a
/// keyword token survived as text (good — preserves source) but the
/// corresponding marker conversion was missed (bad — query-unfriendly).
///
/// Whitespace-tokenization with per-token check: if any token isn't a
/// marker-only name, treat the whole text as identifier/expression
/// (not a keyword leak) and skip.
///
/// Severity: `Error`. Replaces the retired `no_anonymous_keyword_leaks`
/// invariant from `tree_invariants.rs` (retired iter 320).
fn check_no_anonymous_keyword_leak(
    xot: &Xot,
    root: XotNode,
    lang: &str,
    out: &mut Vec<Violation>,
) {
    use std::collections::HashSet;
    if !crate::languages::has_semantic_vocabulary(lang) {
        return;
    }
    walk_elements(xot, root, &mut |xot, node| {
        let parent_name = get_element_name(xot, node);
        if let Some(ref name) = parent_name {
            if KEYWORD_LEAK_FREE_FORM_PARENTS.contains(&name.as_str()) {
                return;
            }
        }
        let element_children: HashSet<String> = xot
            .children(node)
            .filter_map(|c| get_element_name(xot, c))
            .collect();
        for child in xot.children(node) {
            let Some(text) = xot.text_str(child) else { continue };
            let trimmed = text.trim().trim_end_matches(';').trim();
            if trimmed.is_empty() {
                continue;
            }
            let tokens: Vec<&str> = trimmed.split_whitespace().collect();
            if tokens.is_empty() {
                continue;
            }
            let all_keywords = tokens.iter().all(|t| {
                crate::languages::is_marker_only_name(lang, t)
                    || parent_name.as_deref() == Some(*t)
            });
            if !all_keywords {
                continue;
            }
            for token in tokens {
                if parent_name.as_deref() == Some(token) {
                    continue;
                }
                if !crate::languages::is_marker_only_name(lang, token) {
                    continue;
                }
                if element_children.contains(token) {
                    continue;
                }
                let parent_name_str = parent_name.clone().unwrap_or_default();
                let line = get_attr(xot, node, "line").unwrap_or_default();
                out.push(Violation {
                    rule_id: "no-anonymous-keyword-leak",
                    message: format!(
                        "keyword {token:?} in <{parent_name_str}> (line {line}) text without sibling <{token}/> marker"
                    ),
                    severity: Severity::Error,
                });
            }
        }
    });
}

/// Rule `name-declared-in-semantic-module` — every emitted element
/// name must be either:
/// - declared in the language's `TractorNodeSpec` table, OR
/// - a cross-cutting operator marker (`OPERATOR_MARKERS`), OR
/// - a field wrapper (per-language `*_FIELD_WRAPPINGS` table).
///
/// Catches raw tree-sitter kinds leaking through, or
/// chain-inverter-emitted names a language forgot to declare.
///
/// Severity: `Error`. Replaces the retired
/// `all_names_declared_in_semantic_module` invariant from
/// `tree_invariants.rs` (retired iter 322).
fn check_name_declared_in_semantic_module(
    xot: &Xot,
    root: XotNode,
    lang: &str,
    out: &mut Vec<Violation>,
) {
    use crate::languages::{has_semantic_vocabulary, is_declared_name, is_field_wrapper_name};
    use crate::transform::operators::is_operator_marker_name;
    if !has_semantic_vocabulary(lang) {
        return;
    }
    walk_elements(xot, root, &mut |xot, node| {
        let Some(name) = get_element_name(xot, node) else { return };
        if is_declared_name(lang, &name) {
            return;
        }
        if is_operator_marker_name(&name) {
            return;
        }
        if is_field_wrapper_name(lang, &name) {
            return;
        }
        let line = get_attr(xot, node, "line").unwrap_or_default();
        out.push(Violation {
            rule_id: "name-declared-in-semantic-module",
            message: format!(
                "<{name}> (line {line}) is not declared in {lang}'s TractorNodeSpec table (or operator/field-wrapper allowlist)"
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
        // 13 grandfathered sites as of iter 298: 9 in main languages
        // (csharp 1, java 1, python 2, ts 1, rust 3, go 1) all in
        // deferred design-call classes per iter 291's natural-pause
        // note, plus 4 in tsql (3 in MERGE, 1 in WHEN INSERT). Any
        // iter that introduces a new overflow site fails CI.
        // Decrement when an overflow is closed; promote to
        // `Severity::Error` (and drop the ratchet) when the count
        // reaches zero.
        grandfathered_max: Some(13),
    },
    ShapeRule {
        id: "no-marker-wrapper-collision",
        description: "Parent has both empty <X/> marker and <X>…</X> wrapper sibling — JSON key collision (iter 184 archetype).",
        severity: Severity::Error,
        check: check_no_marker_wrapper_collision,
        grandfathered_max: None,
    },
    ShapeRule {
        id: "marker-stays-empty",
        description: "A NodeRole::MarkerOnly element must have no text and no element children.",
        severity: Severity::Error,
        check: check_marker_stays_empty,
        grandfathered_max: None,
    },
    ShapeRule {
        id: "container-has-content",
        description: "A NodeRole::ContainerOnly element must have at least one child (text or element).",
        severity: Severity::Error,
        check: check_container_has_content,
        grandfathered_max: None,
    },
    ShapeRule {
        id: "name-is-text-leaf",
        description: "<name> is reserved for identifier text leaves; element children indicate an un-inlined wrapper.",
        severity: Severity::Error,
        check: check_name_is_text_leaf,
        grandfathered_max: None,
    },
    ShapeRule {
        id: "kind-attribute-non-empty",
        description: "Tree-sitter `kind` attributes must be non-empty when present (a transform that wiped the attribute would break per-kind dispatch).",
        severity: Severity::Error,
        check: check_kind_attribute_non_empty,
        grandfathered_max: None,
    },
    ShapeRule {
        id: "op-marker-matches-text",
        description: "Every <op> with canonical text from OPERATOR_MARKERS must carry its declared primary marker (catches language transforms that bypass `prepend_op_element`).",
        severity: Severity::Error,
        check: check_op_marker_matches_text,
        grandfathered_max: None,
    },
    ShapeRule {
        id: "node-name-lowercase",
        description: "Element names must be lowercase (Principle #3). Uppercase chars are a symptom of identifier promotion.",
        severity: Severity::Error,
        check: check_node_name_lowercase,
        grandfathered_max: None,
    },
    ShapeRule {
        id: "no-dash-in-node-name",
        description: "Element names must not contain dashes (project convention; use snake_case).",
        severity: Severity::Error,
        check: check_no_dash_in_node_name,
        grandfathered_max: None,
    },
    ShapeRule {
        id: "no-grammar-kind-suffix",
        description: "Element names must not end in tree-sitter grammar suffixes (e.g. _statement, _expression). Markdown <code_block> exempt.",
        severity: Severity::Error,
        check: check_no_grammar_kind_suffix,
        grandfathered_max: None,
    },
    ShapeRule {
        id: "no-repeated-parent-child-name",
        description: "<X><X>...</X></X> nesting is almost always a Flatten/rename gap. Recursive structures (path/pattern/member/type/call/etc.) are whitelisted.",
        severity: Severity::Error,
        check: check_no_repeated_parent_child_name,
        grandfathered_max: None,
    },
    ShapeRule {
        id: "no-anonymous-keyword-leak",
        description: "Marker-only keyword in element text content must have a sibling <keyword/> marker (Principle #2/#7).",
        severity: Severity::Error,
        check: check_no_anonymous_keyword_leak,
        grandfathered_max: None,
    },
    ShapeRule {
        id: "name-declared-in-semantic-module",
        description: "Every emitted element name must be in the language's TractorNodeSpec table (or operator-marker / field-wrapper allowlist).",
        severity: Severity::Error,
        check: check_name_declared_in_semantic_module,
        grandfathered_max: None,
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

/// Look up a rule by id. Used by layer 1 (cargo integration test)
/// to retrieve the grandfather ratchet.
pub fn lookup_rule(id: &str) -> Option<&'static ShapeRule> {
    RULES.iter().find(|r| r.id == id)
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
