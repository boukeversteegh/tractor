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
const ASSERT_LOWERCASE: bool = true;
const ASSERT_NO_UNDERSCORE: bool = true;
const ASSERT_NO_GRAMMAR_SUFFIXES: bool = true;
const ASSERT_NAME_IS_TEXT_LEAF: bool = true;
// `markers_stay_empty` now checks the per-language MARKER_ONLY
// declaration in each language's `semantic` module — any name listed
// there must be empty (no text, no element children) everywhere it
// appears. Precise, no heuristic.
const ASSERT_MARKERS_STAY_EMPTY: bool = true;
// `op_marker_matches_text` checks that every `<op>` whose text is in
// the canonical OPERATOR_MARKERS table carries the declared primary
// marker. Unknown operators are accepted without requirements.
const ASSERT_OP_MARKER: bool = true;
// `kind_attribute_is_non_empty` asserts that every tree-sitter-originated
// element (one with a `kind` attribute) has a non-empty kind value. A
// transform that accidentally wiped the attribute would surface here.
const ASSERT_KIND_NON_EMPTY: bool = true;
// `no_repeated_parent_child_name` flags `<X><X>…</X></X>` patterns where
// a parent and immediate child share the same element name (e.g.
// `<body><body>` from iter 30/35, `<constraint><constraint>` from iter
// 34, `<arg><arg>` in TSQL). Allowed cases are listed in
// `REPEATED_NAME_WHITELIST`. Now asserted (zero violations as of
// iter 45). See docs/self-improvement.md "Cross-cutting invariant
// tests".
const ASSERT_NO_REPEATED_NAME: bool = true;

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

/// Grammar-kind suffixes that indicate tree-sitter bleed-through.
/// A transformed element name ending in any of these is almost
/// always a tree-sitter node name we never gave a semantic rename.
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
// Invariant 1: Every node name is lowercase.
//
// Principle #3 — "Always Lowercase". Also supports the
// "identifiers are never element names" decision: user-defined
// identifiers almost always have uppercase chars, so an element
// name with an uppercase char is a symptom of an identifier
// accidentally being promoted.
// ---------------------------------------------------------------------------

#[test]
fn all_node_names_are_lowercase() {
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
            if name.chars().any(|c| c.is_ascii_uppercase()) {
                report.record(&name, &fixture, String::new());
            }
        });
    }
    if !report.is_empty() {
        report.print("Principle #3 — node names must be lowercase");
        if ASSERT_INVARIANTS && ASSERT_LOWERCASE {
            panic!("node names with uppercase chars");
        }
    }
}

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
        ("csharp",     passthrough_kinds(csharp::rules::rule)),
        ("python",     passthrough_kinds(python::rules::rule)),
        ("go",         passthrough_kinds(go::rules::rule)),
        ("rust",       passthrough_kinds(rust_lang::rules::rule)),
        ("java",       passthrough_kinds(java::rules::rule)),
        ("ruby",       passthrough_kinds(ruby::rules::rule)),
        ("php",        passthrough_kinds(php::rules::rule)),
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
// Invariant 3: No grammar-kind suffixes.
//
// Complements #2 — even if someone renames `if_statement` → `if`,
// they might miss `conditional_expression` → `ternary`. This flags
// any element whose name ends in a known grammar suffix.
// ---------------------------------------------------------------------------

#[test]
fn no_grammar_kind_suffixes() {
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
            for suffix in GRAMMAR_SUFFIXES {
                if name.ends_with(suffix) {
                    report.record(&name, &fixture, String::new());
                    return;
                }
            }
        });
    }
    if !report.is_empty() {
        report.print("No grammar-kind suffixes in element names");
        if ASSERT_INVARIANTS && ASSERT_NO_GRAMMAR_SUFFIXES {
            panic!("grammar-suffixed node names");
        }
    }
}

// ---------------------------------------------------------------------------
// Invariant 4: `<name>` is a text leaf.
//
// The design doc pins "identifiers are a single `<name>` element"
// as a text leaf. An element named `<name>` should only contain
// text — no element children. Violating it usually means a wrapper
// didn't get inlined (e.g. `<name><type>Foo</type></name>`).
// ---------------------------------------------------------------------------

#[test]
fn name_element_is_text_leaf() {
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
            if name != "name" {
                return;
            }
            // Any element child is a violation.
            let bad_child = xot.children(node).find(|&c| xot.element(c).is_some());
            if let Some(child) = bad_child {
                let child_name = element_name(xot, child).unwrap_or_default();
                report.record(
                    "name",
                    &fixture,
                    format!("has element child <{}>", child_name),
                );
            }
        });
    }
    if !report.is_empty() {
        report.print("<name> must be a text leaf (identifiers are a single <name> element)");
        if ASSERT_INVARIANTS && ASSERT_NAME_IS_TEXT_LEAF {
            panic!("<name> with element children");
        }
    }
}

// ---------------------------------------------------------------------------
// Invariant 5: Markers stay empty.
//
// Principle #7 + the modifiers-as-empty-elements decision: any name
// declared in a language's `semantic::MARKER_ONLY` slice must, when
// emitted, be empty (no text, no element children). The per-language
// declaration is the source of truth — this test is just a mechanical
// assertion.
//
// Names that double as structural containers in some contexts (e.g.
// `struct`, `type`, `function`) are intentionally OMITTED from each
// language's MARKER_ONLY slice, so they're never flagged here.
// ---------------------------------------------------------------------------

#[test]
fn markers_stay_empty() {
    let mut report = Report::default();

    for fixture in iter_fixtures() {
        let ext = fixture.extension().and_then(|e| e.to_str()).unwrap_or("");
        if DATA_LANG_EXTS.contains(&ext) {
            continue;
        }
        let Some(lang) = lang_from_ext(ext) else { continue };
        if !tractor::languages::has_semantic_vocabulary(lang) {
            continue;
        }
        let Some(parsed) = parse_structure(&fixture) else { continue };
        let xot = parsed.documents.xot();
        let root = parsed.documents.document_node(parsed.doc_handle).unwrap();
        walk_elements(xot, root, &mut |xot, node| {
            let Some(name) = element_name(xot, node) else { return };
            if !tractor::languages::is_marker_only_name(lang, &name) {
                return;
            }
            // Marker element must have no element children and no
            // non-whitespace text children.
            let element_child = xot.children(node).find(|&c| xot.element(c).is_some());
            let text_child = xot
                .children(node)
                .find(|&c| xot.text_str(c).map(|s| !s.trim().is_empty()).unwrap_or(false));
            if let Some(child) = element_child {
                let child_name = element_name(xot, child).unwrap_or_default();
                report.record(
                    &name,
                    &fixture,
                    format!("has element child <{}>", child_name),
                );
            } else if let Some(child) = text_child {
                let text = xot
                    .text_str(child)
                    .map(|s| s.trim().to_string())
                    .unwrap_or_default();
                let shown = if text.len() > 40 {
                    format!("{}…", &text[..40])
                } else {
                    text
                };
                report.record(
                    &name,
                    &fixture,
                    format!("has text {:?}", shown),
                );
            }
        });
    }

    if !report.is_empty() {
        report.print(
            "Principle #7 — MARKER_ONLY names must be empty (no text, no element children)",
        );
        if ASSERT_INVARIANTS && ASSERT_MARKERS_STAY_EMPTY {
            panic!("marker elements with content");
        }
    }
}

// ---------------------------------------------------------------------------
// Invariant 6: `<op>` marker matches its text.
//
// The canonical cross-language `OPERATOR_MARKERS` table
// (`tractor::transform::operators::OPERATOR_MARKERS`) is the single
// source of truth for operator → marker mapping. Every language's
// `extract_operator` routes through `prepend_op_element`, which
// consults this table. The invariant:
//
//   For every `<op>` element whose trimmed text value equals some
//   entry in the table, that entry's `primary` marker (if any) MUST
//   be present as a direct element child of the `<op>`.
//
// Unknown operator text = language-specific operator, accepted
// without requirements (graceful degradation).
//
// This test catches:
//   - A language that extracts operators but forgets to call the
//     shared helper (so no marker gets attached).
//   - A transform that attaches the wrong marker for a canonical op.
// ---------------------------------------------------------------------------

#[test]
fn op_marker_matches_text() {
    use tractor::transform::operators::lookup_operator_spec;

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
            if name != "op" {
                return;
            }
            // Collect the text children; concat + trim so ops that sit
            // inside whitespace (e.g. ` == `) collapse to their canonical form.
            let text: String = xot
                .children(node)
                .filter_map(|c| xot.text_str(c).map(|s| s.to_string()))
                .collect::<Vec<_>>()
                .join("");
            let trimmed = text.trim();
            // Skip bare `<op>` with no text — rare edge case where the
            // transform synthesized an op without source text.
            if trimmed.is_empty() {
                return;
            }
            let Some(spec) = lookup_operator_spec(trimmed) else {
                return; // language-specific op, accepted as-is
            };
            let Some(primary) = spec.primary else {
                return; // canonical op that intentionally has no marker (e.g. bare `=`)
            };
            let has_primary = xot.children(node).any(|c| {
                element_name(xot, c).as_deref() == Some(primary)
            });
            if !has_primary {
                report.record(
                    "op",
                    &fixture,
                    format!("text {:?} missing <{}/> primary marker", trimmed, primary),
                );
            }
        });
    }
    if !report.is_empty() {
        report.print("Canonical operators must carry their declared marker");
        if ASSERT_INVARIANTS && ASSERT_OP_MARKER {
            panic!("<op> elements missing canonical marker");
        }
    }
}

// ---------------------------------------------------------------------------
// Invariant 7: Every emitted element name is declared in the language's
// ALL_NAMES.
//
// Each `tractor/src/languages/<lang>.rs` publishes a `semantic` module
// with `ALL_NAMES: &[&str]` — every node name the transform can emit.
// The registry in `tractor::languages::all_semantic_names(lang)` makes
// that set available here.
//
// Invariant: for every fixture, every transformed element name must
// appear in its language's ALL_NAMES set. Unknowns are either:
//   - a raw tree-sitter kind leaking through (transform missed it)
//   - a newly-introduced name that the module author forgot to add
// Either way it's a drift signal worth gating on once at zero.
//
// Kept advisory until the ALL_NAMES sets are complete — the current
// implementation is "advisory by design" (ASSERT_ALL_NAMES_MEMBERSHIP
// is false by default) because several languages still emit names
// that slipped through during the initial catalogue drafting; this
// test surfaces exactly which names are missing from each module.
// ---------------------------------------------------------------------------

const ASSERT_ALL_NAMES_MEMBERSHIP: bool = true;

#[test]
fn all_names_declared_in_semantic_module() {
    use tractor::languages::{has_semantic_vocabulary, is_declared_name, is_field_wrapper_name};
    use tractor::transform::operators::is_operator_marker_name;

    let mut report = Report::default();
    for fixture in iter_fixtures() {
        let ext = fixture.extension().and_then(|e| e.to_str()).unwrap_or("");
        if DATA_LANG_EXTS.contains(&ext) {
            continue;
        }
        let Some(lang) = lang_from_ext(ext) else { continue };
        if !has_semantic_vocabulary(lang) {
            continue;
        }
        let Some(parsed) = parse_structure(&fixture) else { continue };
        let xot = parsed.documents.xot();
        let root = parsed.documents.document_node(parsed.doc_handle).unwrap();
        walk_elements(xot, root, &mut |xot, node| {
            let Some(name) = element_name(xot, node) else { return };
            // (1) Per-language NODES — the main source of truth.
            if is_declared_name(lang, &name) {
                return;
            }
            // (2) Cross-cutting operator markers emitted by the shared
            //     `OPERATOR_MARKERS` table — universally allowed.
            if is_operator_marker_name(&name) {
                return;
            }
            // (3) Field wrappers introduced by the builder's
            //     `apply_field_wrappings` pass — derived per-language
            //     from that language's `*_FIELD_WRAPPINGS` table.
            if is_field_wrapper_name(lang, &name) {
                return;
            }
            report.record(
                &name,
                &fixture,
                format!("<{}> is not in {}::semantic::NODES", name, lang),
            );
        });
    }
    if !report.is_empty() {
        report.print(
            "Every emitted element name must be declared in the language's ALL_NAMES",
        );
        if ASSERT_INVARIANTS && ASSERT_ALL_NAMES_MEMBERSHIP {
            panic!("undeclared element names");
        }
    }
}

// ---------------------------------------------------------------------------
// Invariant 8: Tree-sitter-originated nodes preserve their `kind`.
//
// Every node the builder emits from a tree-sitter parse carries a
// `kind` attribute recording the original grammar kind. Per-language
// transforms dispatch on that kind (via `get_kind`) in preference to
// the element name so that walk-order doesn't matter — a parent
// handler inspecting a child's role should see the same kind it had
// at parse time, regardless of whether the child's element name has
// already been renamed.
//
// This invariant guards the property: if a node has a `kind` attribute
// at all, its value is non-empty. A transform that accidentally cleared
// the attribute (e.g. by re-creating the element from scratch rather
// than renaming it) would show up here.
//
// Builder-synthesised wrappers (field wrappers, comment wrappers, op
// markers, etc.) legitimately have NO `kind` attribute — we only check
// the attribute when it's present, so wrappers don't trigger the gate.
// ---------------------------------------------------------------------------

#[test]
fn kind_attribute_is_non_empty() {
    use tractor::transform::helpers::get_attr;

    let mut report = Report::default();
    for fixture in iter_fixtures() {
        let Some(parsed) = parse_structure(&fixture) else { continue };
        let xot = parsed.documents.xot();
        let root = parsed.documents.document_node(parsed.doc_handle).unwrap();
        walk_elements(xot, root, &mut |xot, node| {
            let Some(name) = element_name(xot, node) else { return };
            let Some(kind) = get_attr(xot, node, "kind") else {
                return; // Builder wrapper, no tree-sitter origin.
            };
            if kind.is_empty() {
                report.record(
                    &name,
                    &fixture,
                    format!("<{}> has empty kind=\"\"", name),
                );
            }
        });
    }
    if !report.is_empty() {
        report.print("Tree-sitter `kind` attributes must be non-empty");
        if ASSERT_INVARIANTS && ASSERT_KIND_NON_EMPTY {
            panic!("empty kind attributes");
        }
    }
}

// ---------------------------------------------------------------------------
// Invariant 9: No repeated parent/child same-name nesting.
//
// Catches `<body><body>`, `<constraint><constraint>`, `<arg><arg>`,
// `<member><member>` etc. — the failure mode of iters 30 / 34 / 35.
// Almost always a sign that two different rules independently
// produced the same wrapper name and one should Flatten or rename.
//
// A small whitelist covers genuinely-recursive language structures
// where the repetition is meaningful (e.g. `<path><path>` for nested
// module paths). If the whitelist grows past ~5 entries the
// invariant probably isn't holding and should be revisited per
// docs/self-improvement.md.
// ---------------------------------------------------------------------------

/// Element names where parent/child nesting of the same name is
/// genuinely meaningful (a recursive structure, not a grammar-leak
/// double-wrap).
///
/// Each entry includes a brief reason. Adding to this list signals
/// "the invariant doesn't apply here" — keep the list short and
/// require every entry to have a defensible reason.
const REPEATED_NAME_WHITELIST: &[&str] = &[
    // Nested module / namespace paths: `java.util.function` is a
    // path-of-paths in tree-sitter's grammar, and the recursion is
    // semantically meaningful.
    "path",
    // Pattern combinators (Ruby `case in`, Python `match`) compose:
    // `pattern[alternative]/pattern[type]/...` is genuinely nested
    // patterns, not a wrapper bug.
    "pattern",
    // Member-access chains `a.b.c` produce nested `<member>`
    // elements where each level represents one access segment;
    // recursion is the intended shape (every other language does
    // this too).
    "member",
    // Nested type expressions: `List<Map<String, T>>` produces
    // `<type[generic]>/<type[generic]>` — nested by design.
    "type",
    // Composed function calls `f(g(x))` produce
    // `<call>/<argument>/<call>` typically, but parent-being-call
    // does happen in Ruby/Rust with method-chain shapes; the
    // recursion is intentional.
    "call",
    // Chained comparisons (`a < b < c` in Python; `a IS NOT NULL AND
    // b > c` in TSQL) emit nested `<compare>` levels.
    "compare",
    // Binary expressions: `a + b * c` parses as nested binaries.
    "binary",
    // Ternaries can nest: `a ? b : (c ? d : e)`.
    "ternary",
    // Nested list literals: `[[1,2],[3,4]]`. Same for dict-of-dicts,
    // tuple-of-tuples — these compose recursively by nature.
    "list",
    "dict",
    "tuple",
    // String concatenation / nested f-strings: `"a" "b"` becomes
    // `<string[concatenated]>/<string>...</string>`; Python f-strings
    // can nest expressions that contain strings.
    "string",
    // Variable declarators in C-family languages can carry multiple
    // bindings under one declaration (`int a = 1, b = 2`). Some
    // languages also nest `<variable>` to scope.
    "variable",
    // TypeScript chained type assertions `<number>(<unknown>"42")`
    // produce nested `<as>` elements; the recursion is intentional.
    "as",
];

#[test]
fn no_repeated_parent_child_name() {
    use std::cell::RefCell;
    let mut report = Report::default();
    for fixture in iter_fixtures() {
        let ext = fixture.extension().and_then(|e| e.to_str()).unwrap_or("");
        if DATA_LANG_EXTS.contains(&ext) {
            continue;
        }
        let Some(parsed) = parse_structure(&fixture) else { continue };
        let xot = parsed.documents.xot();
        let root = parsed.documents.document_node(parsed.doc_handle).unwrap();
        let parent_stack: RefCell<Vec<String>> = RefCell::new(Vec::new());
        check_repeated(xot, root, &parent_stack, &mut report, &fixture);
    }
    if !report.is_empty() {
        report.print(
            "Repeated parent/child same-name nesting (likely Flatten/rename gap)",
        );
        if ASSERT_INVARIANTS && ASSERT_NO_REPEATED_NAME {
            panic!("repeated parent/child same-name nesting");
        }
    }
}

fn check_repeated(
    xot: &Xot,
    node: Node,
    parent_stack: &std::cell::RefCell<Vec<String>>,
    report: &mut Report,
    fixture: &Path,
) {
    let name = element_name(xot, node);
    if let Some(name) = name.as_deref() {
        if !REPEATED_NAME_WHITELIST.contains(&name) {
            if let Some(parent_name) = parent_stack.borrow().last() {
                if parent_name == name {
                    report.record(
                        name,
                        fixture,
                        format!("<{name}> nested directly under <{name}>"),
                    );
                }
            }
        }
    }
    let pushed = if let Some(name) = name {
        parent_stack.borrow_mut().push(name);
        true
    } else {
        false
    };
    for child in xot.children(node) {
        check_repeated(xot, child, parent_stack, report, fixture);
    }
    if pushed {
        parent_stack.borrow_mut().pop();
    }
}
