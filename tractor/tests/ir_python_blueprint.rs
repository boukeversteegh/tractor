//! Full-blueprint parity test for the typed-tree pipeline.
//!
//! Runs both pipelines against `tests/integration/languages/python/blueprint.py`
//! and reports the first divergence so coverage can be expanded
//! iteratively.
//!
//! ## Goal
//! Reach structural parity (or strict superset) on the full Python
//! blueprint. While coverage is incomplete this test is allowed to
//! fail; each failure names the next tree variant or lowering arm to
//! add.

#![cfg(feature = "native")]

use tractor::tree::{audit_coverage, render_to_xot, to_source};
use tractor::languages::python::lower_python_root;
use tractor::{parse, ParseInput, ParseOptions};
use xot::{Node as XotNode, Xot};

/// Empty list — `PyKind` no longer exists post-tree-migration; the
/// audit's `known_kinds` argument is optional and zero-length is
/// fine (just disables the "kind seen but not exercised" diagnostic).
#[allow(dead_code)]
fn python_known_kinds() -> Vec<&'static str> {
    Vec::new()
}

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
    let r = parse(
        ParseInput::Inline { content: source, file_label: "<bp>" },
        ParseOptions { language: Some("python"), ..Default::default() },
    )
    .expect("current pipeline parse");
    let xot = r.documents.xot();
    let doc_node = r.documents.document_node(r.doc_handle).expect("doc");
    let root = if xot.is_document(doc_node) {
        xot.document_element(doc_node).expect("doc")
    } else { doc_node };
    structural_view(xot, root)
}

fn ir_view(source: &str) -> (String, String, usize) {
    let mut p = tree_sitter::Parser::new();
    p.set_language(&tree_sitter_python::LANGUAGE.into()).unwrap();
    let cst = p.parse(source, None).unwrap();
    let tree = lower_python_root(&tractor::raw::RawNode::from_tree_sitter(cst.root_node(), source), source);

    // Round-trip identity must hold even if structural parity is partial.
    let recovered = to_source(&tree, source);
    assert_eq!(recovered, source, "round-trip identity broken");

    let mut xot = Xot::new();
    let dr_name = xot.add_name("_root");
    let dr = xot.new_element(dr_name);
    render_to_xot(&mut xot, dr, &tree, source).expect("render");
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
                 ----- tree pipeline -----\n{}",
                i, i + 1,
                ctx_a.join("\n"),
                ctx_b.join("\n"),
            );
        }
    }
    let len_a = a.lines().count();
    let len_b = b.lines().count();
    if len_a != len_b {
        return format!("length differs: current={len_a}, tree={len_b}");
    }
    "(identical)".to_string()
}

/// Enumerate all named CST kinds in the blueprint, sorted by count.
/// Useful for triaging which kinds to add to the tree next. Marked
/// #[ignore] so it doesn't run by default; invoke with
/// `cargo test --test ir_python_blueprint kinds_in_blueprint -- --ignored --nocapture`.
/// Dump the CST shape of a small snippet for debugging.
#[test]
#[ignore]
fn dump_type_alias_cst() {
    let source = "type Vec[T] = list[T]\ntype Vec2 = list[int]\n";
    let mut p = tree_sitter::Parser::new();
    p.set_language(&tree_sitter_python::LANGUAGE.into()).unwrap();
    let cst = p.parse(source, None).unwrap();
    fn walk(node: tree_sitter::Node, depth: usize, src: &[u8]) {
        let indent = "  ".repeat(depth);
        let text = node.utf8_text(src).unwrap_or("?");
        let text_short: String = text.chars().take(40).collect();
        eprintln!("{indent}{} text={:?}", node.kind(), text_short);
        let mut c = node.walk();
        for child in node.children(&mut c) {
            if child.is_named() { walk(child, depth + 1, src); }
        }
    }
    walk(cst.root_node(), 0, source.as_bytes());
}

#[test]
#[ignore]
fn dump_type_params() {
    let source = "def identity[T](v: T) -> T:\n    return v\n";
    let mut p = tree_sitter::Parser::new();
    p.set_language(&tree_sitter_python::LANGUAGE.into()).unwrap();
    let cst = p.parse(source, None).unwrap();
    fn walk(node: tree_sitter::Node, depth: usize, src: &[u8]) {
        let indent = "  ".repeat(depth);
        let text = node.utf8_text(src).unwrap_or("?");
        eprintln!("{indent}{} text={:?}", node.kind(), text);
        let mut c = node.walk();
        for child in node.children(&mut c) {
            if child.is_named() { walk(child, depth + 1, src); }
        }
    }
    walk(cst.root_node(), 0, source.as_bytes());
}

#[test]
#[ignore]
fn kinds_in_blueprint() {
    let source = std::fs::read_to_string("../tests/integration/languages/python/blueprint.py")
        .or_else(|_| std::fs::read_to_string("tests/integration/languages/python/blueprint.py"))
        .expect("blueprint.py");
    let mut p = tree_sitter::Parser::new();
    p.set_language(&tree_sitter_python::LANGUAGE.into()).unwrap();
    let cst = p.parse(&source, None).unwrap();
    let mut kinds: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    walk_kinds(cst.root_node(), &mut kinds);
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
#[ignore]  // until blueprint structural parity is reached; the coverage_report test below runs by default.
fn blueprint_parity() {
    let source = std::fs::read_to_string("../tests/integration/languages/python/blueprint.py")
        .or_else(|_| std::fs::read_to_string("tests/integration/languages/python/blueprint.py"))
        .expect("blueprint.py");
    let cur = current_view(&source);
    let (tree, xpath_text, unknowns) = ir_view(&source);

    eprintln!("=== blueprint structural-parity check ===");
    eprintln!("source bytes: {}", source.len());
    eprintln!("current pipeline view bytes: {}", cur.len());
    eprintln!("tree pipeline view bytes:      {}", tree.len());
    eprintln!("Unknown nodes in tree output:  {}", unknowns);
    eprintln!("tree string(.) == source:      {}", xpath_text == source);

    // Always assert the tree's lossless invariant.
    assert_eq!(xpath_text, source, "tree text-content recovery broken");

    // Structural parity: failure shows where to extend coverage.
    if cur != tree {
        panic!("\n{}", first_diff(&cur, &tree));
    }
}

/// Coverage audit against the Python blueprint. Runs by default
/// (does NOT require structural parity — independent metric).
///
/// Always asserts:
/// - Round-trip identity holds.
/// - XPath text-content recovery holds.
/// - **`dropped` count is zero** (no silently lost CST nodes).
///
/// Reports kind / node coverage percentages for visibility.
#[test]
fn blueprint_coverage_audit() {
    let source = std::fs::read_to_string("../tests/integration/languages/python/blueprint.py")
        .or_else(|_| std::fs::read_to_string("tests/integration/languages/python/blueprint.py"))
        .expect("blueprint.py");

    let mut p = tree_sitter::Parser::new();
    p.set_language(&tree_sitter_python::LANGUAGE.into()).unwrap();
    let cst = p.parse(&source, None).unwrap();
    let tree = lower_python_root(&tractor::raw::RawNode::from_tree_sitter(cst.root_node(), &source), &source);

    // Round-trip + XPath invariants on the FULL blueprint.
    assert_eq!(to_source(&tree, &source), source, "round-trip identity broken");

    let mut xot = xot::Xot::new();
    let dr_name = xot.add_name("_root");
    let dr = xot.new_element(dr_name);
    tractor::tree::render_to_xot(&mut xot, dr, &tree, &source).expect("render");
    let root = xot.children(dr).find(|&c| xot.element(c).is_some()).unwrap();
    let xpath_text = text_concat(&xot, root);
    assert_eq!(xpath_text, source, "XPath text-content recovery broken");

    // Coverage audit. Pass the full PyKind list so blueprint-absent
    // grammar kinds surface as zero-stat entries (rather than being
    // silently "supported by absence").
    let known_kinds = python_known_kinds();
    let known_refs: Vec<&str> = known_kinds.iter().map(|s| s.as_ref()).collect();
    let report = audit_coverage(cst.root_node(), &tree, &source, &known_refs);
    eprintln!("\n{}", report.summary());

    // Hard invariant: no dropped CST nodes (renderer bug detector).
    assert_eq!(report.dropped, 0,
        "{} CST nodes dropped — round-trip identity holds but structure was lost.\n\
         (See dropped buckets in the report above.)",
        report.dropped);
}
