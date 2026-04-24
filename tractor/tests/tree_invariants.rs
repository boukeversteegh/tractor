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

const ASSERT_INVARIANTS: bool = false;
const MAX_SHOWN_PER_KIND: usize = 10;

const DATA_LANG_EXTS: &[&str] = &["json", "yaml", "yml", "toml", "ini", "env"];

/// Node names allowed to contain an underscore. Everything else
/// with an underscore is almost certainly a tree-sitter grammar
/// kind leaking through (e.g. `if_statement`, `variable_declarator`,
/// `parenthesized_expression`).
const ALLOWED_UNDERSCORE_NAMES: &[&str] = &[
    "else_if",
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
        if ASSERT_INVARIANTS {
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
    if !report.is_empty() {
        report.print("Principle #1/#2 — no tree-sitter kind leaks (underscored names)");
        if ASSERT_INVARIANTS {
            panic!("underscored node names");
        }
    }
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
        if ASSERT_INVARIANTS {
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
        if ASSERT_INVARIANTS {
            panic!("<name> with element children");
        }
    }
}

// ---------------------------------------------------------------------------
// Invariant 5: Markers stay empty.
//
// Principle #7 + the modifiers-as-empty-elements decision: any
// node we treat as a marker (queryable flag) must have no children
// except possibly whitespace text. We encode this by checking that
// any element which IS empty in some fixture is ALWAYS empty
// everywhere — symptom of markers-that-became-non-empty.
//
// Simpler version for now: just collect element names that have
// mixed empty/non-empty instances. Flags candidates for review.
// ---------------------------------------------------------------------------

#[test]
fn markers_stay_empty() {
    use std::collections::HashMap;

    // Per element-name, (seen_empty, seen_non_empty).
    let mut stats: HashMap<String, (bool, bool, Vec<(PathBuf, String)>)> = HashMap::new();

    for fixture in iter_fixtures() {
        let Some(parsed) = parse_structure(&fixture) else { continue };
        let xot = parsed.documents.xot();
        let root = parsed.documents.document_node(parsed.doc_handle).unwrap();
        walk_elements(xot, root, &mut |xot, node| {
            let Some(name) = element_name(xot, node) else { return };
            let has_content = xot.children(node).any(|c| {
                xot.element(c).is_some()
                    || xot.text_str(c).map(|s| !s.trim().is_empty()).unwrap_or(false)
            });
            let entry = stats
                .entry(name.clone())
                .or_insert((false, false, Vec::new()));
            if has_content {
                entry.1 = true;
                if entry.0 {
                    entry.2.push((fixture.clone(), "non-empty instance".into()));
                }
            } else {
                entry.0 = true;
            }
        });
    }

    // Only names that are BOTH empty somewhere AND non-empty
    // somewhere are suspicious — they might be markers in some
    // contexts and structural elements in others.
    let mut report = Report::default();
    for (name, (saw_empty, saw_nonempty, hits)) in &stats {
        if *saw_empty && *saw_nonempty {
            for (fixture, ctx) in hits.iter().take(MAX_SHOWN_PER_KIND) {
                report.record(name, fixture, ctx.clone());
            }
        }
    }
    if !report.is_empty() {
        report.print(
            "Principle #7 — elements used as markers elsewhere are non-empty here (review candidates)",
        );
        if ASSERT_INVARIANTS {
            panic!("mixed-empty elements");
        }
    }
}
