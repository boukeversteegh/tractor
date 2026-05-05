#![cfg(feature = "native")]

use std::fs;
use tree_sitter::Parser;

use tractor::ir::{audit_coverage, lower_java_root};

#[test]
#[ignore]
fn java_missing_kinds() {
    let candidates = [
        "../tests/integration/languages/java/blueprint.java",
        "tests/integration/languages/java/blueprint.java",
    ];
    let path = candidates
        .iter()
        .find(|c| fs::metadata(c).is_ok())
        .expect("blueprint");
    let source = fs::read_to_string(path).expect("read");

    let mut p = Parser::new();
    p.set_language(&tree_sitter_java::LANGUAGE.into()).unwrap();
    let tree = p.parse(&source, None).unwrap();
    let ir = lower_java_root(tree.root_node(), &source);

    let report = audit_coverage(tree.root_node(), &ir, &source, &[]);
    eprintln!(
        "Java coverage: {} kinds; {} CST nodes",
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

    let parsed = tractor::parser::parse_string_to_xot(
        &source,
        "java",
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
    eprintln!("\nUnknowns in FINAL pipeline output (deep walk):");
    for (k, n) in &counts {
        eprintln!("  {n:>3}  {k}");
    }
}

#[test]
#[ignore]
fn dump_java_field_render() {
    let s = "class C { int x = 1; int a = 1, b = 2; void f() { int u = 1, v = 2; } }\n";
    let parsed = tractor::parser::parse_string_to_xot(
        s, "java", "<x>".to_string(), None,
    ).expect("parse");
    let root = if parsed.xot.is_document(parsed.root) {
        parsed.xot.document_element(parsed.root).expect("doc")
    } else { parsed.root };
    println!("{}", parsed.xot.to_string(root).unwrap());
}
