#![cfg(feature = "native")]

use std::fs;
use tree_sitter::Parser;

use tractor::ir::{audit_coverage, lower_go_root, to_source};

#[test]
#[ignore]
fn go_missing_kinds() {
    let candidates = [
        "../tests/integration/languages/go/blueprint.go",
        "tests/integration/languages/go/blueprint.go",
    ];
    let path = candidates.iter().find(|c| fs::metadata(c).is_ok()).expect("blueprint");
    let source = fs::read_to_string(path).expect("read");

    let mut p = Parser::new();
    p.set_language(&tree_sitter_go::LANGUAGE.into()).unwrap();
    let tree = p.parse(&source, None).unwrap();
    let ir = lower_go_root(tree.root_node(), &source);

    assert_eq!(to_source(&ir, &source), source, "round-trip identity broken");

    let report = audit_coverage(tree.root_node(), &ir, &source, &[]);
    eprintln!(
        "Go coverage: {} kinds; {} CST nodes; {} dropped",
        report.by_kind.len(), report.total_named_cst_nodes, report.dropped,
    );

    let mut untyped: Vec<(String, usize)> = report.by_kind.iter()
        .filter(|(_, s)| s.unknown > 0)
        .map(|(k, s)| (k.clone(), s.unknown))
        .collect();
    untyped.sort_by_key(|(_, n)| std::cmp::Reverse(*n));

    eprintln!("\nUntyped kinds (count):");
    for (k, n) in &untyped { eprintln!("  {n:>3}  {k}"); }
}
