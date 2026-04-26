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
    claim("plain Python string has no interpolation child",
        &mut parse_src("python", "\"hi\"\n"),
        &multi_xpath(r#"
            //string[.='"hi"']
                [not(interpolation)]
        "#),
        1);

    claim("Python f-string wraps interpolation expression",
        &mut parse_src("python", "f\"hi {name}!\"\n"),
        &multi_xpath(r#"
            //string
                [interpolation/name='name']
        "#),
        1);
}

#[test]
fn typescript_template() {
    claim("TypeScript template wraps interpolation expression",
        &mut parse_src("typescript", "`hello ${name}!`;\n"),
        &multi_xpath(r#"
            //template
                [interpolation/name='name']
        "#),
        1);
}

#[test]
fn ruby_double_quote() {
    claim("Ruby double-quote string wraps interpolation expression",
        &mut parse_src("ruby", "\"hi #{name}!\"\n"),
        &multi_xpath(r#"
            //string
                [interpolation/name='name']
        "#),
        1);
}

#[test]
fn csharp_interpolated_string() {
    claim("C# interpolated string wraps interpolation expression",
        &mut parse_src("csharp", "class X { string s = $\"hi {Name}!\"; }"),
        &multi_xpath(r#"
            //string
                [interpolation/name='Name']
        "#),
        1);
}

#[test]
fn php_variable_interpolation() {
    claim("PHP string wraps variable interpolation",
        &mut parse_src("php", "<?php \"hi $name!\";\n"),
        &multi_xpath(r#"
            //string
                [interpolation/variable/name='name']
        "#),
        1);
}

#[test]
fn php_complex_interpolation() {
    claim("PHP string wraps complex call interpolation",
        &mut parse_src("php", "<?php \"x {$obj->method()}\";\n"),
        &multi_xpath(r#"
            //string
                [interpolation/call]
        "#),
        1);
}

/// Go strings: interpreted (double-quoted, escapes) and raw
/// (backtick, no escapes). Both render as <string>; raw strings
/// carry a <raw/> marker (Principle #13).
#[test]
fn go() {
    claim("Go interpreted string has no raw marker",
        &mut parse_src("go", r#"
        package main

        const normal = "hello\nworld"
"#),
        "//string[not(raw)]",
        1);

    let mut raw = parse_src("go", r#"
        package main

        const raw = `hello world`
    "#);

    claim("Go raw string carries raw marker",
        &mut raw, "//string[raw]", 1);

    claim("raw and not-raw partition the strings",
        &mut raw, "//string[raw and not(raw)]", 0);
}

/// Rust strings: regular `"..."` and raw `r"..."`. Both render as
/// <string>; raw forms carry a <raw/> marker.
#[test]
fn rust() {
    claim("Rust raw string carries raw marker",
        &mut parse_src("rust", r#"
        fn f() {
            let _ = r"raw";
        }
"#),
        "//string[raw]",
        1);

    claim("Rust normal string has no raw marker",
        &mut parse_src("rust", r#"
        fn f() {
            let _ = "normal";
        }
"#),
        "//string[not(raw)]",
        1);
}

/// F-strings render as <string> with <interpolation> children
/// and bare literal text in between (Principle #12: grammar
/// wrappers like string_start / string_content / string_end are
/// flattened). Plain strings collapse to a text-only <string>.
#[test]
fn python_fstring() {
    claim("plain Python string has no interpolation child",
        &mut parse_src("python", "\"hello\"\n"), "//string[not(interpolation)]", 1);

    claim("Python f-string with one expression has one interpolation child",
        &mut parse_src("python", "f\"hello {name}\"\n"),
        &multi_xpath(r#"
            //string
                [interpolation/name='name']
                [count(interpolation)=1]
        "#),
        1);

    claim("Python f-string with two expressions has two interpolation children",
        &mut parse_src("python", "f\"hello {name}, you are {age}\"\n"),
        &multi_xpath(r#"
            //string
                [interpolation/name='name']
                [interpolation/name='age']
                [count(interpolation)=2]
        "#),
        1);
}
