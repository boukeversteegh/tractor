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

// ---- callee ---------------------------------------------------------------

/// TypeScript wraps the call-target field as <callee> (rather than
/// <function>, which would collide with function declarations).
/// Plain calls have a direct name child; method calls have a
/// <member> child carrying receiver and property.
#[test]
fn typescript_callee() {
    claim("TypeScript plain call wraps the callee around the function name",
        &mut parse_src("typescript", "f(1, 2);\n"),
        "//call/callee/name='f'",
        1);

    claim("TypeScript method call wraps callee around a member access",
        &mut parse_src("typescript", "console.log(x);\n"),
        &multi_xpath(r#"
            //call/callee/member
                [object/name='console']
                [property/name='log']
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
    claim("C# member access wraps receiver in <object> and property in <property> (iter 178)",
        &mut parse_src("csharp", r#"
        class X {
            void f() {
                obj.Method();
            }
        }
    "#),
        &multi_xpath(r#"
            //call/member
                [object/name='obj']
                [property/name='Method']
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

    claim("field access expression is <field> with value-receiver and name leaf",
        &mut tree,
        &multi_xpath(r#"
            //field
                [value/expression/name='p']
                [name='x']
                [not(type)]
        "#),
        1);
}
