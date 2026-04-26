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
    claim("Python class shape has direct decorator child",
        &mut tree,
        &multi_xpath(r#"
            //class[name='X']
                [decorator[name='dataclass']]
        "#),
        1);

    claim("no <decorated> wrapper — topology matches Java/C#/Rust",
        &mut tree, "//decorated", 0);
}

#[test]
fn java_annotation_is_direct_child() {
    let mut tree = parse_src(
        "java",
        "class X { @Override public void f() {} }",
    );
    claim("Java method shape has direct annotation child",
        &mut tree,
        &multi_xpath(r#"
            //method[name='f']
                [annotation[name='Override']]
        "#),
        1);
}

#[test]
fn csharp_attribute_is_direct_child() {
    let mut tree = parse_src(
        "csharp",
        "class X { [Obsolete] [MaxLength(50)] public string Name; }",
    );

    claim("C# field shape has direct attributes with flat name and argument shape",
        &mut tree,
        &multi_xpath(r#"
            //field
                [attribute[name='Obsolete']]
                [attribute[name='MaxLength']
                    [name]
                    [argument/int='50']
                ]
        "#),
        1);

    claim("attribute <name> holds the identifier as text (no nested <name>)",
        &mut tree, "//attribute[name='MaxLength']/name/*", 0);
}

#[test]
fn rust_attribute_is_flat() {
    let mut tree = parse_src("rust", "#[derive(Debug)] struct S;\n");
    // #[derive] surfaces as a sibling `<attribute>` at the file
    // level — `attribute_item` wrapper was flattened.
    claim("Rust outer attribute flattens to attribute with name",
        &mut tree, "//attribute[name='derive']", 1);
    // Inner attributes (`#![…]`) carry an <inner/> marker to
    // distinguish from outer (`#[…]`) attributes.
    let mut inner = parse_src("rust", "#![allow(dead_code)]\nfn f() {}\n");
    claim("Rust inner attribute carries inner marker",
        &mut inner,
        &multi_xpath(r#"
            //attribute[name='allow']
                [inner]
        "#),
        1);
}

#[test]
fn php_attribute_is_direct_child() {
    let mut tree = parse_src(
        "php",
        "<?php #[Deprecated] class X {}\n",
    );
    claim("PHP class shape has direct attribute child",
        &mut tree,
        &multi_xpath(r#"
            //class[name='X']
                [attribute[name='Deprecated']]
        "#),
        1);
}
