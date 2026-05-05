//! Full-blueprint parity test for the typed-IR pipeline.
//!
//! Runs both pipelines against `tests/integration/languages/python/blueprint.py`
//! and reports the first divergence so coverage can be expanded
//! iteratively.
//!
//! ## Goal
//! Reach structural parity (or strict superset) on the full Python
//! blueprint. While coverage is incomplete this test is allowed to
//! fail; each failure names the next IR variant or lowering arm to
//! add.

#![cfg(feature = "native")]

use tractor::ir::{lower_python_root, render_to_xot, to_source};
use tractor::parser::parse_string_to_xot;
use xot::{Node as XotNode, Xot};

fn structural_view(xot: &Xot, root: XotNode) -> String {
    let mut out = String::new();
    walk(xot, root, 0, &mut out);
    out
}

fn walk(xot: &Xot, node: XotNode, depth: usize, out: &mut String) {
    if let Some(elem) = xot.element(node) {
        let name = xot.local_name_str(elem.name());
        for _ in 0..depth { out.push_str("  "); }
        out.push_str(name);
        let has_child = xot.children(node).any(|c| xot.element(c).is_some());
        if !has_child {
            let direct: String = xot
                .children(node)
                .filter_map(|c| xot.text_str(c).map(|s| s.to_string()))
                .collect();
            if !direct.is_empty() {
                out.push_str(" text=");
                out.push_str(&format!("{:?}", direct));
            }
        }
        out.push('\n');
        for c in xot.children(node) {
            if xot.element(c).is_some() { walk(xot, c, depth + 1, out); }
        }
    }
}

fn current_view(source: &str) -> String {
    let r = parse_string_to_xot(source, "python", "<bp>".to_string(), None)
        .expect("current pipeline parse");
    let root = if r.xot.is_document(r.root) {
        r.xot.document_element(r.root).expect("doc")
    } else { r.root };
    structural_view(&r.xot, root)
}

fn ir_view(source: &str) -> (String, String, usize) {
    let mut p = tree_sitter::Parser::new();
    p.set_language(&tree_sitter_python::LANGUAGE.into()).unwrap();
    let tree = p.parse(source, None).unwrap();
    let ir = lower_python_root(tree.root_node(), source);

    // Round-trip identity must hold even if structural parity is partial.
    let recovered = to_source(&ir, source);
    assert_eq!(recovered, source, "round-trip identity broken");

    let mut xot = Xot::new();
    let dr_name = xot.add_name("_root");
    let dr = xot.new_element(dr_name);
    render_to_xot(&mut xot, dr, &ir, source).expect("render");
    let root = xot.children(dr).find(|&c| xot.element(c).is_some()).unwrap();

    // Count Unknown elements as a coverage signal.
    let mut unknowns = 0usize;
    count_unknowns(&xot, root, &mut unknowns);

    let view = structural_view(&xot, root);
    let xpath_text = text_concat(&xot, root);
    (view, xpath_text, unknowns)
}

fn count_unknowns(xot: &Xot, node: XotNode, out: &mut usize) {
    if let Some(elem) = xot.element(node) {
        let name = xot.local_name_str(elem.name());
        if name == "unknown" { *out += 1; }
    }
    for c in xot.children(node) { count_unknowns(xot, c, out); }
}

fn text_concat(xot: &Xot, node: XotNode) -> String {
    let mut out = String::new();
    walk_text(xot, node, &mut out);
    out
}
fn walk_text(xot: &Xot, node: XotNode, out: &mut String) {
    for c in xot.children(node) {
        if let Some(s) = xot.text_str(c) { out.push_str(s); }
        if xot.element(c).is_some() { walk_text(xot, c, out); }
    }
}

fn first_diff(a: &str, b: &str) -> String {
    for (i, (la, lb)) in a.lines().zip(b.lines()).enumerate() {
        if la != lb {
            let start = i.saturating_sub(3);
            let end = (i + 4).min(a.lines().count().min(b.lines().count()));
            let ctx_a: Vec<&str> = a.lines().skip(start).take(end - start).collect();
            let ctx_b: Vec<&str> = b.lines().skip(start).take(end - start).collect();
            return format!(
                "first diff at line {} (1-based: {})\n\
                 ----- current pipeline -----\n{}\n\
                 ----- IR pipeline -----\n{}",
                i, i + 1,
                ctx_a.join("\n"),
                ctx_b.join("\n"),
            );
        }
    }
    let len_a = a.lines().count();
    let len_b = b.lines().count();
    if len_a != len_b {
        return format!("length differs: current={len_a}, IR={len_b}");
    }
    "(identical)".to_string()
}

/// Enumerate all named CST kinds in the blueprint, sorted by count.
/// Useful for triaging which kinds to add to the IR next. Marked
/// #[ignore] so it doesn't run by default; invoke with
/// `cargo test --test ir_python_blueprint kinds_in_blueprint -- --ignored --nocapture`.
/// Dump the CST shape of a small snippet for debugging.
#[test]
#[ignore]
fn dump_type_params() {
    let source = "def identity[T](v: T) -> T:\n    return v\n";
    let mut p = tree_sitter::Parser::new();
    p.set_language(&tree_sitter_python::LANGUAGE.into()).unwrap();
    let tree = p.parse(source, None).unwrap();
    fn walk(node: tree_sitter::Node, depth: usize, src: &[u8]) {
        let indent = "  ".repeat(depth);
        let text = node.utf8_text(src).unwrap_or("?");
        eprintln!("{indent}{} text={:?}", node.kind(), text);
        let mut c = node.walk();
        for child in node.children(&mut c) {
            if child.is_named() { walk(child, depth + 1, src); }
        }
    }
    walk(tree.root_node(), 0, source.as_bytes());
}

#[test]
#[ignore]
fn kinds_in_blueprint() {
    let source = std::fs::read_to_string("../tests/integration/languages/python/blueprint.py")
        .or_else(|_| std::fs::read_to_string("tests/integration/languages/python/blueprint.py"))
        .expect("blueprint.py");
    let mut p = tree_sitter::Parser::new();
    p.set_language(&tree_sitter_python::LANGUAGE.into()).unwrap();
    let tree = p.parse(&source, None).unwrap();
    let mut kinds: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    walk_kinds(tree.root_node(), &mut kinds);
    let mut sorted: Vec<(String, usize)> = kinds.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));
    for (k, v) in sorted {
        eprintln!("{:5} {}", v, k);
    }
}
fn walk_kinds(node: tree_sitter::Node, out: &mut std::collections::BTreeMap<String, usize>) {
    if node.is_named() {
        *out.entry(node.kind().to_string()).or_insert(0) += 1;
    }
    let mut c = node.walk();
    for child in node.named_children(&mut c) { walk_kinds(child, out); }
}

#[test]
fn blueprint_parity() {
    let source = std::fs::read_to_string("../tests/integration/languages/python/blueprint.py")
        .or_else(|_| std::fs::read_to_string("tests/integration/languages/python/blueprint.py"))
        .expect("blueprint.py");
    let cur = current_view(&source);
    let (ir, xpath_text, unknowns) = ir_view(&source);

    eprintln!("=== blueprint coverage report ===");
    eprintln!("source bytes: {}", source.len());
    eprintln!("current pipeline view bytes: {}", cur.len());
    eprintln!("IR pipeline view bytes:      {}", ir.len());
    eprintln!("Unknown nodes in IR output:  {}", unknowns);
    eprintln!("IR string(.) == source:      {}", xpath_text == source);

    // Always assert the IR's lossless invariant.
    assert_eq!(xpath_text, source, "IR text-content recovery broken");

    // Structural parity: failure shows where to extend coverage.
    if cur != ir {
        panic!("\n{}", first_diff(&cur, &ir));
    }
}
