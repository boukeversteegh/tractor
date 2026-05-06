#![cfg(feature = "native")]

use std::fs;
use tree_sitter::Parser;

use tractor::ir::{audit_coverage, lower_php_root, to_source};

#[test]
#[ignore]
fn php_missing_kinds() {
    let candidates = [
        "../tests/integration/languages/php/blueprint.php",
        "tests/integration/languages/php/blueprint.php",
    ];
    let path = candidates.iter().find(|c| fs::metadata(c).is_ok()).expect("blueprint");
    let source = fs::read_to_string(path).expect("read");

    let mut p = Parser::new();
    p.set_language(&tree_sitter_php::LANGUAGE_PHP.into()).unwrap();
    let tree = p.parse(&source, None).unwrap();
    let ir = lower_php_root(tree.root_node(), &source);

    assert_eq!(to_source(&ir, &source), source, "round-trip identity broken");

    let report = audit_coverage(tree.root_node(), &ir, &source, &[]);
    eprintln!(
        "PHP coverage: {} kinds; {} CST nodes; {} dropped",
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
