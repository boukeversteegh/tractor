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
    claim("Python decorator is a direct child of the class it decorates",
        &mut tree,
        &multi_xpath(r#"
            //class/decorator
                [name='dataclass']
        "#),
        1);

    claim("no <decorated> wrapper — topology matches Java/C#/Rust",
        &mut tree, "//decorated", 0);
}

#[test]
fn java_annotation_is_direct_child() {
    claim("Java annotation is a direct child of the method it annotates",
        &mut parse_src("java", "class X { @Override public void f() {} }"),
        &multi_xpath(r#"
            //method/annotation
                [name='Override']
        "#),
        1);
}

#[test]
fn csharp_attribute_is_direct_child() {
    let mut tree = parse_src(
        "csharp",
        "class X { [Obsolete] [MaxLength(50)] public string Name; }",
    );

    claim("C# attribute name and argument shape is flat",
        &mut tree,
        &multi_xpath(r#"
            //attribute[name='MaxLength']
                [name]
                [argument/int='50']
        "#),
        1);

    claim("C# attributes are direct field children",
        &mut tree, "//field/attribute", 2);

    claim("attribute <name> holds the identifier as text (no nested <name>)",
        &mut tree, "//attribute[name='MaxLength']/name/*", 0);
}

#[test]
fn rust_attribute_is_flat() {
    // #[derive] surfaces as a sibling `<attribute>` at the file
    // level — `attribute_item` wrapper was flattened.
    claim("Rust outer attribute flattens to attribute with name",
        &mut parse_src("rust", "#[derive(Debug)] struct S;\n"), "//attribute[name='derive']", 1);
    // Inner attributes (`#![…]`) carry an <inner/> marker to
    // distinguish from outer (`#[…]`) attributes.
    claim("Rust inner attribute carries inner marker",
        &mut parse_src("rust", "#![allow(dead_code)]\nfn f() {}\n"),
        &multi_xpath(r#"
            //attribute[name='allow']
                [inner]
        "#),
        1);
}

#[test]
fn php_attribute_is_direct_child() {
    claim("PHP attribute is a direct child of the class it decorates",
        &mut parse_src("php", "<?php #[Deprecated] class X {}\n"),
        &multi_xpath(r#"
            //class/attribute
                [name='Deprecated']
        "#),
        1);
}
