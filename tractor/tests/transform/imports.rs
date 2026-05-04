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
    claim("C# using directive renders as <import> with the namespace path as name children (iter 203: wrapped in <path>)",
        &mut parse_src("csharp", "using System.Collections.Generic;\n"),
        &multi_xpath(r#"
            //import/path
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

/// C# namespace declarations come in two syntactic forms — a
/// classic block-scoped form (`namespace Foo { ... }`) and the
/// C# 10+ file-scoped form (`namespace Foo;` with the declarations
/// at the top level). Per Principle #5 (unified concepts), both
/// share the same shape: declarations are direct children of
/// `<namespace>`. The file-scoped form additionally carries a
/// `<file/>` marker so queries can distinguish the two when needed
/// (`//namespace[file]` vs `//namespace[not(file)]`).
#[test]
fn csharp_namespace_block_vs_file_scoped() {
    claim("C# block-scoped namespace contains declarations directly",
        &mut parse_src("csharp", "namespace Block { class A {} }\n"),
        "//namespace[name='Block']/class[name='A']",
        1);

    let mut file_scoped = parse_src("csharp", "namespace File;\nclass A {}\n");

    claim("C# file-scoped namespace contains declarations directly",
        &mut file_scoped,
        "//namespace[name='File']/class[name='A']",
        1);

    claim("C# file-scoped namespace carries a <file/> marker",
        &mut file_scoped,
        "//namespace[name='File'][file]",
        1);
}

#[test]
fn python_plain_import() {
    claim("Python plain import renders as <import> with the module name wrapped in <path>",
        &mut parse_src("python", "import os\n"),
        "//import/path/name='os'",
        1);
}

#[test]
fn python_from_import() {
    claim("Python from-import renames the statement to <from> with structured path/import children",
        &mut parse_src("python", "from foo import bar\n"),
        &multi_xpath(r#"
            //from
                [path/name='foo']
                [import/name='bar']
        "#),
        1);

    claim("no <import_from_statement> raw kind leaks",
        &mut parse_src("python", "from foo import bar\n"),
        "//import_from_statement",
        0);
}

/// `<from>`/`<import>` carries `list="imports"` regardless of how
/// many entities are imported. Per Principle #12, the `<import>`
/// role inside `<from>` is always a list — JSON consumers see
/// `imports: [...]` for both `from x import y` (single) and
/// `from x import a, b, c` (multi). The cardinality-gated
/// `tag_multi_role_children` helper would split the JSON shape
/// here; `python_tag_from_imports_uniform` overrides that for
/// this specific (parent, child) pair.
#[test]
fn python_from_import_list_attr_uniform() {
    claim("Single-name from-import has list='imports' on its <import> child",
        &mut parse_src("python", "from foo import bar\n"),
        "//from/import[@list='imports']",
        1);

    claim("Multi-name from-import has list='imports' on every <import> child",
        &mut parse_src("python", "from foo import a, b, c\n"),
        "//from/import[@list='imports']",
        3);
}

/// Rust `use std::fmt::{Display, Write as IoWrite};` produces
/// `<use[group]>` containing multiple inner `<use>` siblings (one
/// per imported entity). Per Principle #19 those are role-uniform
/// (each is an imported entity); tag with `list="uses"` so JSON
/// renders as `uses: [...]` array rather than colliding on the
/// singleton `use` JSON key. Plain non-group `use HashSet as Set;`
/// is unaffected (no inner `<use>` siblings).
/// PHP `use Foo\{First, Second};` produces `<use[group]>` with
/// multiple inner `<use>` siblings (one per imported entity). Per
/// Principle #19 those are role-uniform (each is an imported
/// entity); tag with `list="uses"` so JSON renders as `uses: [...]`
/// array. Mirrors Rust iter 267.
#[test]
fn php_use_group_lists_inner_uses() {
    claim("PHP use-group tags each inner <use> with list='uses'",
        &mut parse_src("php", "<?php use Foo\\{First, Second, Third};\n"),
        "//use[group]/use[@list='uses']",
        3);

    claim("PHP plain non-group use is unaffected (no inner <use> siblings)",
        &mut parse_src("php", "<?php use Foo\\Bar;\n"),
        "//use/use",
        0);
}

#[test]
fn rust_use_group_lists_inner_uses() {
    claim("Rust use-group tags each inner <use> with list='uses'",
        &mut parse_src(
            "rust",
            "use std::fmt::{Display, Write as IoWrite, self};\n",
        ),
        "//use[group]/use[@list='uses']",
        3);

    claim("Rust plain use (no group) is unaffected — no inner <use> siblings",
        &mut parse_src("rust", "use std::collections::HashMap;\n"),
        "//use/use",
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

    claim("Java import declaration renders as <import> with a flat-segment path",
        &mut tree,
        &multi_xpath(r#"
            //import/path
                [name='java']
                [name='util']
                [name='List']
        "#),
        1);
}

/// Cross-language: every dotted/scoped import path renders as
/// `<import>/<path>` with one `<name>` per segment in source order.
/// The element name (`<import>`) and the path-flattening shape
/// (`<path>` direct children = bare `<name>` segments) is the
/// contract; languages differ only in the source-syntax sigils.
///
/// Pinning this in one place catches the archetype where a future
/// per-language change re-introduces a nested `<path>/<path>` shape
/// (Principle #2 / Principle #5) on one language while the others
/// stay flat. Per-language imports.rs tests above pin one language at
/// a time; this test pins them all uniformly.
#[test]
fn cross_language_import_path_flat_segments() {
    let canonical = r#"
        //import/path
            [name='aaa']
            [name='bbb']
            [name='ccc']
    "#;

    for (lang, src) in &[
        ("csharp", "using aaa.bbb.ccc;\n"),
        ("java",   "import aaa.bbb.ccc;\n"),
        ("python", "import aaa.bbb.ccc\n"),
    ] {
        claim(
            &format!("{lang}: dotted import flattens to <import>/<path>/<name>+"),
            &mut parse_src(lang, src),
            &multi_xpath(canonical),
            1,
        );
    }

    // Rust's `use std::a::b::c;` uses `<use>` (per Rust idiom — `use`
    // is a first-class language keyword, distinct from the import
    // semantics) rather than `<import>`. The PATH SHAPE inside is
    // identical though: `<path>/<name>+` flat segments, with the
    // leaf lifted out to be `<use>`'s direct `<name>` child.
    claim(
        "rust: scoped use flattens its prefix to <use>/<path>/<name>+ with leaf as direct <name>",
        &mut parse_src("rust", "use aaa::bbb::ccc;\n"),
        &multi_xpath(r#"
            //use
                [path
                    [name='aaa']
                    [name='bbb']]
                [name='ccc']
        "#),
        1,
    );
}
