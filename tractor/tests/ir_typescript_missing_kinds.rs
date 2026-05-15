#![cfg(feature = "native")]

use std::fs;
use tree_sitter::Parser;

use tractor::tree::audit_coverage;
use tractor::languages::typescript::lower_typescript_root;

/// Coverage diagnostic for the TypeScript tree pipeline.
/// Run with `cargo test --test ir_typescript_missing_kinds -- --ignored --nocapture`
/// to see typed-vs-untyped CST kind counts and any unknowns leaking
/// through to the final pipeline output.
#[test]
#[ignore]
fn typescript_missing_kinds() {
    let candidates = [
        "../tests/integration/languages/typescript/blueprint.ts",
        "tests/integration/languages/typescript/blueprint.ts",
    ];
    let path = candidates
        .iter()
        .find(|c| fs::metadata(c).is_ok())
        .expect("blueprint");
    let source = fs::read_to_string(path).expect("read");

    let mut p = Parser::new();
    p.set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
        .unwrap();
    let cst = p.parse(&source, None).unwrap();
    let tree = lower_typescript_root(&tractor::raw::RawNode::from_tree_sitter(cst.root_node(), &source), &source);

    let report = audit_coverage(cst.root_node(), &tree, &source, &[]);
    eprintln!(
        "TypeScript coverage: {} kinds; {} CST nodes",
        report.by_kind.len(),
        report.total_named_cst_nodes,
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

    let parsed = tractor::parse(
        tractor::ParseInput::Inline { content: &source, file_label: "<x>" },
        tractor::ParseOptions { language: Some("typescript"), ..Default::default() },
    )
    .expect("parse");
    let xot = parsed.documents.xot();
    let doc_node = parsed.documents.document_node(parsed.doc_handle).expect("doc");
    let root = if xot.is_document(doc_node) {
        xot.document_element(doc_node).expect("doc")
    } else {
        doc_node
    };
    let final_xml = xot.to_string(root).unwrap();
    let mut counts = std::collections::BTreeMap::<String, usize>::new();
    for token in final_xml.split("<unknown kind=\"").skip(1) {
        if let Some(end) = token.find('"') {
            *counts.entry(token[..end].to_string()).or_insert(0) += 1;
        }
    }
    eprintln!("\nUnknowns in FINAL pipeline output (deep walk):");
    for (k, n) in &counts {
        eprintln!("  {n:>3}  {k}");
    }
}
