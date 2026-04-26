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

    claim("Dog declares one <generic> type parameter",
        &mut tree, "//class[name='Dog']/generic[name='T']", 1);

    claim("generic T with where-clause `: Animal` exposes <extends><type>",
        &mut tree, "//class[name='Dog']/generic[name='T']/extends/type[name='Animal']", 1);

    claim("class extends list combines base + interface as siblings",
        &mut tree, "//class[name='Dog']/extends/type[name='Animal' or name='IBarker']", 2);

    claim("List<string> field uses generic type with inner <type>",
        &mut tree, "//field//type[generic][name='List']/type[name='string']", 1);
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

    claim("type parameter T has an <extends> bound on Animal",
        &mut tree, "//class[name='Dog']/generic[name='T']/extends/type[name='Animal']", 1);

    claim("extends list points to <type[name='Animal']>",
        &mut tree, "//class[name='Dog']/extends/type[name='Animal']", 1);

    claim("implements list has 2 <type> entries",
        &mut tree, "//class[name='Dog']/implements/type", 2);

    claim("List<String> field uses generic type with inner <type>",
        &mut tree, "//field//type[generic][name='List']/type[name='String']", 1);

    claim("primitive `int` carries name as text",
        &mut tree, "//type[name='int']", 1);

    claim("primitive `double` carries name as text",
        &mut tree, "//type[name='double']", 1);

    claim("primitive `boolean` carries name as text",
        &mut tree, "//type[name='boolean']", 1);

    claim("user-defined type `Foo` carries name as text",
        &mut tree, "//type[name='Foo']", 1);

    claim("built-in capitalized type `List` carries name as text (bare + generic forms)",
        &mut tree, "//type[name='List']", 2);
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

    claim("Dog declares <generic> with a `: Barker` bound",
        &mut tree, "//struct[name='Dog']/generic[name='T']/bounds/type[name='Barker']", 1);

    claim("Vec<String>: generic with inner <type>",
        &mut tree, "//field[name='tags']/type[generic][name='Vec']/type[name='String']", 1);

    claim("HashMap<String, i32>: generic with two inner <type> children",
        &mut tree, "//field[name='scores']/type[generic][name='HashMap']/type", 2);

    claim("Option<Box<Dog<T>>> nests 3 levels of <type[generic]>",
        &mut tree, "//field[name='parent']/type[generic]/type[generic]/type[generic]", 1);

    claim("parameter type wraps name in <name>",
        &mut tree, "//parameter/type[name='i32']", 1);

    claim("return type wraps name in <name>",
        &mut tree, "//returns/type[name='String']", 1);
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

    claim("plain alias points at a single <type>",
        &mut tree, "//alias[name='Id']/type[name='number']", 1);

    claim("function-type alias carries <type[function]>",
        &mut tree, "//alias[name='Handler']/type[function]", 1);

    claim("generic alias carries a <generic> child via <generics> wrapper",
        &mut tree, "//alias[name='Box']/generics/generic[name='T']", 1);

    claim("Dog extends and implements both wrap base types",
        &mut tree, "//class[name='Dog']/extends/type[name='Animal'] | //class[name='Dog']/implements/type[name='Barker']", 2);

    claim("function declaration's parameter type wraps name in <name>",
        &mut tree, "//function[name='f']/parameter/type[name='number']", 1);

    claim("function declaration's return type wraps name in <name>",
        &mut tree, "//function[name='f']/returns/type[name='string']", 1);

    claim("generic-alias type parameter has <name> child holding T (not nested type)",
        &mut tree, "//generic[name='T']", 1);

    claim("no spurious <type> wrapper inside the <name> of a generic",
        &mut tree, "//generic/name/type", 0);

    claim("no raw `function_type` kind leak (renamed to <type[function]>)",
        &mut tree, "//function_type", 0);
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

    claim("fn type carries <function/>",
        &mut tree, "//type[function]", 1);

    claim("tuple type carries <tuple/>",
        &mut tree, "//type[tuple]", 1);

    claim("array type carries <array/>",
        &mut tree, "//type[array]", 1);

    claim("pointer type carries <pointer/>",
        &mut tree, "//type[pointer]", 1);

    claim("never type carries <never/>",
        &mut tree, "//type[never]", 1);

    claim("unit type carries <unit/>",
        &mut tree, "//type[unit]", 1);

    claim("dyn trait object carries <dynamic/>",
        &mut tree, "//type[dynamic]", 1);
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

    claim("array type carries <array/>",
        &mut tree, "//type[array]", 1);

    claim("tuple type carries <tuple/>",
        &mut tree, "//type[tuple]", 1);

    claim("nullable type carries <nullable/>",
        &mut tree, "//type[nullable]", 1);
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

    claim("union type carries <union/>",
        &mut tree, "//type[union]", 1);

    claim("intersection type carries <intersection/>",
        &mut tree, "//type[intersection]", 1);

    claim("tuple type carries <tuple/>",
        &mut tree, "//type[tuple]", 1);

    // `number[]` is array_type; `readonly number[]` wraps in readonly_type.
    claim("array types carry <array/> (number[] + readonly number[])",
        &mut tree, "//type[array]", 2);

    claim("literal type carries <literal/>",
        &mut tree, "//type[literal]", 1);

    claim("function type carries <function/>",
        &mut tree, "//type[function]", 1);

    claim("object type carries <object/>",
        &mut tree, "//type[object]", 1);

    claim("readonly type carries <readonly/>",
        &mut tree, "//type[readonly]", 1);
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

    claim("void type has both <void/> marker AND <name>void</name>",
        &mut tree, "//type[void][name='void']", 1);

    claim("exactly one void type in the source",
        &mut tree, "//type[void]", 1);

    claim("non-void types have no <void/> marker",
        &mut tree, "//type[not(void)]", 1);
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

    claim("4 reference types: 2x &str (param + return) + &mut Vec<u8> + &'static str",
        &mut tree, "//type[borrowed]", 4);

    claim("only the &mut Vec<u8> carries the mut marker",
        &mut tree, "//type[borrowed and mut]", 1);

    claim("borrowed type wraps the referenced type as a nested <type>",
        &mut tree, "//type[borrowed]/type", 4);

    claim("`&'static` exposes a <lifetime> child",
        &mut tree, "//type[borrowed]/lifetime[name='static']", 1);

    claim("inner type of &mut is the generic Vec<u8>",
        &mut tree, "//type[borrowed and mut]/type[generic][name='Vec']", 1);

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

    claim("two aliases declared",
        &mut tree, "//alias", 2);

    claim("no raw `type_item` grammar leaf leaks",
        &mut tree, "//type_item", 0);

    claim("aliases default to <private/>",
        &mut tree, "//alias[private]", 2);

    claim("simple alias resolves to <type>",
        &mut tree, "//alias[name='Id']/type[name='u32']", 1);

    claim("generic alias declares a <generic> parameter",
        &mut tree, "//alias[name='Mapping']/generic[name='T']", 1);

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

    claim("defined type renders as <type>",
        &mut tree, "//type[name='MyInt']", 1);

    claim("alias renders as <alias>",
        &mut tree, "//alias[name='Color']", 1);

    claim("alias inner refers to underlying <type>",
        &mut tree, "//alias[name='Color']/type[name='int']", 1);

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

    claim("3 generics declared on Repo (T, U, V)",
        &mut tree, "//class[name='Repo']/generic", 3);

    claim("T composes class + new shape markers",
        &mut tree, "//generic[class and new][name='T']", 1);

    claim("U has the struct constraint",
        &mut tree, "//generic[struct][name='U']", 1);

    claim("V has the notnull constraint",
        &mut tree, "//generic[notnull][name='V']", 1);

    claim("T's IComparable<T> bound wraps in <extends><type>...",
        &mut tree, "//generic[name='T']/extends/type[name='IComparable']", 1);

    claim("U has no <extends> bound",
        &mut tree, "//generic[name='U']/extends", 0);
}
