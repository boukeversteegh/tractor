//! Generic per-language coverage report.
//!
//! For each language we have a blueprint for, parse it through the
//! production pipeline and count which CST kinds end up surfacing
//! as `<unknown kind="...">` in the final XML — those are the
//! still-uncovered kinds the lowering / transform layer didn't have
//! a mapping for.
//!
//! Run with: `cargo test --test coverage_report -- --ignored --nocapture`
//!
//! The output is a per-language summary intended to drive the next
//! coverage-push iteration. It is `#[ignore]` because it's a
//! diagnostic, not an assertion — the ratchet for "no <unknown>
//! anywhere" lives in `shape_contracts`.

#![cfg(feature = "native")]

use std::collections::BTreeMap;
use std::fs;

struct LangCase {
    name: &'static str,
    /// Path to the blueprint file from the workspace root.
    blueprint: &'static str,
}

const LANG_CASES: &[LangCase] = &[
    LangCase { name: "csharp",     blueprint: "tests/integration/languages/csharp/blueprint.cs" },
    LangCase { name: "java",       blueprint: "tests/integration/languages/java/blueprint.java" },
    LangCase { name: "python",     blueprint: "tests/integration/languages/python/blueprint.py" },
    LangCase { name: "typescript", blueprint: "tests/integration/languages/typescript/blueprint.ts" },
    LangCase { name: "rust",       blueprint: "tests/integration/languages/rust/blueprint.rs" },
    LangCase { name: "go",         blueprint: "tests/integration/languages/go/blueprint.go" },
    LangCase { name: "ruby",       blueprint: "tests/integration/languages/ruby/blueprint.rb" },
    LangCase { name: "php",        blueprint: "tests/integration/languages/php/blueprint.php" },
    LangCase { name: "tsql",       blueprint: "tests/integration/languages/tsql/blueprint.sql" },
    LangCase { name: "json",       blueprint: "tests/integration/languages/json/blueprint.json" },
    LangCase { name: "yaml",       blueprint: "tests/integration/languages/yaml/blueprint.yml" },
    LangCase { name: "toml",       blueprint: "tests/integration/languages/toml/blueprint.toml" },
    LangCase { name: "ini",        blueprint: "tests/integration/languages/ini/blueprint.ini" },
    LangCase { name: "markdown",   blueprint: "tests/integration/languages/markdown/blueprint.md" },
];

fn read_blueprint(rel: &str) -> Option<String> {
    let candidates = [rel.to_string(), format!("../{rel}")];
    for c in &candidates {
        if let Ok(s) = fs::read_to_string(c) {
            return Some(s);
        }
    }
    None
}

fn count_unknowns(xml: &str) -> BTreeMap<String, usize> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for token in xml.split("<unknown kind=\"").skip(1) {
        if let Some(end) = token.find('"') {
            *counts.entry(token[..end].to_string()).or_insert(0) += 1;
        }
    }
    counts
}

#[test]
#[ignore]
fn coverage_summary_all_languages() {
    println!("\n=== Per-language coverage report (blueprint <unknown> kinds) ===\n");
    let mut total_unknowns = 0usize;
    let mut total_ok = 0usize;
    let mut not_found: Vec<&str> = Vec::new();
    for case in LANG_CASES {
        let source = match read_blueprint(case.blueprint) {
            Some(s) => s,
            None => {
                not_found.push(case.name);
                continue;
            }
        };
        let parsed = match tractor::parser::parse_string_to_xot(
            &source,
            case.name,
            "<x>".to_string(),
            None,
        ) {
            Ok(p) => p,
            Err(e) => {
                println!("  [{}]  PARSE FAILED: {e}", case.name);
                continue;
            }
        };
        let root = if parsed.xot.is_document(parsed.root) {
            parsed
                .xot
                .document_element(parsed.root)
                .expect("doc element")
        } else {
            parsed.root
        };
        let xml = parsed.xot.to_string(root).unwrap();
        let counts = count_unknowns(&xml);
        if counts.is_empty() {
            println!("  [{}]  100% covered — 0 unknown kinds", case.name);
            total_ok += 1;
        } else {
            let total: usize = counts.values().sum();
            println!(
                "  [{}]  {} unknown occurrence(s) across {} kind(s):",
                case.name,
                total,
                counts.len(),
            );
            for (k, n) in &counts {
                println!("        {n:>3}  {k}");
            }
            total_unknowns += total;
        }
    }
    if !not_found.is_empty() {
        println!("\n  (no blueprint found for: {})", not_found.join(", "));
    }
    println!(
        "\n=== Summary: {} fully-covered languages, {} total unknown occurrence(s) ===",
        total_ok, total_unknowns,
    );
}
