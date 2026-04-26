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
    claim("Python f-string assignment wraps interpolation expression",
        &mut tree,
        &multi_xpath(r#"
            //assign[left/name='x']
                [right/string/interpolation/name='name']
        "#),
        1);
}

#[test]
fn typescript_template() {
    let mut tree = parse_src(
        "typescript",
        "const s = `hello ${name}!`;\n",
    );
    claim("TypeScript template assignment wraps interpolation expression",
        &mut tree,
        &multi_xpath(r#"
            //variable[name='s']
                [value/template/interpolation/name='name']
        "#),
        1);
}

#[test]
fn ruby_double_quote() {
    let mut tree = parse_src(
        "ruby",
        "s = \"hi #{name}!\"\n",
    );
    claim("Ruby double-quote assignment wraps interpolation expression",
        &mut tree,
        &multi_xpath(r#"
            //assign[left/name='s']
                [right/string/interpolation/name='name']
        "#),
        1);
}

#[test]
fn csharp_interpolated_string() {
    let mut tree = parse_src(
        "csharp",
        "class X { string s = $\"hi {Name}!\"; }",
    );
    claim("C# field initializer wraps interpolated expression",
        &mut tree,
        &multi_xpath(r#"
            //field[.//name='s']
                [.//string/interpolation/name='Name']
        "#),
        1);
}

#[test]
fn php_variable_interpolation() {
    let mut tree = parse_src(
        "php",
        "<?php $s = \"hi $name!\";\n",
    );
    claim("PHP string assignment wraps variable interpolation",
        &mut tree,
        &multi_xpath(r#"
            //assign[left/variable/name='s']
                [right/string/interpolation/variable/name='name']
        "#),
        1);
}

#[test]
fn php_complex_interpolation() {
    let mut tree = parse_src(
        "php",
        "<?php $s = \"x {$obj->method()}\";\n",
    );
    claim("PHP string assignment wraps complex call interpolation",
        &mut tree,
        &multi_xpath(r#"
            //assign[left/variable/name='s']
                [right/string/interpolation/call]
        "#),
        1);
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

    claim("Go const shapes distinguish interpreted and raw strings",
        &mut tree,
        &multi_xpath(r#"
            //file
                [const[name='normal']
                    [value/string[not(raw)]]]
                [const[name='raw']
                    [value/string[raw]]]
                [const[name='pattern']
                    [value/string[raw]]]
                [count(.//string)=3]
        "#),
        1);

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

    claim("Rust let string values distinguish raw and normal strings",
        &mut tree,
        &multi_xpath(r#"
            //function[name='f']/body
                [let[value/string[raw]]]
                [let[value/string[not(raw)]]]
                [count(.//string)=2]
        "#),
        1);
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

    claim("Python string assignments distinguish plain and f-string interpolation shapes",
        &mut tree,
        &multi_xpath(r#"
            //module
                [assign
                    [left/name='plain']
                    [right/string[not(interpolation)]]]
                [assign
                    [left/name='greeting']
                    [right/string/interpolation/name='name']]
                [assign[left/name='status']
                    [right/string
                        [interpolation/name='name']
                        [interpolation/name='age']
                        [count(interpolation)=2]
                    ]
                ]
                [count(.//string)=3]
        "#),
        1);
}
