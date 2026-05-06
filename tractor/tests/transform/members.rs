//! Cross-language: member access, scoped paths, and the
//! call-target wrapper.
//!
//! - `<member>` is the universal element for attribute / property
//!   access (`obj.x`, `obj.method`).
//! - `<path>` is the rename for scoped / qualified identifier
//!   chains (`std::collections::HashMap`, `com.example.Foo`).
//! - `<callee>` wraps the function position of a `<call>` so that
//!   precise queries can distinguish "the function being called"
//!   from "the function being declared".
//! - Rust unifies declaration / initialiser / access under a single
//!   `<field>` element; the parent (`<struct>` body, `<literal>`
//!   body, or value-position) disambiguates context.

use crate::support::semantic::*;

// ---- chain (post chain-inversion iter 243) -------------------------------

/// After chain inversion, TypeScript calls match the canonical
/// shape: `<call>` directly contains the callee (no `<callee>`
/// wrapper). Method calls become `<object[access]>` chains.
#[test]
fn typescript_callee() {
    claim("TypeScript plain call has the function name as a direct call child",
        &mut parse_src("typescript", "f(1, 2);\n"),
        "//call/name='f'",
        1);

    claim("TypeScript method call inverts to <object[access]> chain shape",
        &mut parse_src("typescript", "console.log(x);\n"),
        &multi_xpath(r#"
            //object[access]
                [name='console']
                [call/name='log']
        "#),
        1);
}

// ---- member ---------------------------------------------------------------

/// `<member>` is the cross-language element for attribute /
/// property access (Principle #5). Rendered uniformly across C# /
/// Java / TS regardless of the source's tree-sitter kind
/// (`member_access_expression`, `field_access`, `member_expression`).
#[test]
fn csharp_member() {
    claim("C# method call inverts to <object[access]> chain shape (iter 245)",
        &mut parse_src("csharp", r#"
        class X {
            void f() {
                obj.Method();
            }
        }
    "#),
        &multi_xpath(r#"
            //object[access]
                [name='obj']
                [call/name='Method']
        "#),
        1);
}

// ---- path -----------------------------------------------------------------

/// Scoped / qualified identifier chains rename to <path> in Java
/// and Rust. The chain nests left-recursively: every step is a
/// <path> wrapping a previous <path> plus the new <name> leaf.
#[test]
fn java_scoped_path() {
    claim("Java import target flattens com.example.Foo into a single path of name segments",
        &mut parse_src("java", "import com.example.Foo;\n"),
        &multi_xpath(r#"
            //import/path
                [name='com']
                [name='example']
                [name='Foo']
        "#),
        1);
}

#[test]
fn rust_scoped_path() {
    claim("Rust use target lifts the leaf out of <path> and exposes the prefix segments as flat path/name children",
        &mut parse_src("rust", "use std::collections::HashMap;\n"),
        &multi_xpath(r#"
            //use
                [path
                    [name='std']
                    [name='collections']]
                [name='HashMap']
        "#),
        1);
}

// ---- field_unification ----------------------------------------------------

/// Rust uses one element name — `<field>` — for THREE distinct
/// constructs (declaration, initialisation, access). The parent
/// disambiguates: `<struct>/<body>` for declarations,
/// `<literal>/<body>` for struct-expression initialisers,
/// value-position elsewhere for field-access expressions.
#[test]
fn rust_field_unification() {
    let mut tree = parse_src("rust", r#"
        struct Point { x: i32 }

        fn use_point() {
            let p = Point { x: 1 };
            let v = p.x;
        }
    "#);

    claim("struct field declaration is <field> with type child under <struct>/<body>",
        &mut tree,
        &multi_xpath(r#"
            //struct[name='Point']/body/field
                [name='x']
                [type/name='i32']
                [not(value)]
        "#),
        1);

    claim("struct expression initialiser is <field> with value child under <literal>/<body>",
        &mut tree,
        &multi_xpath(r#"
            //literal[name='Point']/body/field
                [name='x']
                [value/expression/int='1']
                [not(type)]
        "#),
        1);

    claim("field access expression inverts to <object[access]> chain (iter 248)",
        &mut tree,
        &multi_xpath(r#"
            //object[access]
                [name='p']
                [member/name='x']
        "#),
        1);
}

/// Cross-language: PHP `Foo::CONST` and Ruby `Foo::Bar` (both use
/// the `::` scope-resolution operator) render uniformly as
/// `<member[static]>` (Principle #5 cross-language alignment).
///
/// Iter 342 unified the marker name: PHP previously used
/// `<member[constant]>` (misleading — `constant` doesn't apply to
/// static method-call sites, just the const-access shape it was
/// scoped to) while Ruby already used `<member[static]>` for the
/// equivalent shape. PHP's own `<call><static/>` for static method
/// calls already uses `<static/>`; this brings PHP `Foo::CONST`
/// into shape with both Ruby and PHP-internal conventions.
#[test]
fn cross_language_static_member_access_marker() {
    for (lang, src) in &[
        ("php", "<?php $x = Foo::BAR;"),
        ("ruby", "x = Foo::Bar\n"),
    ] {
        claim(
            &format!("{lang}: `Foo::X` scope-resolution renders as <member[static]>"),
            &mut parse_src(lang, src),
            "//member[static]",
            1,
        );
    }
}
