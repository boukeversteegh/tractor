//! Cross-language: primitive literal element names.
//!
//! These are pure renames, but the per-language naming choices
//! split the cross-language vocabulary in three deliberate ways
//! that are documented per spec:
//!
//! 1. **Numbers** — most languages distinguish `<int>` and
//!    `<float>` (matching their own type names). TypeScript /
//!    JavaScript collapse to a single `<number>` element since the
//!    JS grammar has no integer / float split.
//! 2. **Booleans** — Rust, TypeScript, C#, and PHP render booleans
//!    as a single `<bool>` element whose text is `true` or
//!    `false`. Java, Go, Python, and Ruby split into `<true>` /
//!    `<false>` separate elements (the source keyword IS the
//!    semantic kind in those grammars).
//! 3. **Nulls** — `<null>` (Java, C#, TypeScript, PHP), `<nil>`
//!    (Go, Ruby), `<none>` (Python). Each preserves the
//!    language-idiomatic keyword.

use crate::support::semantic::*;

#[test]
fn java() {
    let mut tree = parse_src("java", r#"
        class Settings {
            int retries = 3;
            double pi = 3.14;
            boolean enabled = true;
            boolean disabled = false;
            String missing = null;
        }
    "#);

    claim("Java integer literal renders as <int>",
        &mut tree, "//field[name='retries']/value/int='3'", 1);

    claim("Java floating-point literal renders as <float>",
        &mut tree, "//field[name='pi']/value/float='3.14'", 1);

    claim("Java true keyword renders as <true> (split element)",
        &mut tree, "//field[name='enabled']/value/true='true'", 1);

    claim("Java false keyword renders as <false> (split element)",
        &mut tree, "//field[name='disabled']/value/false='false'", 1);

    claim("Java null keyword renders as <null>",
        &mut tree, "//field[name='missing']/value/null='null'", 1);
}

#[test]
fn csharp() {
    let mut tree = parse_src("csharp", r#"
        class Settings {
            int retries = 3;
            double pi = 3.14;
            bool enabled = true;
            string missing = null;
        }
    "#);

    claim("C# integer literal renders as <int>",
        &mut tree, "//field[name='retries']/int='3'", 1);

    claim("C# floating-point literal renders as <float>",
        &mut tree, "//field[name='pi']/float='3.14'", 1);

    claim("C# true keyword renders as <bool> with text 'true'",
        &mut tree, "//field[name='enabled']/bool='true'", 1);

    claim("C# null keyword renders as <null>",
        &mut tree, "//field[name='missing']/null='null'", 1);
}

#[test]
fn typescript() {
    let mut tree = parse_src("typescript", r#"
        const retries = 3;
        const pi = 3.14;
        const enabled = true;
        const missing = null;
    "#);

    claim("TypeScript integer literal renders as <number> (no int/float split)",
        &mut tree, "//variable[name='retries']/value/number='3'", 1);

    claim("TypeScript floating-point literal also renders as <number>",
        &mut tree, "//variable[name='pi']/value/number='3.14'", 1);

    claim("TypeScript true keyword renders as <bool> with text 'true'",
        &mut tree, "//variable[name='enabled']/value/bool='true'", 1);

    claim("TypeScript null keyword renders as <null>",
        &mut tree, "//variable[name='missing']/value/null='null'", 1);
}

#[test]
fn rust() {
    let mut tree = parse_src("rust", r#"
        fn run() {
            let retries = 3;
            let pi = 3.14;
            let enabled = true;
        }
    "#);

    claim("Rust integer literal renders as <int>",
        &mut tree, "//let[name='retries']/value/int='3'", 1);

    claim("Rust floating-point literal renders as <float>",
        &mut tree, "//let[name='pi']/value/float='3.14'", 1);

    claim("Rust true keyword renders as <bool> with text 'true'",
        &mut tree, "//let[name='enabled']/value/bool='true'", 1);
}

#[test]
fn go() {
    let mut tree = parse_src("go", r#"
        package main

        var retries = 3
        var pi = 3.14
        var enabled = true
        var disabled = false
        var missing = nil
    "#);

    claim("Go integer literal renders as <int>",
        &mut tree, "//var[name='retries']/value/int='3'", 1);

    claim("Go floating-point literal renders as <float>",
        &mut tree, "//var[name='pi']/value/float='3.14'", 1);

    claim("Go true keyword renders as <true> (split element)",
        &mut tree, "//var[name='enabled']/value/true='true'", 1);

    claim("Go false keyword renders as <false> (split element)",
        &mut tree, "//var[name='disabled']/value/false='false'", 1);

    claim("Go nil keyword renders as <nil>",
        &mut tree, "//var[name='missing']/value/nil='nil'", 1);
}

#[test]
fn python() {
    let mut tree = parse_src("python", r#"
        retries = 3
        pi = 3.14
        enabled = True
        disabled = False
        missing = None
    "#);

    claim("Python integer literal renders as <int>",
        &mut tree, "//assign[left/name='retries']/right/int='3'", 1);

    claim("Python floating-point literal renders as <float>",
        &mut tree, "//assign[left/name='pi']/right/float='3.14'", 1);

    claim("Python True keyword renders as <true> (split element, lowercase tag)",
        &mut tree, "//assign[left/name='enabled']/right/true='True'", 1);

    claim("Python False keyword renders as <false> (split element, lowercase tag)",
        &mut tree, "//assign[left/name='disabled']/right/false='False'", 1);

    claim("Python None keyword renders as <none>",
        &mut tree, "//assign[left/name='missing']/right/none='None'", 1);
}

#[test]
fn ruby() {
    let mut tree = parse_src("ruby", r#"
        retries = 3
        pi = 3.14
        enabled = true
        disabled = false
        missing = nil
    "#);

    claim("Ruby integer literal renders as <int>",
        &mut tree, "//assign[left/name='retries']/right/int='3'", 1);

    claim("Ruby floating-point literal renders as <float>",
        &mut tree, "//assign[left/name='pi']/right/float='3.14'", 1);

    claim("Ruby true keyword renders as <true> (split element)",
        &mut tree, "//assign[left/name='enabled']/right/true='true'", 1);

    claim("Ruby false keyword renders as <false> (split element)",
        &mut tree, "//assign[left/name='disabled']/right/false='false'", 1);

    claim("Ruby nil keyword renders as <nil>",
        &mut tree, "//assign[left/name='missing']/right/nil='nil'", 1);
}

#[test]
fn php() {
    let mut tree = parse_src("php", r#"<?php
        $retries = 3;
        $pi = 3.14;
        $enabled = true;
        $missing = null;
    "#);

    claim("PHP integer literal renders as <int>",
        &mut tree, "//assign[left/variable/name='retries']/right/int='3'", 1);

    claim("PHP floating-point literal renders as <float>",
        &mut tree, "//assign[left/variable/name='pi']/right/float='3.14'", 1);

    claim("PHP true keyword renders as <bool> with text 'true'",
        &mut tree, "//assign[left/variable/name='enabled']/right/bool='true'", 1);

    claim("PHP null keyword renders as <null>",
        &mut tree, "//assign[left/variable/name='missing']/right/null='null'", 1);
}
