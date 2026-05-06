//! Cross-cutting invariant tests that enforce design principles
//! mechanically across every language fixture.
//!
//! Each test walks every fixture's transformed tree and reports
//! violations in advisory mode (prints diagnostics, does NOT fail).
//! Flip the `ASSERT_*` constants to true once the violations are
//! down to zero to turn them into hard gates.
//!
//! Design references:
//! - `specs/tractor-parse/semantic-tree/design.md` — principles &
//!   goals we're enforcing.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tractor::{parse, ParseInput, ParseOptions, TreeMode};
use xot::{Node, Xot};

// Global kill-switch: if false, no individual gate fires even if
// true. Individual per-invariant gates below are the real controls
// once each invariant is genuinely at zero for the languages it
// applies to.
const ASSERT_INVARIANTS: bool = true;

// Per-invariant gates. Flip to `true` once a given invariant is
// at zero across all non-data fixtures.
const ASSERT_NO_UNDERSCORE: bool = true;

const MAX_SHOWN_PER_KIND: usize = 10;

const DATA_LANG_EXTS: &[&str] = &["json", "yaml", "yml", "toml", "ini", "env"];

/// Map a file extension to the canonical language id used by the
/// per-language registry in `tractor::languages`.
fn lang_from_ext(ext: &str) -> Option<&'static str> {
    match ext {
        "cs" => Some("csharp"),
        "ts" | "tsx" | "js" | "jsx" => Some("typescript"),
        "py" => Some("python"),
        "rs" => Some("rust"),
        "go" => Some("go"),
        "java" => Some("java"),
        "php" => Some("php"),
        "rb" => Some("ruby"),
        "sql" => Some("tsql"),
        _ => None,
    }
}

/// Node names allowed to contain an underscore. Everything else
/// with an underscore is almost certainly a tree-sitter grammar
/// kind leaking through (e.g. `if_statement`, `variable_declarator`,
/// `parenthesized_expression`).
const ALLOWED_UNDERSCORE_NAMES: &[&str] = &[
    "else_if",
    // `<non_null/>` marker for TypeScript's `foo!` non-null assertion.
    // Underscore separates the multi-word concept; not a grammar leak.
    "non_null",
];


// ---------------------------------------------------------------------------
// Fixture discovery (shared with text_preservation; kept local for
// cargo to pick up without a shared mod).
// ---------------------------------------------------------------------------

fn languages_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("tests/integration/languages")
}

fn iter_fixtures() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack: Vec<PathBuf> = vec![languages_dir()];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            if name.contains(".snapshot.") {
                continue;
            }
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if matches!(ext, "xml" | "md" | "sh" | "json") {
                continue;
            }
            out.push(path);
        }
    }
    out.sort();
    out
}

fn parse_structure(path: &Path) -> Option<tractor::XeeParseResult> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let mode = if DATA_LANG_EXTS.contains(&ext) {
        Some(TreeMode::Structure)
    } else {
        None
    };
    parse(
        ParseInput::Disk { path },
        ParseOptions {
            tree_mode: mode,
            ..ParseOptions::default()
        },
    )
    .ok()
}

// ---------------------------------------------------------------------------
// Tree walk helper
// ---------------------------------------------------------------------------

fn walk_elements<F: FnMut(&Xot, Node)>(xot: &Xot, node: Node, f: &mut F) {
    if xot.element(node).is_some() {
        f(xot, node);
    }
    for child in xot.children(node) {
        walk_elements(xot, child, f);
    }
}

fn element_name(xot: &Xot, node: Node) -> Option<String> {
    let name_id = xot.element(node)?.name();
    Some(xot.name_ns_str(name_id).0.to_string())
}

// ---------------------------------------------------------------------------
// Report helper — group violations by element name so fixtures with
// the same problem collapse.
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Report {
    /// Element name → list of (fixture, extra context).
    by_name: BTreeMap<String, Vec<(PathBuf, String)>>,
}

impl Report {
    fn record(&mut self, name: &str, fixture: &Path, context: String) {
        self.by_name
            .entry(name.to_string())
            .or_default()
            .push((fixture.to_path_buf(), context));
    }

    fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }

    fn total(&self) -> usize {
        self.by_name.values().map(|v| v.len()).sum()
    }

    fn print(&self, headline: &str) {
        eprintln!();
        eprintln!("⚠ {} — {} violations across {} element name(s)",
                  headline, self.total(), self.by_name.len());
        eprintln!();
        for (name, hits) in &self.by_name {
            eprintln!("  <{}> — {} occurrence(s)", name, hits.len());
            for (fixture, ctx) in hits.iter().take(MAX_SHOWN_PER_KIND) {
                eprintln!(
                    "    {}  {}",
                    fixture
                        .strip_prefix(languages_dir())
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|_| fixture.display().to_string()),
                    ctx
                );
            }
            if hits.len() > MAX_SHOWN_PER_KIND {
                eprintln!("    … and {} more", hits.len() - MAX_SHOWN_PER_KIND);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Invariant 1 (RETIRED iter 314): all_node_names_are_lowercase
// migrated to the spec-conformance walker. The shape-contract rule
// `node-name-lowercase` in `tractor/src/transform/shape_contracts.rs`
// runs both via the cargo test (against blueprint fixtures) AND via
// the debug-build assertion in `transform/builder.rs` (every transform
// invocation). Strictly more coverage than the previous blueprint-only
// walk.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Invariant 2: No underscores in node names (except whitelisted).
//
// A transformed element name containing an underscore is nearly
// always a tree-sitter grammar kind that never got renamed — e.g.
// `if_statement`, `parenthesized_expression`, `mutable_specifier`,
// `variable_declarator`, `string_content`. These violate multiple
// principles at once (grammar vocabulary leaking, cross-language
// inconsistency).
// ---------------------------------------------------------------------------

#[test]
fn no_underscore_in_node_names_except_whitelist() {
    let mut report = Report::default();
    for fixture in iter_fixtures() {
        let ext = fixture.extension().and_then(|e| e.to_str()).unwrap_or("");
        if DATA_LANG_EXTS.contains(&ext) {
            continue;
        }
        let Some(parsed) = parse_structure(&fixture) else { continue };
        let xot = parsed.documents.xot();
        let root = parsed.documents.document_node(parsed.doc_handle).unwrap();
        walk_elements(xot, root, &mut |xot, node| {
            let Some(name) = element_name(xot, node) else { return };
            if !name.contains('_') {
                return;
            }
            if ALLOWED_UNDERSCORE_NAMES.contains(&name.as_str()) {
                return;
            }
            report.record(&name, &fixture, String::new());
        });
    }

    // Static-side complement: each language's rule table is the source
    // of truth for which grammar kinds survive the transform with their
    // raw name (`Rule::Passthrough`). Walk every language's rules and
    // panic on any passthrough kind whose snake_case form contains an
    // underscore not on the allowlist — even if no fixture currently
    // exercises that kind. Catches drift before it reaches output.
    let mut rule_violations: Vec<(&'static str, &'static str)> = Vec::new();
    for (lang, kinds) in passthrough_kinds_per_language() {
        for kind in kinds {
            if !kind.contains('_') {
                continue;
            }
            if ALLOWED_UNDERSCORE_NAMES.contains(&kind) {
                continue;
            }
            rule_violations.push((lang, kind));
        }
    }
    if !rule_violations.is_empty() {
        eprintln!();
        eprintln!(
            "⚠ Principle #1/#2 — passthrough rule produces underscored kind names ({} violation(s))",
            rule_violations.len()
        );
        eprintln!();
        eprintln!("  These kinds map to `Rule::Passthrough` in the language's rules.rs,");
        eprintln!("  so their raw grammar string surfaces as the element name. Either");
        eprintln!("  rename them via `Rule::Rename` / `RenameWithMarker` / `Custom`, or");
        eprintln!("  add the resulting name to ALLOWED_UNDERSCORE_NAMES if it's an");
        eprintln!("  intentional multi-word concept (rare).");
        eprintln!();
        for (lang, kind) in &rule_violations {
            eprintln!("  {:<12} {}", lang, kind);
        }
    }

    let report_violation = !report.is_empty();
    let rule_violation = !rule_violations.is_empty();
    if report_violation {
        report.print("Principle #1/#2 — no tree-sitter kind leaks (underscored names)");
    }
    if (report_violation || rule_violation) && ASSERT_INVARIANTS && ASSERT_NO_UNDERSCORE {
        panic!("underscored node names");
    }
}

// ---------------------------------------------------------------------------
// Invariant (RETIRED iter 314): no_dash_in_node_names migrated to the
// spec-conformance walker. The shape-contract rule
// `no-dash-in-node-name` in `tractor/src/transform/shape_contracts.rs`
// runs both via the cargo test (against blueprint fixtures) AND via
// the debug-build assertion in `transform/builder.rs` (every transform
// invocation). Strictly more coverage than the previous blueprint-only
// walk.

/// `(lang_id, kind_strs)` for every language whose rule table is
/// rule-driven. The kind strings are the snake_case `IntoStaticStr`
/// outputs of the language's `Kind` enum, filtered to those that map
/// to `Rule::Passthrough`. JSON / YAML have two rule branches (syntax
/// + data); we union them so a kind passing through *either* branch
/// counts.
fn passthrough_kinds_per_language() -> Vec<(&'static str, Vec<&'static str>)> {
    use tractor::languages::rule::passthrough_kinds;
    use tractor::languages::*;

    fn dedupe(mut v: Vec<&'static str>) -> Vec<&'static str> {
        v.sort();
        v.dedup();
        v
    }

    vec![
        ("typescript", passthrough_kinds(typescript::rules::rule)),
        // C# moved off the imperative pipeline: rule table no longer
        // exists. The IR pipeline owns its own dispatch table; there's
        // no equivalent passthrough catalogue (and the
        // `Ir::Unknown` fall-through covers the same diagnostic).
        // Python moved off the imperative pipeline: rule table no
        // longer exists. The IR pipeline owns its dispatch; the
        // `Ir::Unknown` fall-through covers the same diagnostic.
        // Go moved off the imperative pipeline: rule table no longer
        // exists. `Ir::Unknown` covers the diagnostic.
        ("rust",       passthrough_kinds(rust_lang::rules::rule)),
        // Java moved off the imperative pipeline: rule table no
        // longer exists. `Ir::Unknown` covers the diagnostic.
        ("ruby",       passthrough_kinds(ruby::rules::rule)),
        // PHP moved off the imperative pipeline: rule table no longer
        // exists. `Ir::Unknown` covers the diagnostic.
        ("tsql",       passthrough_kinds(tsql::rules::rule)),
        ("toml",       passthrough_kinds(toml::rules::rule)),
        ("ini",        passthrough_kinds(ini::rules::rule)),
        ("env",        passthrough_kinds(env::rules::rule)),
        ("markdown",   passthrough_kinds(markdown::rules::rule)),
        ("json", dedupe({
            let mut v = passthrough_kinds(json::rules::syntax_rule);
            v.extend(passthrough_kinds(json::rules::data_rule));
            v
        })),
        ("yaml", dedupe({
            let mut v = passthrough_kinds(yaml::rules::syntax_rule);
            v.extend(passthrough_kinds(yaml::rules::data_rule));
            v
        })),
    ]
}

// ---------------------------------------------------------------------------
// Invariant 3 (RETIRED iter 317): no_grammar_kind_suffixes migrated to
// the spec-conformance walker. The shape-contract rule
// `no-grammar-kind-suffix` in `tractor/src/transform/shape_contracts.rs`
// (with its own GRAMMAR_SUFFIXES list) runs both via the cargo test
// (against blueprint fixtures) AND via the debug-build assertion in
// `transform/builder.rs` (every transform invocation). Strictly more
// coverage than the previous blueprint-only walk.
//
// Migration succeeded after iter 316 detached PHP empty modifier
// nodes and iter 344 renamed markdown's `<code_block>` to
// `<codeblock>` (the two iter-315 findings). The
// GRAMMAR_SUFFIX_EXEMPT list is now empty.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Invariant 4 (RETIRED iter 299): name_element_is_text_leaf migrated
// to the spec-conformance walker. The shape-contract rule
// `name-is-text-leaf` in `tractor/src/transform/shape_contracts.rs`
// runs both via the cargo test (against blueprint fixtures) AND via
// the debug-build assertion in `transform/builder.rs` (every
// transform invocation). Strictly more coverage than the previous
// blueprint-only walk. Six clean iters (292-298) prove parity.
// Phase 2 will add a `TextLeaf` `NodeRole` variant + per-language
// declaration so the rule applies generically rather than hardcoding
// "name"; for now the rule hardcodes "name" as the only TextLeaf.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Invariant 5 (RETIRED iter 297): markers_stay_empty migrated to the
// spec-conformance walker. The shape-contract rule
// `marker-stays-empty` in `tractor/src/transform/shape_contracts.rs`
// is driven by `NodeRole::MarkerOnly` and runs both via the cargo
// test (`tractor/tests/shape_contracts.rs`, against blueprint
// fixtures) AND via the debug-build assertion in
// `transform/builder.rs` (every transform invocation, including
// per-test source). Strictly more coverage than the previous
// blueprint-only walk; iters 294/295/296 demonstrate the broader
// coverage caught real bugs the hand-coded version missed for
// years (Rust Crate/Super, C# Struct, Java Super dual-use
// misclassifications).
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Invariant 6 (RETIRED iter 313): op_marker_matches_text migrated to
// the spec-conformance walker. The shape-contract rule
// `op-marker-matches-text` in `tractor/src/transform/shape_contracts.rs`
// runs both via the cargo test (against blueprint fixtures) AND via
// the debug-build assertion in `transform/builder.rs` (every transform
// invocation). Strictly more coverage than the previous blueprint-only
// walk.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Invariant 7 (RETIRED iter 322): all_names_declared_in_semantic_module
// migrated to the spec-conformance walker. The shape-contract rule
// `name-declared-in-semantic-module` in
// `tractor/src/transform/shape_contracts.rs` runs both via the cargo
// test (against blueprint fixtures) AND via the debug-build assertion
// in `transform/builder.rs` (every transform invocation). Strictly
// more coverage than the previous blueprint-only walk.
//
// Migration succeeded after iter 321 declared `Subscript` in Rust's
// TractorNode enum (the chain inverter emits it programmatically;
// the enum hadn't catalogued it).
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Invariant 8 (RETIRED iter 312): kind_attribute_is_non_empty migrated
// to the spec-conformance walker. The shape-contract rule
// `kind-attribute-non-empty` in `tractor/src/transform/shape_contracts.rs`
// runs both via the cargo test (against blueprint fixtures) AND via
// the debug-build assertion in `transform/builder.rs` (every transform
// invocation). Strictly more coverage than the previous blueprint-only
// walk.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Invariant 9 (RETIRED iter 318): no_repeated_parent_child_name migrated
// to the spec-conformance walker. The shape-contract rule
// `no-repeated-parent-child-name` in
// `tractor/src/transform/shape_contracts.rs` (with its own
// REPEATED_NAME_WHITELIST including markdown's `<section>` and TSX's
// `<element>` — discovered via layer-2 coverage) runs both via the
// cargo test (against blueprint fixtures) AND via the debug-build
// assertion in `transform/builder.rs` (every transform invocation).
// Strictly more coverage than the previous blueprint-only walk.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Invariant 10 (RETIRED iter 297): containers_have_content_or_are_absent
// migrated to the spec-conformance walker. The shape-contract rule
// `container-has-content` in `tractor/src/transform/shape_contracts.rs`
// is driven by `NodeRole::ContainerOnly` and runs both via the cargo
// test (against blueprint fixtures) AND via the debug-build assertion
// in `transform/builder.rs` (every transform invocation). Strictly
// more coverage than the previous blueprint-only walk.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Invariant 11 (RETIRED iter 320): no_anonymous_keyword_leaks migrated
// to the spec-conformance walker. The shape-contract rule
// `no-anonymous-keyword-leak` in
// `tractor/src/transform/shape_contracts.rs` (with its own
// KEYWORD_LEAK_FREE_FORM_PARENTS skip list) runs both via the cargo
// test (against blueprint fixtures) AND via the debug-build assertion
// in `transform/builder.rs` (every transform invocation). Strictly
// more coverage than the previous blueprint-only walk.
//
// Migration succeeded after iter 319 fixed TS
// `extract_function_markers` to also extract static/readonly/override/
// abstract (the bug surfaced by the first migration attempt).
// ---------------------------------------------------------------------------
