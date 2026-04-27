//! Cross-language: import-statement and namespace renames.
//!
//! Scope is intentionally narrow. The implemented decisions are:
//!
//! - C# `using_directive` → `<import>` (developer mental model:
//!   "I'm importing the System namespace").
//! - Python `import_statement` → `<import>` and
//!   `import_from_statement` → `<from>` (the `from` keyword reads
//!   more naturally than `import_from`).
//! - Java `import_declaration` → `<import>`; the qualified target
//!   `com.example.Foo` becomes a nested `<path>` chain (covered in
//!   detail by `members.rs::java_scoped_path`).
//! - Java `package_declaration` → `<package>`.
//!
//! Open-question grouping shapes (Go `import (…)` blocks, PHP `use`
//! variants, TypeScript named-import patterns) are NOT tested
//! here; their final shape is still unsettled in the spec docs.

use crate::support::semantic::*;

#[test]
fn csharp_using_renames_to_import() {
    claim("C# using directive renders as <import> with the namespace path as name children",
        &mut parse_src("csharp", "using System.Collections.Generic;\n"),
        &multi_xpath(r#"
            //import
                [name='System']
                [name='Collections']
                [name='Generic']
        "#),
        1);

    claim("no <using> declaration leaks at the top level",
        &mut parse_src("csharp", "using System;\n"),
        "//unit/using",
        0);
}

#[test]
fn python_plain_import() {
    claim("Python plain import renders as <import> with the module name as a child",
        &mut parse_src("python", "import os\n"),
        "//import/name='os'",
        1);
}

#[test]
fn python_from_import() {
    claim("Python from-import renames the statement to <from>",
        &mut parse_src("python", "from foo import bar\n"),
        &multi_xpath(r#"
            //from
                [name='foo']
                [name='bar']
        "#),
        1);

    claim("no <import_from_statement> raw kind leaks",
        &mut parse_src("python", "from foo import bar\n"),
        "//import_from_statement",
        0);
}

#[test]
fn java_import_and_package() {
    let mut tree = parse_src("java", r#"
        package com.example;

        import java.util.List;
    "#);

    claim("Java package declaration renders as <package> with a path target",
        &mut tree,
        &multi_xpath(r#"
            //package/path
                [name='com']
                [name='example']
        "#),
        1);

    claim("Java import declaration renders as <import> with a nested-path target",
        &mut tree,
        &multi_xpath(r#"
            //import/path
                [path
                    [name='java']
                    [name='util']]
                [name='List']
        "#),
        1);
}
