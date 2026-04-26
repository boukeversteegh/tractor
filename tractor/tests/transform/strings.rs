//! Cross-language: string literal shape, including raw strings and
//! interpolation.
//!
//! Every language that supports string interpolation wraps the
//! interpolated expression in an `<interpolation>` element inside
//! `<string>` (or `<template>` in TS). The element name is shared;
//! the delimiter tokens (`${` / `#{` / `{` / `$`) live as text inside
//! the `<string>` (or, for some languages, inside the
//! `<interpolation>`) but the queryable shape
//! `//string/interpolation/<expr>` works uniformly across languages.

use crate::support::semantic::*;

#[test]
fn python_interpolation() {
    let mut tree = parse_src("python", "x = f\"hi {name}!\"\n");
    assert_count(
        &mut tree,
        "//string/interpolation/name[.='name']",
        1,
        "Python f-string interpolation wraps the expression",
    );
}

#[test]
fn typescript_template() {
    let mut tree = parse_src(
        "typescript",
        "const s = `hello ${name}!`;\n",
    );
    assert_count(
        &mut tree,
        "//template/interpolation/name[.='name']",
        1,
        "TypeScript template interpolation wraps the expression",
    );
}

#[test]
fn ruby_double_quote() {
    let mut tree = parse_src(
        "ruby",
        "s = \"hi #{name}!\"\n",
    );
    assert_count(
        &mut tree,
        "//string/interpolation/name[.='name']",
        1,
        "Ruby double-quote interpolation wraps the expression",
    );
}

#[test]
fn csharp_interpolated_string() {
    let mut tree = parse_src(
        "csharp",
        "class X { string s = $\"hi {Name}!\"; }",
    );
    assert_count(
        &mut tree,
        "//string/interpolation/name[.='Name']",
        1,
        "C# interpolated string wraps the expression",
    );
}

#[test]
fn php_variable_interpolation() {
    let mut tree = parse_src(
        "php",
        "<?php $s = \"hi $name!\";\n",
    );
    assert_count(
        &mut tree,
        "//string/interpolation/variable/name[.='name']",
        1,
        "PHP variable interpolation wraps the expression",
    );
}

#[test]
fn php_complex_interpolation() {
    let mut tree = parse_src(
        "php",
        "<?php $s = \"x {$obj->method()}\";\n",
    );
    assert_count(
        &mut tree,
        "//string/interpolation/call",
        1,
        "PHP complex interpolation wraps the expression",
    );
}

/// Go strings: interpreted (double-quoted, escapes) and raw
/// (backtick, no escapes). Both render as <string>; raw strings
/// carry a <raw/> marker (Principle #13).
#[test]
fn go() {
    let mut tree = parse_src("go", r#"
        package main

        const normal = "hello\nworld"
        const raw = `hello world`
        const pattern = `^\d+$`
    "#);

    claim("3 strings total — bare //string catches both forms",
        &mut tree, "//string", 3);

    claim("interpreted string has no <raw/> marker",
        &mut tree, "//string[not(raw)]", 1);

    claim("two backtick strings carry <raw/>",
        &mut tree, "//string[raw]", 2);

    claim("raw and not-raw partition the strings",
        &mut tree, "//string[raw and not(raw)]", 0);
}

/// Rust strings: regular `"..."` and raw `r"..."`. Both render as
/// <string>; raw forms carry a <raw/> marker.
#[test]
fn rust() {
    let mut tree = parse_src("rust", r#"
        fn f() {
            let _ = r"raw";
            let _ = "normal";
        }
    "#);

    claim("raw string carries <raw/> marker",
        &mut tree, "//string[raw]", 1);

    claim("both raw and normal strings use <string>",
        &mut tree, "//string", 2);
}

/// F-strings render as <string> with <interpolation> children
/// and bare literal text in between (Principle #12: grammar
/// wrappers like string_start / string_content / string_end are
/// flattened). Plain strings collapse to a text-only <string>.
#[test]
fn python_fstring() {
    let mut tree = parse_src("python", r#"
plain = "hello"
greeting = f"hello {name}"
status = f"hello {name}, you are {age}"
"#);

    claim("3 strings total",
        &mut tree, "//string", 3);

    claim("plain string has no <interpolation> child",
        &mut tree, "//string[not(interpolation)]", 1);

    claim("two f-strings carry interpolations",
        &mut tree, "//string[interpolation]", 2);

    claim("interpolation wraps a <name>",
        &mut tree, "//string/interpolation/name='name'", 1);

    claim("`status` f-string has 2 interpolations",
        &mut tree, "//string[count(interpolation)=2]", 1);

    claim("interpolation can match by interpolated name",
        &mut tree, "//string/interpolation[name='age']", 1);

    claim("string_content grammar wrapper flattens to text",
        &mut tree, "//string_content", 0);

    claim("string_start grammar wrapper flattens to text",
        &mut tree, "//string_start", 0);
}
