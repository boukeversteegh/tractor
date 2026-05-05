//! Diagnostic: list Python CST kinds that the IR pipeline lowers to
//! `Ir::Unknown`. Run with `--ignored` and `--nocapture` to see the
//! list — the blueprint provides a representative corpus.

#![cfg(feature = "native")]

use std::fs;
use tree_sitter::Parser;

use tractor::ir::{audit_coverage, lower_python_root};

#[test]
#[ignore]
fn python_missing_kinds() {
    let candidates = [
        "../tests/integration/languages/python/blueprint.py",
        "tests/integration/languages/python/blueprint.py",
    ];
    let path = candidates.iter().find(|c| fs::metadata(c).is_ok()).expect("blueprint");
    let source = fs::read_to_string(path).expect("read");

    let mut p = Parser::new();
    p.set_language(&tree_sitter_python::LANGUAGE.into()).unwrap();
    let tree = p.parse(&source, None).unwrap();
    let ir = lower_python_root(tree.root_node(), &source);

    let report = audit_coverage(tree.root_node(), &ir, &source, &[]);
    eprintln!(
        "Python coverage: {} kinds; {} CST nodes",
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

    // Render to XML and grep `<unknown kind="X">` to surface every
    // inner-handler fallthrough (the audit only sees outer kinds).
    let mut xot = xot::Xot::new();
    let doc = xot.new_document();
    tractor::ir::render_to_xot(&mut xot, doc, &ir, &source).expect("render");
    let xml = xot.to_string(doc).unwrap();
    let mut counts = std::collections::BTreeMap::<String, usize>::new();
    for token in xml.split("<unknown kind=\"").skip(1) {
        if let Some(end) = token.find('"') {
            *counts.entry(token[..end].to_string()).or_insert(0) += 1;
        }
    }
    eprintln!("\nUnknown kind values (deep walk):");
    for (k, n) in &counts {
        eprintln!("  {n:>3}  {k}");
    }

    // Final-pipeline check: parse via the actual parser entry
    // (which runs IR + post_transform) and grep the final XML for
    // `<unknown` to see what survives the full pipeline.
    let parsed = tractor::parser::parse_string_to_xot(
        &source,
        "python",
        "<x>".to_string(),
        None,
    ).expect("parse");
    let root = if parsed.xot.is_document(parsed.root) {
        parsed.xot.document_element(parsed.root).expect("doc")
    } else { parsed.root };
    let final_xml = parsed.xot.to_string(root).unwrap();
    let mut final_counts = std::collections::BTreeMap::<String, usize>::new();
    for token in final_xml.split("<unknown kind=\"").skip(1) {
        if let Some(end) = token.find('"') {
            *final_counts.entry(token[..end].to_string()).or_insert(0) += 1;
        }
    }
    eprintln!("\nUnknowns in FINAL pipeline output (post post_transform):");
    for (k, n) in &final_counts {
        eprintln!("  {n:>3}  {k}");
    }
    eprintln!("any 'unknown' substring count: {}", final_xml.matches("unknown").count());
    if let Some(pos) = final_xml.find("unknown") {
        let from = pos.saturating_sub(120);
        let to = (pos + 200).min(final_xml.len());
        eprintln!("first 'unknown' context:\n{}", &final_xml[from..to]);
    }
}
