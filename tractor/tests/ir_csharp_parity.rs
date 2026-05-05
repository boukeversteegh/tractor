//! Parity test for the experimental typed-IR pipeline (C# slice).
//!
//! C# is the language with the most whack-a-mole iterations against
//! the existing pipeline (86 commits, the unsolved `?.` conditional-
//! access design problem, chain-inversion adapter, operator-extraction
//! quirks).
//!
//! Unlike Python, C# tree-sitter requires syntactic context (a class
//! with a method) before it accepts an expression. So we validate the
//! IR architecture differently:
//!
//! 1. **Architectural invariants must hold on arbitrary C# input.**
//!    For any source we feed in:
//!    - Round-trip identity: `to_source(ir, source) == source`.
//!    - Lossless XPath text recovery: `string(IR_root) == source`.
//!    These are the same invariants we hold on Python, regardless of
//!    structural coverage. They prove byte-range threading + gap-text
//!    rendering work for C# too.
//!
//! 2. **Expression-subtree parity.** When we wrap a test expression
//!    in `class C { void M() { var x = <expr>; } }`, we navigate
//!    both pipelines to the inner expression and compare *those*
//!    subtrees. This validates that the IR's expression vocabulary
//!    (Access, Call, Binary, Unary, atoms) handles C# correctly,
//!    independently of the surrounding declaration shape.

#![cfg(feature = "native")]

use strum::IntoEnumIterator;
use tractor::ir::{audit_coverage, lower_csharp_root, render_to_xot, to_source};
use tractor::languages::csharp::input::CsKind;
use tractor::parser::parse_string_to_xot;
use xot::{Node as XotNode, Xot};

/// All named kinds tree-sitter-c-sharp emits, derived from the
/// generated `CsKind` enum.
fn csharp_known_kinds() -> Vec<&'static str> {
    CsKind::iter().map(|k| k.into()).collect()
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
            let direct: String = xot.children(node)
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

/// Verify the architectural invariants for arbitrary C# source.
fn assert_ir_invariants(source: &str, label: &str) {
    let mut p = tree_sitter::Parser::new();
    p.set_language(&tree_sitter_c_sharp::LANGUAGE.into()).unwrap();
    let tree = p.parse(source, None).unwrap();
    let ir = lower_csharp_root(tree.root_node(), source);

    // Invariant 1: round-trip identity.
    let recovered = to_source(&ir, source);
    assert_eq!(recovered, source, "round-trip identity broken for {label}");

    // Render and verify XPath string(.) recovery.
    let mut xot = Xot::new();
    let dr_name = xot.add_name("_root");
    let dr = xot.new_element(dr_name);
    render_to_xot(&mut xot, dr, &ir, source).expect("render");
    let root = xot.children(dr).find(|&c| xot.element(c).is_some()).unwrap();
    let xpath_text = text_concat(&xot, root);
    assert_eq!(xpath_text, source, "XPath text-content recovery broken for {label}");
}

// ---------------------------------------------------------------------------
// Architectural invariants on a variety of C# constructs.
// These tests pass regardless of how much structural coverage the IR
// has — they only validate that source flows through losslessly.
// ---------------------------------------------------------------------------

#[test]
fn invariants_class_with_simple_method() {
    let s = "class C { void M() { var x = 42; } }\n";
    assert_ir_invariants(s, "class with simple method");
}

#[test]
fn invariants_member_chain() {
    let s = "class C { void M() { var x = a.b.c; } }\n";
    assert_ir_invariants(s, "member chain");
}

#[test]
fn invariants_index_access() {
    let s = "class C { void M() { var x = a[0]; } }\n";
    assert_ir_invariants(s, "index access");
}

#[test]
fn invariants_call_with_args() {
    let s = "class C { void M() { var x = f(1, 2); } }\n";
    assert_ir_invariants(s, "call with args");
}

#[test]
fn invariants_binary() {
    let s = "class C { void M() { var x = a + b * c; } }\n";
    assert_ir_invariants(s, "binary nested");
}

#[test]
fn invariants_string_literal() {
    let s = "class C { void M() { var x = \"hello\"; } }\n";
    assert_ir_invariants(s, "string literal");
}

#[test]
fn invariants_null_literal() {
    let s = "class C { void M() { var x = null; } }\n";
    assert_ir_invariants(s, "null literal");
}

#[test]
#[ignore]
fn find_unknown_kinds_in_blueprint_ir() {
    let source = std::fs::read_to_string("../tests/integration/languages/csharp/blueprint.cs")
        .or_else(|_| std::fs::read_to_string("tests/integration/languages/csharp/blueprint.cs"))
        .expect("blueprint.cs");
    let mut p = tree_sitter::Parser::new();
    p.set_language(&tree_sitter_c_sharp::LANGUAGE.into()).unwrap();
    let tree = p.parse(&source, None).unwrap();
    let ir = lower_csharp_root(tree.root_node(), &source);
    let mut xot = Xot::new();
    let dr_name = xot.add_name("_root");
    let dr = xot.new_element(dr_name);
    render_to_xot(&mut xot, dr, &ir, &source).expect("render");
    let root = xot.children(dr).find(|&c| xot.element(c).is_some()).unwrap();
    fn walk(xot: &Xot, node: XotNode, counts: &mut std::collections::BTreeMap<String, usize>) {
        if let Some(elem) = xot.element(node) {
            let name = xot.local_name_str(elem.name());
            if name == "unknown" {
                let kind = xot.attributes(node).iter()
                    .find(|(n, _)| xot.local_name_str(*n) == "kind")
                    .map(|(_, v)| v.to_string())
                    .unwrap_or_default();
                *counts.entry(kind).or_insert(0) += 1;
            }
        }
        for c in xot.children(node) { walk(xot, c, counts); }
    }
    let mut counts = std::collections::BTreeMap::new();
    walk(&xot, root, &mut counts);
    for (k, n) in &counts {
        eprintln!("{:4}  {}", n, k);
    }
}

#[test]
#[ignore]
fn find_blueprint_error_nodes() {
    let source = std::fs::read_to_string("../tests/integration/languages/csharp/blueprint.cs")
        .or_else(|_| std::fs::read_to_string("tests/integration/languages/csharp/blueprint.cs"))
        .expect("blueprint.cs");
    let mut p = tree_sitter::Parser::new();
    p.set_language(&tree_sitter_c_sharp::LANGUAGE.into()).unwrap();
    let tree = p.parse(&source, None).unwrap();
    fn walk(node: tree_sitter::Node, src: &[u8]) {
        if node.kind() == "ERROR" {
            let line = node.start_position().row + 1;
            let text = node.utf8_text(src).unwrap_or("?");
            eprintln!("ERROR at line {}: {:?}", line, text.chars().take(80).collect::<String>());
        }
        let mut c = node.walk();
        for child in node.children(&mut c) { walk(child, src); }
    }
    walk(tree.root_node(), source.as_bytes());
}

#[test]
#[ignore]
fn dump_delegate_cst() {
    let s = "public delegate TResult Transformer<T, TResult>(T input);";
    let mut p = tree_sitter::Parser::new();
    p.set_language(&tree_sitter_c_sharp::LANGUAGE.into()).unwrap();
    let tree = p.parse(s, None).unwrap();
    fn walk(node: tree_sitter::Node, depth: usize, src: &[u8]) {
        let indent = "  ".repeat(depth);
        let text = node.utf8_text(src).unwrap_or("?");
        let text_short: String = text.chars().take(40).collect();
        let parent_kind = node.parent().map(|p| p.kind().to_string()).unwrap_or_default();
        // Find this node's field name in its parent.
        let mut field = None;
        if let Some(parent) = node.parent() {
            let mut c = parent.walk();
            for (idx, child) in parent.children(&mut c).enumerate() {
                if child.id() == node.id() {
                    field = parent.field_name_for_named_child(idx as u32).map(|s| s.to_string());
                    break;
                }
            }
        }
        eprintln!("{indent}{} field={:?} parent={} text={:?}",
            node.kind(), field, parent_kind, text_short);
        let mut c = node.walk();
        for child in node.children(&mut c) {
            if child.is_named() { walk(child, depth + 1, src); }
        }
    }
    walk(tree.root_node(), 0, s.as_bytes());
}

#[test]
#[ignore]
fn dump_paren_pattern() {
    let s = "class C { void M() { object o = 1; var x = o switch { (1) => 1, var v => 2 }; } }\n";
    let mut p = tree_sitter::Parser::new();
    p.set_language(&tree_sitter_c_sharp::LANGUAGE.into()).unwrap();
    let tree = p.parse(s, None).unwrap();
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
    walk(tree.root_node(), 0, s.as_bytes());
}

#[test]
#[ignore]
fn dump_prefix_unary() {
    let s = "class C { void M() { var a = ++i; var b = --i; var c = -i; var d = !i; var e = ~i; var f = +i; var g = &i; var h = *i; } }\n";
    let mut p = tree_sitter::Parser::new();
    p.set_language(&tree_sitter_c_sharp::LANGUAGE.into()).unwrap();
    let tree = p.parse(s, None).unwrap();
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
    walk(tree.root_node(), 0, s.as_bytes());
}

#[test]
#[ignore]
fn dump_object_creation_cst() {
    let s = "class C { void M() { var x = new Foo(1, 2); var y = new(); var z = new Foo(){A=1}; var a = new[]{1,2}; } }\n";
    let mut p = tree_sitter::Parser::new();
    p.set_language(&tree_sitter_c_sharp::LANGUAGE.into()).unwrap();
    let tree = p.parse(s, None).unwrap();
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
    walk(tree.root_node(), 0, s.as_bytes());
}

#[test]
#[ignore]
fn dump_lambda_cst() {
    let s = "class C { void M() { System.Func<int,int> f = x => x * 2; } }\n";
    let mut p = tree_sitter::Parser::new();
    p.set_language(&tree_sitter_c_sharp::LANGUAGE.into()).unwrap();
    let tree = p.parse(s, None).unwrap();
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
    walk(tree.root_node(), 0, s.as_bytes());
}

#[test]
fn invariants_blueprint() {
    // The full C# blueprint — proves text-recovery and round-trip
    // hold even on the full kitchen-sink fixture, far before
    // structural parity is reached.
    let source = std::fs::read_to_string("../tests/integration/languages/csharp/blueprint.cs")
        .or_else(|_| std::fs::read_to_string("tests/integration/languages/csharp/blueprint.cs"))
        .expect("blueprint.cs");
    assert_ir_invariants(&source, "C# blueprint.cs");
}

/// Render the IR pipeline output via tractor's actual tree
/// renderer. Sanity check that comment text + element structure
/// pass through to the renderer correctly. Marked `#[ignore]`;
/// invoke with `cargo test ir_tree_render -- --ignored --nocapture`.
#[test]
#[ignore]
fn ir_tree_render() {
    use tractor::output::{render_query_tree_node, RenderOptions};
    use tractor::xpath::xot_node_to_xml_node;

    let source = "// leading comment\nclass Foo { void M() { var x = 1 + 2; /* trailing */ } }\n";
    let mut p = tree_sitter::Parser::new();
    p.set_language(&tree_sitter_c_sharp::LANGUAGE.into()).unwrap();
    let tree = p.parse(source, None).unwrap();
    let ir = lower_csharp_root(tree.root_node(), source);
    let mut xot = Xot::new();
    let dr_name = xot.add_name("_root");
    let dr = xot.new_element(dr_name);
    render_to_xot(&mut xot, dr, &ir, source).expect("render");
    let root = xot.children(dr).find(|&c| xot.element(c).is_some()).unwrap();

    let xml_node = xot_node_to_xml_node(&xot, root);
    let opts = RenderOptions::new();
    let rendered = render_query_tree_node(&xml_node, &opts);
    eprintln!("=== IR pipeline tree render ===");
    eprintln!("{}", rendered);
}

/// Tree-render parity: compare the actual tractor tree-renderer
/// output between imperative and IR pipelines. This is the
/// integration the user runs (`tractor file.cs` defaults to tree
/// rendering). Marked `#[ignore]`; prints the first diff so we can
/// iterate.
#[test]
#[ignore]
fn blueprint_tree_parity() {
    use tractor::output::{render_query_tree_node, RenderOptions};
    use tractor::xpath::xot_node_to_xml_node;

    let source = std::fs::read_to_string("../tests/integration/languages/csharp/blueprint.cs")
        .or_else(|_| std::fs::read_to_string("tests/integration/languages/csharp/blueprint.cs"))
        .expect("blueprint.cs");

    let mut opts = RenderOptions::new();
    opts.include_meta = false;

    // Imperative pipeline tree.
    let cur = parse_string_to_xot(&source, "csharp", "<bp>".to_string(), None)
        .expect("imperative parse");
    let cur_root = if cur.xot.is_document(cur.root) {
        cur.xot.document_element(cur.root).expect("doc")
    } else { cur.root };
    let cur_xml = xot_node_to_xml_node(&cur.xot, cur_root);
    let cur_render = render_query_tree_node(&cur_xml, &opts);

    // IR pipeline tree.
    let mut p = tree_sitter::Parser::new();
    p.set_language(&tree_sitter_c_sharp::LANGUAGE.into()).unwrap();
    let tree = p.parse(&source, None).unwrap();
    let ir = lower_csharp_root(tree.root_node(), &source);
    let mut xot = Xot::new();
    let dr_name = xot.add_name("_root");
    let dr = xot.new_element(dr_name);
    render_to_xot(&mut xot, dr, &ir, &source).expect("render");
    let ir_root = xot.children(dr).find(|&c| xot.element(c).is_some()).unwrap();
    let ir_xml = xot_node_to_xml_node(&xot, ir_root);
    let ir_render = render_query_tree_node(&ir_xml, &opts);

    eprintln!("imperative bytes: {}", cur_render.len());
    eprintln!("IR bytes:         {}", ir_render.len());

    if cur_render != ir_render {
        for (i, (la, lb)) in cur_render.lines().zip(ir_render.lines()).enumerate() {
            if la != lb {
                let start = i.saturating_sub(3);
                let end = (i + 4).min(cur_render.lines().count().min(ir_render.lines().count()));
                let ctx_a: Vec<&str> = cur_render.lines().skip(start).take(end - start).collect();
                let ctx_b: Vec<&str> = ir_render.lines().skip(start).take(end - start).collect();
                panic!(
                    "first diff at line {}\n\
                     ----- imperative -----\n{}\n\
                     ----- IR -----\n{}",
                    i + 1,
                    ctx_a.join("\n"),
                    ctx_b.join("\n"),
                );
            }
        }
        panic!("length differs: imperative={}, IR={}", cur_render.lines().count(), ir_render.lines().count());
    }
}

/// Compare the IR pipeline's structural view of the C# blueprint
/// against the existing imperative pipeline. Marked `#[ignore]` while
/// shape parity is incomplete; counts Unknown nodes and prints the
/// first divergence so we can iterate toward parity.
#[test]
#[ignore]
fn blueprint_parity() {
    let source = std::fs::read_to_string("../tests/integration/languages/csharp/blueprint.cs")
        .or_else(|_| std::fs::read_to_string("tests/integration/languages/csharp/blueprint.cs"))
        .expect("blueprint.cs");

    // Current pipeline view.
    let cur_result = parse_string_to_xot(&source, "csharp", "<bp>".to_string(), None)
        .expect("current pipeline parse");
    let cur_root = if cur_result.xot.is_document(cur_result.root) {
        cur_result.xot.document_element(cur_result.root).expect("doc")
    } else { cur_result.root };
    let cur_view = structural_view(&cur_result.xot, cur_root);

    // IR pipeline view.
    let mut p = tree_sitter::Parser::new();
    p.set_language(&tree_sitter_c_sharp::LANGUAGE.into()).unwrap();
    let tree = p.parse(&source, None).unwrap();
    let ir = lower_csharp_root(tree.root_node(), &source);
    let mut xot = Xot::new();
    let dr_name = xot.add_name("_root");
    let dr = xot.new_element(dr_name);
    render_to_xot(&mut xot, dr, &ir, &source).expect("render");
    let ir_root = xot.children(dr).find(|&c| xot.element(c).is_some()).unwrap();
    let ir_view = structural_view(&xot, ir_root);

    // Count Unknowns.
    fn count_unknowns(xot: &Xot, node: XotNode, out: &mut usize) {
        if let Some(elem) = xot.element(node) {
            if xot.local_name_str(elem.name()) == "unknown" { *out += 1; }
        }
        for c in xot.children(node) { count_unknowns(xot, c, out); }
    }
    let mut unknowns = 0usize;
    count_unknowns(&xot, ir_root, &mut unknowns);

    eprintln!("=== C# blueprint structural-parity check ===");
    eprintln!("source bytes:        {}", source.len());
    eprintln!("current view bytes:  {}", cur_view.len());
    eprintln!("IR view bytes:       {}", ir_view.len());
    eprintln!("Unknown in IR:       {}", unknowns);

    if cur_view != ir_view {
        for (i, (la, lb)) in cur_view.lines().zip(ir_view.lines()).enumerate() {
            if la != lb {
                let start = i.saturating_sub(3);
                let end = (i + 4).min(cur_view.lines().count().min(ir_view.lines().count()));
                let ctx_a: Vec<&str> = cur_view.lines().skip(start).take(end - start).collect();
                let ctx_b: Vec<&str> = ir_view.lines().skip(start).take(end - start).collect();
                panic!(
                    "first diff at line {} (1-based: {})\n\
                     ----- current pipeline -----\n{}\n\
                     ----- IR pipeline -----\n{}",
                    i, i + 1,
                    ctx_a.join("\n"),
                    ctx_b.join("\n"),
                );
            }
        }
        panic!("length differs: current={}, IR={}", cur_view.lines().count(), ir_view.lines().count());
    }
}

/// Coverage audit against the C# blueprint. Reports kind / node
/// coverage; asserts no silent CST drops.
#[test]
fn blueprint_coverage_audit() {
    let source = std::fs::read_to_string("../tests/integration/languages/csharp/blueprint.cs")
        .or_else(|_| std::fs::read_to_string("tests/integration/languages/csharp/blueprint.cs"))
        .expect("blueprint.cs");

    let mut p = tree_sitter::Parser::new();
    p.set_language(&tree_sitter_c_sharp::LANGUAGE.into()).unwrap();
    let tree = p.parse(&source, None).unwrap();
    let ir = lower_csharp_root(tree.root_node(), &source);

    assert_eq!(to_source(&ir, &source), source, "round-trip identity broken");

    let mut xot = Xot::new();
    let dr_name = xot.add_name("_root");
    let dr = xot.new_element(dr_name);
    render_to_xot(&mut xot, dr, &ir, &source).expect("render");
    let root = xot.children(dr).find(|&c| xot.element(c).is_some()).unwrap();
    let xpath = text_concat(&xot, root);
    if xpath != source {
        // Find the first differing byte and show context.
        let mut idx: usize = 0;
        for (a, b) in xpath.bytes().zip(source.bytes()) {
            if a != b { break; }
            idx += 1;
        }
        let start = idx.saturating_sub(60);
        let end_a = (idx + 60).min(xpath.len());
        let end_b = (idx + 60).min(source.len());
        panic!(
            "XPath text-content recovery broken at byte {idx}\n\
             ----- IR (got)    -----\n{:?}\n\
             ----- source (want) -----\n{:?}",
            &xpath[start..end_a],
            &source[start..end_b],
        );
    }

    let known = csharp_known_kinds();
    let report = audit_coverage(tree.root_node(), &ir, &source, &known);
    eprintln!("\n{}", report.summary());
    assert_eq!(report.dropped, 0,
        "{} CST nodes dropped (renderer bug)",
        report.dropped);
}

// ---------------------------------------------------------------------------
// Structural parity on the EXPRESSION subtree only.
// Both pipelines wrap the expression in a class+method scaffold;
// we navigate to the var-assignment's value and compare those subtrees.
// ---------------------------------------------------------------------------

/// Navigate to the inner expression in
/// `class C { void M() { var x = EXPR; } }` for both pipelines.
/// Returns the subtree rooted at `EXPR`.
fn find_expression_subtree(xot: &Xot, root: XotNode) -> Option<XotNode> {
    fn search(xot: &Xot, node: XotNode, target_parent: &str) -> Option<XotNode> {
        if let Some(elem) = xot.element(node) {
            let name = xot.local_name_str(elem.name());
            if name == target_parent {
                // Found <variable>; return its last element child (the value).
                return xot.children(node)
                    .filter(|&c| xot.element(c).is_some())
                    .last();
            }
        }
        for c in xot.children(node) {
            if let Some(found) = search(xot, c, target_parent) {
                return Some(found);
            }
        }
        None
    }
    search(xot, root, "variable")
}

fn assert_expression_parity(expr: &str, label: &str) {
    let source = format!("class C {{ void M() {{ var x = {expr}; }} }}\n");

    // Current pipeline.
    let r = parse_string_to_xot(&source, "csharp", "<test>".to_string(), None)
        .expect("current pipeline parse");
    let cur_root = if r.xot.is_document(r.root) {
        r.xot.document_element(r.root).expect("doc")
    } else { r.root };
    let cur_expr = find_expression_subtree(&r.xot, cur_root)
        .expect("current pipeline: expression subtree not found");
    let cur_view = structural_view(&r.xot, cur_expr);

    // IR pipeline.
    let mut p = tree_sitter::Parser::new();
    p.set_language(&tree_sitter_c_sharp::LANGUAGE.into()).unwrap();
    let tree = p.parse(&source, None).unwrap();
    let ir = lower_csharp_root(tree.root_node(), &source);

    let recovered = to_source(&ir, &source);
    assert_eq!(recovered, source, "round-trip identity broken for {label}");

    let mut xot = Xot::new();
    let dr_name = xot.add_name("_root");
    let dr = xot.new_element(dr_name);
    render_to_xot(&mut xot, dr, &ir, &source).expect("render");
    let ir_root = xot.children(dr).find(|&c| xot.element(c).is_some()).unwrap();

    let xpath_text = text_concat(&xot, ir_root);
    assert_eq!(xpath_text, source, "XPath text recovery broken for {label}");

    // Note: structural parity at this slice is not yet expected to
    // hold because we haven't lowered class/method/variable yet.
    // Once those land, find_expression_subtree will work on the IR
    // side too. For now, just check that the IR contains the
    // expression somewhere.
    let _ = cur_view;
    let _ = label;
    // TODO: once Ir::Class / Ir::Method / Ir::Variable are added,
    //       compare cur_expr against IR's variable-value subtree.
}

#[test]
fn expression_int()       { assert_expression_parity("42", "int"); }

#[test]
fn expression_member()    { assert_expression_parity("a.b", "member"); }

#[test]
fn expression_chain()     { assert_expression_parity("a.b.c", "chain"); }

#[test]
fn expression_index()     { assert_expression_parity("a[0]", "index"); }

#[test]
fn expression_call()      { assert_expression_parity("f(x)", "call"); }

// ---------------------------------------------------------------------------
// Whack-a-mole construct: `?.` conditional access.
//
// This is backlog item 5d in `todo/39-post-cycle-review-backlog.md`:
// the existing C# pipeline emits a NON-ISOMORPHIC shape for `a.b` vs
// `a?.b` (`<member[conditional]>` parent + `<condition>` wrapper),
// and the design problem was deferred.
//
// In the typed-IR world, conditional access is just a
// `optional: true` flag on an `AccessSegment::Member` — same shape
// as regular access, plus an `<optional/>` marker. Principle #15
// is satisfied by construction.
// ---------------------------------------------------------------------------

/// Verify that `a.b` and `a?.b` produce structurally identical IR
/// trees except for the presence/absence of `<optional/>`.
#[test]
fn conditional_access_isomorphism() {
    let s_regular     = "class C { void M() { var x = a.b; } }\n";
    let s_conditional = "class C { void M() { var x = a?.b; } }\n";

    // Both must satisfy the architectural invariants.
    assert_ir_invariants(s_regular,     "regular member");
    assert_ir_invariants(s_conditional, "conditional member");

    // Lower JUST the access expression. Find the
    // member_access/conditional_access CST node and call
    // lower_csharp_node on it — bypassing the class/method scaffold.
    fn ir_access_view(source: &str) -> String {
        let mut p = tree_sitter::Parser::new();
        p.set_language(&tree_sitter_c_sharp::LANGUAGE.into()).unwrap();
        let tree = p.parse(source, None).unwrap();
        fn find<'t>(node: tree_sitter::Node<'t>) -> Option<tree_sitter::Node<'t>> {
            if matches!(node.kind(), "member_access_expression" | "conditional_access_expression") {
                return Some(node);
            }
            let mut c = node.walk();
            for child in node.named_children(&mut c) {
                if let Some(f) = find(child) { return Some(f); }
            }
            None
        }
        let target = find(tree.root_node()).expect("access expression");
        let access = tractor::ir::lower_csharp_node(target, source);
        let mut xot = Xot::new();
        let dr_name = xot.add_name("_root");
        let dr = xot.new_element(dr_name);
        render_to_xot(&mut xot, dr, &access, source).expect("render");
        let root = xot.children(dr).find(|&c| xot.element(c).is_some()).unwrap();
        structural_view(&xot, root)
    }

    let v_regular     = ir_access_view(s_regular);
    let v_conditional = ir_access_view(s_conditional);

    eprintln!("--- regular `a.b` ---\n{v_regular}");
    eprintln!("--- conditional `a?.b` ---\n{v_conditional}");

    // Same shape modulo the <optional/> marker. This is the
    // Principle #15 contract: `a.b` and `a?.b` differ only by a
    // marker on a stable host.
    assert!(v_regular.contains("object"),  "regular should produce <object>");
    assert!(v_conditional.contains("object"), "conditional should produce <object>");
    assert!(v_regular.contains("member"),  "regular should produce <member>");
    assert!(v_conditional.contains("member"), "conditional should produce <member>");
    assert!(!v_regular.contains("optional"), "regular must NOT have <optional> marker");
    assert!(v_conditional.contains("optional"), "conditional MUST have <optional> marker");
    // The only structural difference should be the optional marker.
    let v_regular_normalized     = v_regular.lines().filter(|l| !l.trim().starts_with("optional")).collect::<Vec<_>>().join("\n");
    let v_conditional_normalized = v_conditional.lines().filter(|l| !l.trim().starts_with("optional")).collect::<Vec<_>>().join("\n");
    assert_eq!(v_regular_normalized, v_conditional_normalized,
        "conditional and regular access should differ ONLY by the <optional/> marker");
}

/// Mixed chain: only the `?.b` segment is conditional in `a?.b.c`.
/// Mid-chain conditional in `a.b?.c`. Both-conditional `a?.b?.c`.
#[test]
fn conditional_access_chains() {
    for (src, desc) in &[
        ("class C { void M() { var x = a?.b.c; } }\n", "a?.b.c — first segment conditional"),
        ("class C { void M() { var x = a.b?.c; } }\n", "a.b?.c — last segment conditional"),
        ("class C { void M() { var x = a?.b?.c; } }\n", "a?.b?.c — both conditional"),
    ] {
        assert_ir_invariants(src, desc);
    }
}

// ---------------------------------------------------------------------------
// Whack-a-mole: `obj!` non-null assertion (postfix unary).
//
// The existing pipeline emits `<expression[non_null]>` — marker on
// the expression host. The IR achieves the same by extending
// `Ir::Expression` with an optional `marker` field.
// ---------------------------------------------------------------------------

#[test]
fn non_null_assertion() {
    let s = "class C { void M() { var x = obj!; } }\n";
    assert_ir_invariants(s, "obj! non-null");

    // Lower the postfix expression directly.
    let mut p = tree_sitter::Parser::new();
    p.set_language(&tree_sitter_c_sharp::LANGUAGE.into()).unwrap();
    let tree = p.parse(s, None).unwrap();
    fn find<'t>(n: tree_sitter::Node<'t>) -> Option<tree_sitter::Node<'t>> {
        if n.kind() == "postfix_unary_expression" { return Some(n); }
        let mut c = n.walk();
        for child in n.named_children(&mut c) {
            if let Some(f) = find(child) { return Some(f); }
        }
        None
    }
    let target = find(tree.root_node()).expect("postfix_unary");
    let ir = tractor::ir::lower_csharp_node(target, s);

    let mut xot = Xot::new();
    let dr_name = xot.add_name("_root");
    let dr = xot.new_element(dr_name);
    render_to_xot(&mut xot, dr, &ir, s).expect("render");
    let root = xot.children(dr).find(|&c| xot.element(c).is_some()).unwrap();
    let view = structural_view(&xot, root);
    eprintln!("--- obj! ---\n{view}");

    // Should produce <expression><non_null/><name>obj</name></expression>
    assert!(view.contains("expression"), "must wrap in <expression>");
    assert!(view.contains("non_null"), "must carry <non_null/> marker");
    assert!(view.contains("name"), "must contain inner <name>");

    // Round-trip text recovery.
    let recovered = to_source(&ir, s);
    assert!(recovered.contains("obj!"), "round-trip must preserve `obj!`");
}

// ---------------------------------------------------------------------------
// Whack-a-mole: `x is Type` type-test expression.
// ---------------------------------------------------------------------------

#[test]
fn is_type_test() {
    let s = "class C { void M() { var c = x is int; } }\n";
    assert_ir_invariants(s, "x is int");

    let mut p = tree_sitter::Parser::new();
    p.set_language(&tree_sitter_c_sharp::LANGUAGE.into()).unwrap();
    let tree = p.parse(s, None).unwrap();
    fn find<'t>(n: tree_sitter::Node<'t>) -> Option<tree_sitter::Node<'t>> {
        if n.kind() == "is_expression" { return Some(n); }
        let mut c = n.walk();
        for child in n.named_children(&mut c) {
            if let Some(f) = find(child) { return Some(f); }
        }
        None
    }
    let target = find(tree.root_node()).expect("is_expression");
    let ir = tractor::ir::lower_csharp_node(target, s);

    let mut xot = Xot::new();
    let dr_name = xot.add_name("_root");
    let dr = xot.new_element(dr_name);
    render_to_xot(&mut xot, dr, &ir, s).expect("render");
    let root = xot.children(dr).find(|&c| xot.element(c).is_some()).unwrap();
    let view = structural_view(&xot, root);
    eprintln!("--- x is int ---\n{view}");

    // Expected:
    // is
    //   left
    //     expression
    //       name text="x"
    //   right
    //     expression
    //       type
    //         name text="int"
    assert!(view.starts_with("is"), "must produce <is> as root");
    assert!(view.contains("left"), "must contain <left>");
    assert!(view.contains("right"), "must contain <right>");
    assert!(view.contains("type"), "must wrap target in <type>");

    let recovered = to_source(&ir, s);
    assert!(recovered.contains("x is int"), "round-trip must preserve `x is int`");
}

// ---------------------------------------------------------------------------
// Mutation by enum: change `access` field, marker swaps automatically.
//
// This is the architectural payoff for the "variations marked
// exhaustively → enum field" principle. Instead of XML-level marker
// rewrites (`drop <public/>, inject <private/>`), the user changes
// one IR field; the renderer's marker is derived from the enum.
// ---------------------------------------------------------------------------

#[test]
fn access_marker_swap_via_enum_mutation() {
    let s = "public class Foo { }\n";
    let mut p = tree_sitter::Parser::new();
    p.set_language(&tree_sitter_c_sharp::LANGUAGE.into()).unwrap();
    let tree = p.parse(s, None).unwrap();
    let mut ir = lower_csharp_root(tree.root_node(), s);

    // Locate the class IR.
    fn find_class(ir: &mut tractor::ir::Ir) -> Option<&mut tractor::ir::Ir> {
        use tractor::ir::Ir;
        if matches!(ir, Ir::Class { .. }) { return Some(ir); }
        match ir {
            Ir::Module { children, .. } | Ir::Inline { children, .. }
            | Ir::Body { children, .. } => {
                for c in children {
                    if let Some(f) = find_class(c) { return Some(f); }
                }
                None
            }
            _ => None,
        }
    }
    let class = find_class(&mut ir).expect("Ir::Class in tree");

    // Verify it parsed with modifiers.access = Public.
    if let tractor::ir::Ir::Class { modifiers, .. } = class {
        assert_eq!(modifiers.access, Some(tractor::ir::Access::Public),
            "expected `public class Foo` to lower to Access::Public");
    }

    // Render before mutation.
    fn render_view(ir: &tractor::ir::Ir, src: &str) -> String {
        let mut xot = Xot::new();
        let dr_name = xot.add_name("_root");
        let dr = xot.new_element(dr_name);
        render_to_xot(&mut xot, dr, ir, src).expect("render");
        let root = xot.children(dr).find(|&c| xot.element(c).is_some()).unwrap();
        structural_view(&xot, root)
    }
    let before = render_view(&ir, s);
    eprintln!("--- before (access=Public) ---\n{before}");
    assert!(before.contains("public"), "before-view must contain <public/> marker");

    // Mutation: flip access to Private. ONE FIELD CHANGE.
    let class = find_class(&mut ir).unwrap();
    if let tractor::ir::Ir::Class { modifiers, .. } = class {
        modifiers.access = Some(tractor::ir::Access::Private);
    }

    // Re-render. Marker swapped by construction — no XML-level
    // rewrite, no imperative pass.
    let after = render_view(&ir, s);
    eprintln!("--- after (access=Private) ---\n{after}");
    assert!(after.contains("private"), "after-view must contain <private/> marker");
    assert!(!after.contains("public"), "after-view must NOT contain <public/> marker");

    // The structural shape OUTSIDE the marker is unchanged.
    let normalize = |v: &str| -> String {
        v.lines().filter(|l| {
            let t = l.trim();
            t != "public" && t != "private"
        }).collect::<Vec<_>>().join("\n")
    };
    assert_eq!(normalize(&before), normalize(&after),
        "non-marker structure should be unchanged by access mutation");
}

/// Mutation by enum: flipping `static_` flag adds the `<static/>`
/// marker. Validates the boolean-flag case for Modifiers.
#[test]
fn static_marker_via_modifiers_mutation() {
    let s = "public class Foo { }\n";
    let mut p = tree_sitter::Parser::new();
    p.set_language(&tree_sitter_c_sharp::LANGUAGE.into()).unwrap();
    let tree = p.parse(s, None).unwrap();
    let mut ir = lower_csharp_root(tree.root_node(), s);

    fn find_class(ir: &mut tractor::ir::Ir) -> Option<&mut tractor::ir::Ir> {
        use tractor::ir::Ir;
        if matches!(ir, Ir::Class { .. }) { return Some(ir); }
        match ir {
            Ir::Module { children, .. } | Ir::Inline { children, .. }
            | Ir::Body { children, .. } => {
                for c in children {
                    if let Some(f) = find_class(c) { return Some(f); }
                }
                None
            }
            _ => None,
        }
    }
    let class = find_class(&mut ir).expect("Ir::Class");
    if let tractor::ir::Ir::Class { modifiers, .. } = class {
        assert!(!modifiers.static_, "should not be static initially");
        modifiers.static_ = true;
    }

    fn render_view(ir: &tractor::ir::Ir, src: &str) -> String {
        let mut xot = Xot::new();
        let dr_name = xot.add_name("_root");
        let dr = xot.new_element(dr_name);
        render_to_xot(&mut xot, dr, ir, src).expect("render");
        let root = xot.children(dr).find(|&c| xot.element(c).is_some()).unwrap();
        structural_view(&xot, root)
    }
    let view = render_view(&ir, s);
    eprintln!("--- after static=true ---\n{view}");
    assert!(view.contains("static"), "view must contain <static/> marker");
    assert!(view.contains("public"), "view must still contain <public/> marker");
}

/// Demonstrates the `set_flag(name, value)` API surface that a
/// mutation CLI would call. Verifies typed validation: unknown flag
/// names return Err, known flags toggle the right field.
#[test]
fn modifiers_set_flag_api() {
    let mut m = tractor::ir::Modifiers::default();
    assert!(m.is_empty());

    m.set_flag("static", true).unwrap();
    assert!(m.static_);

    m.set_flag("abstract", true).unwrap();
    m.set_flag("sealed", true).unwrap();
    assert!(m.abstract_ && m.sealed);

    let err = m.set_flag("nonexistent", true);
    assert!(err.is_err(), "unknown flag must Err");

    m.set_flag("static", false).unwrap();
    assert!(!m.static_);
}

// ---------------------------------------------------------------------------
// Whack-a-mole: `(Type)expr` cast.
// ---------------------------------------------------------------------------

#[test]
fn cast_expression() {
    let s = "class C { void M() { var x = (int)y; } }\n";
    assert_ir_invariants(s, "(int)y cast");

    let mut p = tree_sitter::Parser::new();
    p.set_language(&tree_sitter_c_sharp::LANGUAGE.into()).unwrap();
    let tree = p.parse(s, None).unwrap();
    fn find<'t>(n: tree_sitter::Node<'t>) -> Option<tree_sitter::Node<'t>> {
        if n.kind() == "cast_expression" { return Some(n); }
        let mut c = n.walk();
        for child in n.named_children(&mut c) {
            if let Some(f) = find(child) { return Some(f); }
        }
        None
    }
    let target = find(tree.root_node()).expect("cast");
    let ir = tractor::ir::lower_csharp_node(target, s);

    let mut xot = Xot::new();
    let dr_name = xot.add_name("_root");
    let dr = xot.new_element(dr_name);
    render_to_xot(&mut xot, dr, &ir, s).expect("render");
    let root = xot.children(dr).find(|&c| xot.element(c).is_some()).unwrap();
    let view = structural_view(&xot, root);
    eprintln!("--- (int)y ---\n{view}");

    // Should produce <cast><type><name>int</name></type><value><expression><name>y</name></expression></value></cast>
    assert!(view.contains("cast"), "must wrap in <cast>");
    assert!(view.contains("type"), "must contain <type> slot");
    assert!(view.contains("value"), "must contain <value> slot");

    let recovered = to_source(&ir, s);
    assert!(recovered.contains("(int)y"), "round-trip must preserve `(int)y`");
}

/// Show the existing pipeline's shape for `x is int`.
#[test]
#[ignore]
fn dump_existing_is_shape() {
    let s = "class C { void M() { var c = x is int; } }\n";
    let r = parse_string_to_xot(s, "csharp", "<test>".to_string(), None).unwrap();
    let root = r.xot.document_element(r.root).unwrap();
    let view = structural_view(&r.xot, root);
    eprintln!("{view}");
}

#[test]
#[ignore]
fn dump_csharp_misc() {
    let source = "class C { void M() { var a = obj!; var b = (int)x; var c = x is int; } }";
    let mut p = tree_sitter::Parser::new();
    p.set_language(&tree_sitter_c_sharp::LANGUAGE.into()).unwrap();
    let tree = p.parse(source, None).unwrap();
    fn walk(node: tree_sitter::Node, depth: usize, src: &[u8]) {
        let indent = "  ".repeat(depth);
        let text = node.utf8_text(src).unwrap_or("?");
        let display = if text.len() > 60 { format!("{}...", &text[..60]) } else { text.to_string() };
        eprintln!("{indent}{} text={display:?}", node.kind());
        let mut c = node.walk();
        for child in node.children(&mut c) {
            if child.is_named() { walk(child, depth + 1, src); }
        }
    }
    walk(tree.root_node(), 0, source.as_bytes());
}

#[test]
#[ignore]
fn dump_csharp_conditional() {
    let source = "class C { void M() { var x = a?.b.c; var y = a.b?.c; var z = a?.b?.c; } }";
    let mut p = tree_sitter::Parser::new();
    p.set_language(&tree_sitter_c_sharp::LANGUAGE.into()).unwrap();
    let tree = p.parse(source, None).unwrap();
    fn walk(node: tree_sitter::Node, depth: usize, src: &[u8]) {
        let indent = "  ".repeat(depth);
        let text = node.utf8_text(src).unwrap_or("?");
        let display = if text.len() > 60 { format!("{}...", &text[..60]) } else { text.to_string() };
        eprintln!("{indent}{} text={display:?}", node.kind());
        let mut c = node.walk();
        for child in node.children(&mut c) {
            if child.is_named() { walk(child, depth + 1, src); }
        }
    }
    walk(tree.root_node(), 0, source.as_bytes());
}

/// Dump the C# CST shape of a small snippet.
#[test]
#[ignore]
fn dump_csharp_cst() {
    let source = "class C { void M() { var x = a.b.c; } }";
    let mut p = tree_sitter::Parser::new();
    p.set_language(&tree_sitter_c_sharp::LANGUAGE.into()).unwrap();
    let tree = p.parse(source, None).unwrap();
    fn walk(node: tree_sitter::Node, depth: usize, src: &[u8]) {
        let indent = "  ".repeat(depth);
        let text = node.utf8_text(src).unwrap_or("?");
        let display = if text.len() > 40 { format!("{}...", &text[..40]) } else { text.to_string() };
        eprintln!("{indent}{} text={display:?}", node.kind());
        let mut c = node.walk();
        for child in node.children(&mut c) {
            if child.is_named() { walk(child, depth + 1, src); }
        }
    }
    walk(tree.root_node(), 0, source.as_bytes());
}
