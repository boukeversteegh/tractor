//! Layer 1 of the transform-validation architecture
//! (`docs/transform-validation-architecture.md` § 3).
//!
//! Iterates every blueprint fixture, parses + transforms via the
//! standard library entry point, and runs every rule from
//! `tractor::transform::shape_contracts::RULES` against the resulting
//! tree.
//!
//! - `Severity::Error` violations fail the test.
//! - `Severity::Advisory` violations print diagnostics without failing
//!   (used while a rule's target population is on its way to zero).
//!
//! Phase 2 will extend the rule list and migrate selected hand-coded
//! invariants from `tree_invariants.rs` into the unified spec-conformance
//! walker.

use std::collections::BTreeMap;
use std::path::PathBuf;

use tractor::transform::shape_contracts::{validate_shape_contracts, Severity, Violation};
use tractor::{parse, ParseInput, ParseOptions};

const DATA_LANG_EXTS: &[&str] = &["json", "yaml", "yml", "toml", "ini", "env"];

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
            if DATA_LANG_EXTS.contains(&ext) {
                continue;
            }
            out.push(path);
        }
    }
    out.sort();
    out
}

#[test]
fn shape_contracts_against_blueprints() {
    // (rule_id, severity) → (count, sample messages)
    let mut grouped: BTreeMap<(&str, Severity), (usize, Vec<String>)> = BTreeMap::new();
    let mut error_total = 0usize;
    let mut advisory_total = 0usize;

    for fixture in iter_fixtures() {
        let ext = fixture.extension().and_then(|e| e.to_str()).unwrap_or("");
        let Some(lang) = lang_from_ext(ext) else { continue };

        let parsed = match parse(
            ParseInput::Disk { path: &fixture },
            ParseOptions::default(),
        ) {
            Ok(p) => p,
            Err(_) => continue,
        };
        let xot = parsed.documents.xot();
        let Some(root) = parsed.documents.document_node(parsed.doc_handle) else {
            continue;
        };

        let violations: Vec<Violation> = validate_shape_contracts(xot, root, lang);
        for v in violations {
            let key = (v.rule_id, v.severity);
            let entry = grouped.entry(key).or_insert((0, Vec::new()));
            entry.0 += 1;
            // Keep the first 25 sample messages per (rule, severity) for
            // diagnostic readability.
            if entry.1.len() < 25 {
                let rel = fixture
                    .strip_prefix(languages_dir())
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|_| fixture.display().to_string());
                entry.1.push(format!("{rel}: {}", v.message));
            }
            match v.severity {
                Severity::Error => error_total += 1,
                Severity::Advisory => advisory_total += 1,
            }
        }
    }

    if !grouped.is_empty() {
        eprintln!();
        eprintln!(
            "shape-contract scan — {error_total} error(s), {advisory_total} advisory"
        );
        eprintln!();
        for ((rule_id, severity), (count, samples)) in &grouped {
            let tag = match severity {
                Severity::Error => "ERROR",
                Severity::Advisory => "advisory",
            };
            eprintln!("  [{tag}] {rule_id} — {count} occurrence(s)");
            for line in samples {
                eprintln!("    {line}");
            }
            if *count > samples.len() {
                eprintln!("    … and {} more", count - samples.len());
            }
        }
        eprintln!();
    }

    assert_eq!(
        error_total, 0,
        "shape-contract Error violations must be zero ({error_total} found)"
    );
}
