//! Cross-language: type SHAPE markers, reference / borrowed types,
//! type aliases, and the defined-vs-alias distinction.
//!
//! Generic vocabulary, bounds, and where-clause constraints live in
//! `generics.rs`.
//!
//! Principle #9 / #14: every type reference wraps its name in a
//! <name> child; type flavours (array, tuple, nullable, function,
//! pointer, never, dyn, etc.) collapse to <type> with a shape
//! marker so cross-language queries match on a uniform attribute.

use crate::support::semantic::*;

// ---- type_markers ---------------------------------------------------------

/// Rust type flavors all collapse to <type> with a shape marker —
/// function, tuple, array, pointer, never, unit, dyn. (The `[T]`
/// inside `&[T]` is treated as `array_type` by tree-sitter-rust,
/// so `slice` markers only appear for explicit slice forms — which
/// the cross-file blueprint snapshot covers separately.)
#[test]
fn rust_markers() {
    let mut tree = parse_src("rust", r#"
        fn f(cb: fn(i32) -> i32, t: (i32, i32), a: [u8; 4], p: *const u8) -> ! { loop {} }
        fn g() -> () {}
        fn h(d: &dyn Drawable) {}
    "#);

    claim("Rust function type carries function marker",
        &mut tree, "//parameter[name='cb']/type[function]", 1);

    claim("Rust tuple type carries tuple marker",
        &mut tree, "//parameter[name='t']/type[tuple]", 1);

    claim("Rust array type carries array marker",
        &mut tree, "//parameter[name='a']/type[array]", 1);

    claim("Rust pointer type carries pointer marker",
        &mut tree, "//parameter[name='p']/type[pointer]", 1);

    claim("Rust never return type carries never marker",
        &mut tree, "//returns/type[never]", 1);

    claim("Rust unit return type carries unit marker",
        &mut tree, "//returns/type[unit]", 1);

    claim("Rust dyn trait object carries dynamic marker inside borrowed type",
        &mut tree, "//type[borrowed]/type[dynamic]", 1);
}

/// C# type flavors — array/tuple/nullable — all collapse to
/// <type> with a shape marker. `nullable_type` gets its
/// <nullable/> marker via a direct rewrite (not the map) but the
/// end shape is the same.
#[test]
fn csharp_markers() {
    claim("C# type marker shapes stay attached to their field declarations",
        &mut parse_src("csharp", r#"
        class X {
            int[] a;
            (int, string) t;
            int? n;
        }
    "#),
        &multi_xpath(r#"
            //class[name='X']/body
                [field
                    [name='a']
                    [type[array]]
                    [not(variable)]]
                [field
                    [name='t']
                    [type[tuple]]
                    [not(variable)]]
                [field
                    [name='n']
                    [type[nullable]]
                    [not(variable)]]
        "#),
        1);
}

/// C# tuple types `(int count, string tag)` produce `<type[tuple]>`
/// with multiple `<element>` siblings (one per tuple position).
/// Per Principle #19 they're role-uniform; tag with
/// `list="elements"` so JSON renders as `elements: [...]` array.
#[test]
fn csharp_tuple_type_lists_elements() {
    claim("C# tuple type with two positions tags each <element>",
        &mut parse_src("csharp", r#"
        class T { void M() { (int count, string tag) p = (1, "x"); } }
    "#),
        "//type[tuple]/element[@list='elements']",
        2);
}

/// C# LINQ `from n in numbers` wraps the source in `<value>` slot
/// so the binding `<name>` doesn't collide with the source `<name>`
/// on the JSON `name` key.
#[test]
fn csharp_linq_from_clause_wraps_source() {
    claim("C# `from n in numbers` wraps the source in <value>",
        &mut parse_src("csharp", r#"
        class T { void M() { var q = from n in numbers select n; } }
    "#),
        "//from[name='n'][value/expression/name='numbers']",
        1);
}

/// TypeScript type flavors all collapse to <type> with a shape
/// marker (Principle #9) so `//type[union]`, `//type[tuple]`,
/// etc. work uniformly without matching on text.
#[test]
fn typescript_markers() {
    let mut tree = parse_src("typescript", r#"
        type A = string | number;
        type B = string & object;
        type C = [string, number];
        type D = string[];
        type E = 'idle';
        type F = (x: number) => number;
        type G = { x: number };
        type H = readonly number[];
    "#);

    claim("TypeScript union type carries union marker",
        &mut tree, "//alias[name='A']/type[union]", 1);

    claim("TypeScript intersection type carries intersection marker",
        &mut tree, "//alias[name='B']/type[intersection]", 1);

    claim("TypeScript tuple type carries tuple marker",
        &mut tree, "//alias[name='C']/type[tuple]", 1);

    claim("TypeScript array type carries array marker",
        &mut tree, "//alias[name='D']/type[array]", 1);

    claim("TypeScript literal type carries literal marker",
        &mut tree, "//alias[name='E']/type[literal]", 1);

    claim("TypeScript function type carries function marker",
        &mut tree, "//alias[name='F']/type[function]", 1);

    claim("TypeScript object type carries object marker",
        &mut tree, "//alias[name='G']/type[object]", 1);

    claim("TypeScript readonly array carries readonly marker outside array marker",
        &mut tree, "//alias[name='H']/type[readonly]/type[array]", 1);
}

/// TypeScript `asserts x is T` should collapse to a single
/// `<predicate>` with an `<asserts/>` marker and the bound name/type
/// directly under it.
#[test]
fn typescript_asserts_predicate() {
    let mut tree = parse_src("typescript", r#"
        function f(x: unknown): asserts x is number {
            return;
        }
    "#);

    claim("asserts predicates surface the variable name and type directly",
        &mut tree,
        &multi_xpath(r#"
            //function[name='f']/returns/predicate
                [asserts]
                [name='x']
                [type[name='number']]
        "#),
        1);

    claim("raw type-predicate wrappers do not survive the transform",
        &mut tree,
        "//function[name='f']/returns/predicate/type_predicate",
        0);
}

/// Java `void` carries an additional <void/> marker on top of the
/// `<name>void</name>` text leaf — the marker is a query
/// shortcut, not a replacement. Other primitives keep just the
/// name child.
#[test]
fn java_markers() {
    claim("Java method return types distinguish void marker from named primitive",
        &mut parse_src("java", r#"
        class X {
            void f() {}
            int g() { return 0; }
        }
    "#),
        &multi_xpath(r#"
            //class[name='X']/body
                [method[name='f']
                    [returns/type[name='void']
                        [void]]]
                [method[name='g']
                    [returns/type[name='int']
                        [not(void)]]]
        "#),
        1);
}

// ---- reference_type -------------------------------------------------------

/// Reference types `&T` / `&mut T` / `&'a T` render as a single
/// <type> with a <borrowed/> marker (Principles #14 + #13). The
/// inner referenced type is a nested <type> child.
#[test]
fn rust_reference() {
    let mut tree = parse_src("rust", r#"
        fn read(s: &str) -> &str { s }
        fn write(buf: &mut Vec<u8>) {}
        fn static_ref() -> &'static str { "" }
    "#);

    claim("Rust borrowed parameter wraps referenced type",
        &mut tree, "//parameter[name='s']/type[borrowed]/type[name='str']", 1);

    claim("Rust borrowed return wraps referenced type",
        &mut tree, "//returns/type[borrowed]/type[name='str']", 2);

    claim("Rust mutable borrow carries mut marker and generic referent type",
        &mut tree,
        &multi_xpath(r#"
            //parameter[name='buf']/type[borrowed]
                [mut]
                [type[name='Vec']
                    [generic]
                    [type[name='u8']]]
        "#),
        1);

    claim("Rust lifetime borrow exposes lifetime child",
        &mut tree, "//type[borrowed][lifetime/name='static'][type/name='str']", 1);

    claim("no legacy <ref> element",
        &mut tree, "//ref", 0);
}

// ---- typedef --------------------------------------------------------------

/// Rust `type_item` renders as <alias> (parallel with
/// TS / Java / C#).
#[test]
fn rust_typedef() {
    let mut tree = parse_src("rust", r#"
        type Id = u32;
        type Mapping<T> = std::collections::HashMap<String, T>;
    "#);

    claim("Rust simple alias exposes visibility and target type",
        &mut tree, "//alias[name='Id'][private][type/name='u32']", 1);

    claim("Rust generic alias exposes generic parameter and target type arguments",
        &mut tree,
        &multi_xpath(r#"
            //alias[name='Mapping']
                [private]
                [generic[name='T']]
                [type[name='std::collections::HashMap']
                    [generic]
                    [type[name='String']]
                    [type[name='T']]]
        "#),
        1);

    claim("no legacy <typedef> element",
        &mut tree, "//typedef", 0);
}

// ---- defined_type_vs_alias ------------------------------------------------

/// Go distinguishes defined types (`type MyInt int`) from type
/// aliases (`type Color = int`). Defined type -> <type>; alias
/// -> <alias> (parallel with Rust / TS / C# / Java).
#[test]
fn go_defined_vs_alias() {
    let mut tree = parse_src("go", r#"
        package main

        type MyInt int
        type Color = int
    "#);

    claim("Go defined type renders as type declaration",
        &mut tree, "//type[name='MyInt']/type[name='int']", 1);

    claim("Go type alias renders as alias declaration",
        &mut tree, "//alias[name='Color']/type[name='int']", 1);

    claim("alias does NOT also render as <type> at the top level",
        &mut tree, "//file/type[name='Color']", 0);
}
