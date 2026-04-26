//! Cross-language: type vocabulary and shape markers, type aliases,
//! reference / borrowed types, and generic where-clause constraints.
//!
//! Principle #14: every type reference wraps its name in a <name>
//! child. No bare-text <type> nodes; type parameters use <generic>;
//! bounds wrap in <extends>; collection-of-T uses <type[generic]>
//! with nested <type> children.

use crate::support::semantic::*;

// ---- type_vocabulary ------------------------------------------------------

#[test]
fn csharp_vocabulary() {
    let mut tree = parse_src("csharp", r#"
        using System.Collections.Generic;

        interface IBarker { void Bark(); }
        class Animal {}

        class Dog<T> : Animal, IBarker where T : Animal
        {
            public T Owner;
            public List<string> Tags;
            public void Bark() {}
        }
    "#);

    claim("every <type> has a <name> child (no bare-text types)",
        &mut tree, "//type[not(name)]", 0);

    claim("Dog class shape keeps generics, bounds, inheritance, and field types visible",
        &mut tree,
        &multi_xpath(r#"
            //class[name='Dog']
                [generic[name='T']
                    [extends/type[name='Animal']]]
                [extends
                    [type[name='Animal']]
                    [type[name='IBarker']]]
                [body/field
                    [variable/declarator/name='Owner']
                    [variable/type[name='T']]]
                [body/field
                    [variable/declarator/name='Tags']
                    [variable/type[name='List']
                        [generic]
                        [type[name='string']]]]
        "#),
        1);
}

#[test]
fn java_vocabulary() {
    let mut tree = parse_src("java", r#"
        import java.util.List;

        class Animal {}
        interface Barker { void bark(); }
        interface Runner { void run(); }

        class Dog<T extends Animal> extends Animal implements Barker, Runner {
            int a;
            double b;
            boolean c;
            Foo e;
            List l;
            T owner;
            List<String> tags;

            public void bark() {}
            public void run() {}
        }
    "#);

    claim("every <type> has a <name> child",
        &mut tree, "//type[not(name)]", 0);

    claim("Dog class shape keeps bounds, inheritance, implementations, and field vocabulary",
        &mut tree,
        &multi_xpath(r#"
            //class[name='Dog']
                [generic[name='T']
                    [extends/type[name='Animal']]]
                [extends/type[name='Animal']]
                [implements
                    [type[name='Barker']]
                    [type[name='Runner']]]
                [body/field
                    [declarator/name='a']
                    [type[name='int']]]
                [body/field
                    [declarator/name='b']
                    [type[name='double']]]
                [body/field
                    [declarator/name='c']
                    [type[name='boolean']]]
                [body/field
                    [declarator/name='e']
                    [type[name='Foo']]]
                [body/field
                    [declarator/name='l']
                    [type[name='List']]]
                [body/field
                    [declarator/name='owner']
                    [type[name='T']]]
                [body/field
                    [declarator/name='tags']
                    [type[name='List']
                        [generic]
                        [type[name='String']]]]
        "#),
        1);
}

#[test]
fn rust_vocabulary() {
    let mut tree = parse_src("rust", r#"
        use std::collections::HashMap;

        trait Barker { fn bark(&self); }

        struct Dog<T: Barker> {
            owner: T,
            tags: Vec<String>,
            scores: HashMap<String, i32>,
            parent: Option<Box<Dog<T>>>,
        }

        fn make(x: i32) -> String { String::new() }
    "#);

    claim("every <type> has a <name> child",
        &mut tree, "//type[not(name)]", 0);

    claim("Dog struct shape keeps generic bounds and nested generic field types",
        &mut tree,
        &multi_xpath(r#"
            //struct[name='Dog']
                [generic[name='T']
                    [bounds/type[name='Barker']]]
                [body/field[name='owner']
                    [type[name='T']]]
                [body/field[name='tags']
                    [type[name='Vec']
                        [generic]
                        [type[name='String']]]]
                [body/field[name='scores']
                    [type[name='HashMap']
                        [generic]
                        [type[name='String']]
                        [type[name='i32']]]]
                [body/field[name='parent']
                    [type[name='Option']
                        [generic]
                        [type[name='Box']
                            [generic]
                            [type[name='Dog']
                                [generic]
                                [type[name='T']]]]]]
        "#),
        1);

    claim("make() keeps parameter and return type names",
        &mut tree,
        &multi_xpath(r#"
            //function[name='make']
                [parameter
                    [name='x']
                    [type[name='i32']]]
                [returns/type[name='String']]
        "#),
        1);
}

#[test]
fn typescript_vocabulary() {
    let mut tree = parse_src("typescript", r#"
        type Id = number;
        type Handler = (x: number) => void;
        type Box<T> = Array<T>;

        class Animal {}
        interface Barker { bark(): void; }
        class Dog extends Animal implements Barker {
            bark(): void {}
        }

        function f(x: number): string { return ""; }
    "#);

    claim("only <type[function]> may lack a <name> (it's defined by signature)",
        &mut tree, "//type[not(name) and not(function)]", 0);

    claim("TypeScript program shape keeps aliases, class relations, and function types visible",
        &mut tree,
        &multi_xpath(r#"
            //program
                [alias[name='Id']
                    [type[name='number']]]
                [alias[name='Handler']
                    [type
                        [function]]]
                [alias[name='Box']
                    [generics/generic[name='T']]
                    [type[name='Array']
                        [generic]
                        [type[name='T']]]]
                [class[name='Dog']
                    [extends/type[name='Animal']]
                    [implements/type[name='Barker']]]
                [function[name='f']
                    [parameter
                        [name='x']
                        [type[name='number']]]
                    [returns/type[name='string']]]
        "#),
        1);

    claim("generic-alias type parameter has <name> child holding T (not nested type)",
        &mut tree, "//generic[name='T']", 1);

    claim("no spurious <type> wrapper inside the <name> of a generic",
        &mut tree, "//generic/name/type", 0);
}

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

    claim("Rust type marker shapes stay attached to their containing signatures",
        &mut tree,
        &multi_xpath(r#"
            //file
                [function[name='f']
                    [parameter
                        [name='cb']
                        [type[function]]]
                    [parameter
                        [name='t']
                        [type[tuple]]]
                    [parameter
                        [name='a']
                        [type[array]]]
                    [parameter
                        [name='p']
                        [type[pointer]]]
                    [returns/type[never]]]
                [function[name='g']
                    [returns/type[unit]]]
                [function[name='h']
                    [parameter
                        [name='d']
                        [type[borrowed]
                            [type[dynamic]]]]]
        "#),
        1);
}

/// C# type flavors — array/tuple/nullable — all collapse to
/// <type> with a shape marker. `nullable_type` gets its
/// <nullable/> marker via a direct rewrite (not the map) but the
/// end shape is the same.
#[test]
fn csharp_markers() {
    let mut tree = parse_src("csharp", r#"
        class X {
            int[] a;
            (int, string) t;
            int? n;
        }
    "#);

    claim("C# type marker shapes stay attached to their field declarations",
        &mut tree,
        &multi_xpath(r#"
            //class[name='X']/body
                [field
                    [variable/declarator/name='a']
                    [variable/type[array]]]
                [field
                    [variable/declarator/name='t']
                    [variable/type[tuple]]]
                [field
                    [variable/declarator/name='n']
                    [variable/type[nullable]]]
        "#),
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

    claim("TypeScript type marker shapes stay attached to aliases",
        &mut tree,
        &multi_xpath(r#"
            //program
                [alias[name='A']
                    [type[union]]]
                [alias[name='B']
                    [type[intersection]]]
                [alias[name='C']
                    [type[tuple]]]
                [alias[name='D']
                    [type[array]]]
                [alias[name='E']
                    [type[literal]]]
                [alias[name='F']
                    [type[function]]]
                [alias[name='G']
                    [type[object]]]
                [alias[name='H']
                    [type[readonly]
                        [type[array]]]]
        "#),
        1);
}

/// Java `void` carries an additional <void/> marker on top of the
/// `<name>void</name>` text leaf — the marker is a query
/// shortcut, not a replacement. Other primitives keep just the
/// name child.
#[test]
fn java_markers() {
    let mut tree = parse_src("java", r#"
        class X {
            void f() {}
            int g() { return 0; }
        }
    "#);

    claim("Java method return types distinguish void marker from named primitive",
        &mut tree,
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

    claim("Rust borrowed type shape keeps mutability, lifetime, and nested referent types",
        &mut tree,
        &multi_xpath(r#"
            //file
                [function[name='read']
                    [parameter
                        [name='s']
                        [type[borrowed]
                            [type[name='str']]]]
                    [returns/type[borrowed]
                        [type[name='str']]]]
                [function[name='write']
                    [parameter
                        [name='buf']
                        [type[borrowed]
                            [mut]
                            [type[name='Vec']
                                [generic]
                                [type[name='u8']]]]]]
                [function[name='static_ref']
                    [returns/type[borrowed]
                        [lifetime[name='static']]
                        [type[name='str']]]]
        "#),
        1);

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

    claim("Rust aliases expose visibility, names, generic parameters, and target types",
        &mut tree,
        &multi_xpath(r#"
            //file
                [alias[name='Id']
                    [private]
                    [type[name='u32']]]
                [alias[name='Mapping']
                    [private]
                    [generic[name='T']]
                    [type[name='std::collections::HashMap']
                        [generic]
                        [type[name='String']]
                        [type[name='T']]]]
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

    claim("Go file shape distinguishes defined types from aliases",
        &mut tree,
        &multi_xpath(r#"
            //file
                [type[name='MyInt']
                    [type[name='int']]]
                [alias[name='Color']
                    [type[name='int']]]
        "#),
        1);

    claim("alias does NOT also render as <type> at the top level",
        &mut tree, "//file/type[name='Color']", 0);
}

// ---- where_clause ---------------------------------------------------------

/// C# `where` clause constraints attach to the matching
/// <generic> element. Shape constraints (class / struct /
/// notnull / unmanaged / new) become empty markers that
/// compose; type bounds wrap in <extends><type>…</type></extends>.
#[test]
fn csharp_where() {
    let mut tree = parse_src("csharp", r#"
        using System;

        class Repo<T, U, V>
            where T : class, IComparable<T>, new()
            where U : struct
            where V : notnull
        {
        }
    "#);

    claim("C# where constraints attach to the matching generic parameters",
        &mut tree,
        &multi_xpath(r#"
            //class[name='Repo']
                [generic[name='T']
                    [class]
                    [new]
                    [extends/type[name='IComparable']
                        [generic]
                        [type[name='T']]]]
                [generic[name='U']
                    [struct]
                    [not(extends)]]
                [generic[name='V']
                    [notnull]]
        "#),
        1);
}
