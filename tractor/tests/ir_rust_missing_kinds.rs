#![cfg(feature = "native")]

use std::fs;
use tree_sitter::Parser;

use tractor::ir::{audit_coverage, lower_rust_root, to_source};

/// Diagnostic for the Rust IR pipeline. Reports CST kind coverage,
/// roundtrip identity, and any unknowns in the IR. The production
/// pipeline does NOT yet route Rust through this lowering.
#[test]
#[ignore]
fn rust_missing_kinds() {
    let candidates = [
        "../tests/integration/languages/rust/blueprint.rs",
        "tests/integration/languages/rust/blueprint.rs",
    ];
    let path = candidates
        .iter()
        .find(|c| fs::metadata(c).is_ok())
        .expect("blueprint");
    let source = fs::read_to_string(path).expect("read");

    let mut p = Parser::new();
    p.set_language(&tree_sitter_rust::LANGUAGE.into()).unwrap();
    let tree = p.parse(&source, None).unwrap();
    let ir = lower_rust_root(tree.root_node(), &source);

    // Round-trip identity.
    let recovered = to_source(&ir, &source);
    assert_eq!(recovered, source, "round-trip identity broken");

    let report = audit_coverage(tree.root_node(), &ir, &source, &[]);
    eprintln!(
        "Rust coverage: {} kinds; {} CST nodes; {} dropped",
        report.by_kind.len(),
        report.total_named_cst_nodes,
        report.dropped,
    );

    let mut untyped: Vec<(String, usize)> = report
        .by_kind
        .iter()
        .filter(|(_, s)| s.unknown > 0)
        .map(|(k, s)| (k.clone(), s.unknown))
        .collect();
    untyped.sort_by_key(|(_, n)| std::cmp::Reverse(*n));

    eprintln!("\nUntyped kinds (count):");
    for (k, n) in &untyped {
        eprintln!("  {n:>3}  {k}");
    }

    let parsed = tractor::parser::parse_string_to_xot(
        &source,
        "rust",
        "<x>".to_string(),
        None,
    )
    .expect("parse");
    let root = if parsed.xot.is_document(parsed.root) {
        parsed.xot.document_element(parsed.root).expect("doc")
    } else {
        parsed.root
    };
    let final_xml = parsed.xot.to_string(root).unwrap();
    let mut counts = std::collections::BTreeMap::<String, usize>::new();
    for token in final_xml.split("<unknown kind=\"").skip(1) {
        if let Some(end) = token.find('"') {
            *counts.entry(token[..end].to_string()).or_insert(0) += 1;
        }
    }
    eprintln!("\nUnknowns in FINAL pipeline output:");
    for (k, n) in &counts { eprintln!("  {n:>3}  {k}"); }
}
