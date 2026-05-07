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
fn dump_java_generic_bound_render() {
    let s = "class Dog<T extends Animal> extends Animal implements Barker, Runner { int a; double b; T owner; java.util.List<String> tags; public void bark() {} public void run() {} }";
    let parsed = tractor::parser::parse_string_to_xot(s, "java", "<x>".to_string(), None).expect("parse");
    let root = if parsed.xot.is_document(parsed.root) {
        parsed.xot.document_element(parsed.root).expect("doc")
    } else { parsed.root };
    println!("{}", parsed.xot.to_string(root).unwrap());
}

#[test]
#[ignore]
fn dump_java_generic_bound_cst() {
    let s = "class Dog<T extends Animal> { }";
    let mut p = tree_sitter::Parser::new();
    p.set_language(&tree_sitter_java::LANGUAGE.into()).unwrap();
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
fn dump_java_type_pattern_cst() {
    let s = "class T { String f(Object o) { return switch (o) { case Integer i -> \"int\"; default -> \"other\"; }; } }";
    let mut p = tree_sitter::Parser::new();
    p.set_language(&tree_sitter_java::LANGUAGE.into()).unwrap();
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
fn dump_java_type_pattern_ir_only() {
    let s = "class T { String f(Object o) { return switch (o) { case Integer i -> \"int\"; default -> \"other\"; }; } }";
    let mut p = tree_sitter::Parser::new();
    p.set_language(&tree_sitter_java::LANGUAGE.into()).unwrap();
    let tree = p.parse(s, None).unwrap();
    let ir = tractor::ir::lower_java_root(tree.root_node(), s);
    eprintln!("{:#?}", ir);
}

#[test]
#[ignore]
fn dump_java_type_pattern_inner_cst() {
    let s = "case Integer i -> 1;";
    let mut p = tree_sitter::Parser::new();
    p.set_language(&tree_sitter_java::LANGUAGE.into()).unwrap();
    let _ = p.parse(s, None).unwrap();
    let s2 = "class T { String f(Object o) { return switch (o) { case Integer i -> \"int\"; default -> \"other\"; }; } }";
    let tree = p.parse(s2, None).unwrap();
    fn walk(n: tree_sitter::Node, src: &[u8], depth: usize) {
        let indent = "  ".repeat(depth);
        let txt = n.utf8_text(src).unwrap_or("?");
        let short: String = txt.chars().take(40).collect();
        let mut field = None;
        if let Some(parent) = n.parent() {
            let mut c = parent.walk();
            for (i, ch) in parent.children(&mut c).enumerate() {
                if ch.id() == n.id() { field = parent.field_name_for_child(i as u32); break; }
            }
        }
        eprintln!("{indent}{}{} text={:?}", n.kind(), field.map(|f| format!(" [{f}]")).unwrap_or_default(), short);
        let mut c = n.walk();
        for ch in n.children(&mut c) { walk(ch, src, depth + 1); }
    }
    walk(tree.root_node(), s2.as_bytes(), 0);
}

#[test]
#[ignore]
fn dump_java_variadic_cst() {
    let s = "class T { void f(int... xs) { } }";
    let mut p = tree_sitter::Parser::new();
    p.set_language(&tree_sitter_java::LANGUAGE.into()).unwrap();
    let tree = p.parse(s, None).unwrap();
    fn walk(n: tree_sitter::Node, src: &[u8], depth: usize) {
        let indent = "  ".repeat(depth);
        let txt = n.utf8_text(src).unwrap_or("?");
        let short: String = txt.chars().take(40).collect();
        let mut field = None;
        if let Some(parent) = n.parent() {
            let mut c = parent.walk();
            for (i, ch) in parent.children(&mut c).enumerate() {
                if ch.id() == n.id() { field = parent.field_name_for_child(i as u32); break; }
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
fn dump_java_variadic_render() {
    let s = "class T { void f(int... xs) { } }";
    let parsed = tractor::parser::parse_string_to_xot(s, "java", "<x>".to_string(), None).expect("parse");
    let root = if parsed.xot.is_document(parsed.root) {
        parsed.xot.document_element(parsed.root).expect("doc")
    } else { parsed.root };
    println!("{}", parsed.xot.to_string(root).unwrap());
}

#[test]
#[ignore]
fn dump_java_type_pattern_render() {
    let s = "class T { String f(Object o) { return switch (o) { case Integer i -> \"int\"; default -> \"other\"; }; } }";
    let parsed = tractor::parser::parse_string_to_xot(s, "java", "<x>".to_string(), None).expect("parse");
    let root = if parsed.xot.is_document(parsed.root) {
        parsed.xot.document_element(parsed.root).expect("doc")
    } else { parsed.root };
    println!("{}", parsed.xot.to_string(root).unwrap());
}

#[test]
#[ignore]
fn dump_java_chain_render() {
    let s = "class X { void f() { obj.foo().bar.baz(); } }";
    let parsed = tractor::parser::parse_string_to_xot(s, "java", "<x>".to_string(), None).expect("parse");
    let root = if parsed.xot.is_document(parsed.root) {
        parsed.xot.document_element(parsed.root).expect("doc")
    } else { parsed.root };
    println!("{}", parsed.xot.to_string(root).unwrap());
}

#[test]
#[ignore]
fn dump_java_void_render() {
    let s = "class X { void f() {} int g() { return 0; } }";
    let parsed = tractor::parser::parse_string_to_xot(s, "java", "<x>".to_string(), None).expect("parse");
    let root = if parsed.xot.is_document(parsed.root) {
        parsed.xot.document_element(parsed.root).expect("doc")
    } else { parsed.root };
    println!("{}", parsed.xot.to_string(root).unwrap());
}

#[test]
#[ignore]
fn dump_java_super_method() {
    let s = "class X { Object f() { return super.toString(); } }";
    let mut p = tree_sitter::Parser::new();
    p.set_language(&tree_sitter_java::LANGUAGE.into()).unwrap();
    let tree = p.parse(s, None).unwrap();
    fn walk(n: tree_sitter::Node, src: &[u8], depth: usize) {
        let indent = "  ".repeat(depth);
        let txt = n.utf8_text(src).unwrap_or("?");
        let short: String = txt.chars().take(40).collect();
        let mut field = None;
        if let Some(parent) = n.parent() {
            let mut c = parent.walk();
            for (i, ch) in parent.children(&mut c).enumerate() {
                if ch.id() == n.id() { field = parent.field_name_for_child(i as u32); break; }
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
fn dump_java_modifiers() {
    let s = "public abstract static class M { public static final int X = 1; public synchronized void s() {} int pkg = 4; }";
    let parsed = tractor::parser::parse_string_to_xot(s, "java", "<x>".to_string(), None).expect("parse");
    let root = if parsed.xot.is_document(parsed.root) {
        parsed.xot.document_element(parsed.root).expect("doc")
    } else { parsed.root };
    println!("{}", parsed.xot.to_string(root).unwrap());
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
