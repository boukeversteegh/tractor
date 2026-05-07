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
    eprintln!("\n=== Look for nested expression/body ===");
    for needle in &["<expression line=\"58\"", "<body line=\"138\""] {
        if let Some(pos) = final_xml.find(needle) {
            let from = pos.saturating_sub(120);
            let to = (pos + 400).min(final_xml.len());
            eprintln!("\n{needle}\n{}", &final_xml[from..to]);
        }
    }
}

#[test]
#[ignore]
fn dump_chain_render() {
    let s = "obj.foo().bar.baz()\n";
    let parsed = tractor::parser::parse_string_to_xot(
        s, "python", "<x>".to_string(), None,
    ).expect("parse");
    let root = if parsed.xot.is_document(parsed.root) {
        parsed.xot.document_element(parsed.root).expect("doc")
    } else { parsed.root };
    println!("{}", parsed.xot.to_string(root).unwrap());
}

#[test]
#[ignore]
fn dump_except_render() {
    let s = "try:\n    f()\nexcept ValueError as err:\n    g()\n";
    let parsed = tractor::parser::parse_string_to_xot(
        s, "python", "<x>".to_string(), None,
    ).expect("parse");
    let root = if parsed.xot.is_document(parsed.root) {
        parsed.xot.document_element(parsed.root).expect("doc")
    } else { parsed.root };
    println!("{}", parsed.xot.to_string(root).unwrap());
}

#[test]
#[ignore]
fn dump_list_pattern_render() {
    let s = "match x:\n    case [1, 2, *rest]:\n        pass\n";
    let parsed = tractor::parser::parse_string_to_xot(
        s, "python", "<x>".to_string(), None,
    ).expect("parse");
    let root = if parsed.xot.is_document(parsed.root) {
        parsed.xot.document_element(parsed.root).expect("doc")
    } else { parsed.root };
    println!("{}", parsed.xot.to_string(root).unwrap());
}

#[test]
#[ignore]
fn dump_fstring_format_cst() {
    let s = "x = f\"hello {name!r}, value={n:>05d} nested={f'{name}'}\"\n";
    let mut p = tree_sitter::Parser::new();
    p.set_language(&tree_sitter_python::LANGUAGE.into()).unwrap();
    let tree = p.parse(s, None).unwrap();
    fn walk(n: tree_sitter::Node, src: &[u8], depth: usize) {
        let indent = "  ".repeat(depth);
        let txt = n.utf8_text(src).unwrap_or("?");
        let short: String = txt.chars().take(40).collect();
        eprintln!("{indent}{} text={:?}", n.kind(), short);
        let mut c = n.walk();
        for ch in n.children(&mut c) { walk(ch, src, depth + 1); }
    }
    walk(tree.root_node(), s.as_bytes(), 0);
}

#[test]
#[ignore]
fn dump_dict_pattern_cst() {
    let s = "match x:\n    case {\"a\": 1, \"b\": 2}:\n        pass\n";
    let mut p = tree_sitter::Parser::new();
    p.set_language(&tree_sitter_python::LANGUAGE.into()).unwrap();
    let tree = p.parse(s, None).unwrap();
    fn walk(n: tree_sitter::Node, src: &[u8], depth: usize) {
        let indent = "  ".repeat(depth);
        let txt = n.utf8_text(src).unwrap_or("?");
        let short: String = txt.chars().take(40).collect();
        let mut field = None;
        if let Some(parent) = n.parent() {
            let mut c = parent.walk();
            for (i, ch) in parent.children(&mut c).enumerate() {
                if ch.id() == n.id() {
                    field = parent.field_name_for_child(i as u32);
                    break;
                }
            }
        }
        eprintln!("{indent}{}{} text={:?}", n.kind(), field.map(|f| format!(" [{f}]")).unwrap_or_default(), short);
        let mut c = n.walk();
        for ch in n.children(&mut c) { walk(ch, src, depth + 1); }
    }
    walk(tree.root_node(), s.as_bytes(), 0);
}

#[test]
#[ignore]
fn dump_dict_pattern_render() {
    let s = "match x:\n    case {\"a\": 1, \"b\": 2}:\n        pass\n";
    let parsed = tractor::parser::parse_string_to_xot(
        s, "python", "<x>".to_string(), None,
    ).expect("parse");
    let root = if parsed.xot.is_document(parsed.root) {
        parsed.xot.document_element(parsed.root).expect("doc")
    } else { parsed.root };
    println!("{}", parsed.xot.to_string(root).unwrap());
}
