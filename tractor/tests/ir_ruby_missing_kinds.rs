#![cfg(feature = "native")]

use std::fs;
use tree_sitter::Parser;

use tractor::tree::{audit_coverage, to_source};
use tractor::languages::ruby::lower_ruby_root;

#[test]
#[ignore]
fn ruby_missing_kinds() {
    let candidates = [
        "../tests/integration/languages/ruby/blueprint.rb",
        "tests/integration/languages/ruby/blueprint.rb",
    ];
    let path = candidates.iter().find(|c| fs::metadata(c).is_ok()).expect("blueprint");
    let source = fs::read_to_string(path).expect("read");

    let mut p = Parser::new();
    p.set_language(&tree_sitter_ruby::LANGUAGE.into()).unwrap();
    let cst = p.parse(&source, None).unwrap();
    let tree = lower_ruby_root(&tractor::raw::RawNode::from_tree_sitter(cst.root_node(), &source), &source);

    assert_eq!(to_source(&tree, &source), source, "round-trip identity broken");

    let report = audit_coverage(cst.root_node(), &tree, &source, &[]);
    eprintln!(
        "Ruby coverage: {} kinds; {} CST nodes; {} dropped",
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
