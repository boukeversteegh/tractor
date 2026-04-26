//! Cross-language: decorator / annotation / attribute topology.
//!
//! The element name is idiomatic per language (Python uses <decorator>,
//! Java <annotation>, C#/PHP/Rust <attribute>) but the STRUCTURAL
//! TOPOLOGY is shared: the thing lives as a direct child of the
//! decorated/annotated declaration, with a <name> child holding the
//! qualifier name as text. No language uses an enclosing wrapper like
//! <decorated> or <attributes>.

use crate::support::semantic::*;

#[test]
fn python_decorator_is_direct_child() {
    let mut tree = parse_src("python", "@dataclass\nclass X: pass\n");
    assert_count(
        &mut tree,
        "//class/decorator[name='dataclass']",
        1,
        "Python decorator is a direct child of the decorated <class>",
    );
    assert_count(
        &mut tree,
        "//decorated",
        0,
        "no <decorated> wrapper — topology matches Java/C#/Rust",
    );
}

#[test]
fn java_annotation_is_direct_child() {
    let mut tree = parse_src(
        "java",
        "class X { @Override public void f() {} }",
    );
    assert_count(
        &mut tree,
        "//method/annotation[name='Override']",
        1,
        "Java annotation is a direct child of the annotated <method>",
    );
}

#[test]
fn csharp_attribute_is_direct_child() {
    let mut tree = parse_src(
        "csharp",
        "class X { [Obsolete] [MaxLength(50)] public string Name; }",
    );

    claim("C# attribute is a direct child of the attributed declaration",
        &mut tree, "//field/attribute[name='Obsolete']", 1);

    claim("attribute with arguments still has exactly one <name> child",
        &mut tree, "//attribute[name='MaxLength']/name", 1);

    claim("attribute <name> holds the identifier as text (no nested <name>)",
        &mut tree, "//attribute[name='MaxLength']/name/*", 0);
}

#[test]
fn rust_attribute_is_flat() {
    let mut tree = parse_src("rust", "#[derive(Debug)] struct S;\n");
    // #[derive] surfaces as a sibling `<attribute>` at the file
    // level — `attribute_item` wrapper was flattened.
    assert_count(
        &mut tree,
        "//attribute[name='derive']",
        1,
        "Rust attribute flattens: <attribute> with <name> child, not nested",
    );
    // Inner attributes (`#![…]`) carry an <inner/> marker to
    // distinguish from outer (`#[…]`) attributes.
    let mut inner = parse_src("rust", "#![allow(dead_code)]\nfn f() {}\n");
    assert_count(
        &mut inner,
        "//attribute[inner][name='allow']",
        1,
        "Rust inner attribute carries <inner/> marker",
    );
}

#[test]
fn php_attribute_is_direct_child() {
    let mut tree = parse_src(
        "php",
        "<?php #[Deprecated] class X {}\n",
    );
    assert_count(
        &mut tree,
        "//class/attribute[name='Deprecated']",
        1,
        "PHP attribute is a direct child of the attributed <class>",
    );
}
