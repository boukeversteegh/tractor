//! Cross-language: generic type vocabulary, type parameters, bounds,
//! `where`-clause constraints, and inheritance/implementation
//! relations attached to generic declarations.
//!
//! Principle #14 grounds the shape: every type reference wraps its
//! name in a <name> child; type parameters render as <generic> with
//! optional <bounds>/<extends>; collection-of-T uses <type[generic]>
//! with nested <type> children.

use crate::support::semantic::*;

// ---- vocabulary -----------------------------------------------------------

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
                [extends/type[name='Animal']]
                [extends/type[name='IBarker']]
                [body/field
                    [name='Owner']
                    [type[name='T']]
                    [not(variable)]]
                [body/field
                    [name='Tags']
                    [type[name='List']
                        [generic]
                        [type[name='string']]]
                    [not(variable)]]
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
                [implements/type[name='Barker']]
                [implements/type[name='Runner']]
                [body/field
                    [name='a']
                    [type[name='int']]]
                [body/field
                    [name='b']
                    [type[name='double']]]
                [body/field
                    [name='c']
                    [type[name='boolean']]]
                [body/field
                    [name='e']
                    [type[name='Foo']]]
                [body/field
                    [name='l']
                    [type[name='List']]]
                [body/field
                    [name='owner']
                    [type[name='T']]]
                [body/field
                    [name='tags']
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
                    [extends/type[name='Barker']]]
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

    claim("TypeScript plain alias points at a single named type",
        &mut tree, "//alias[name='Id']/type[name='number']", 1);

    claim("TypeScript function-type alias carries function marker",
        &mut tree, "//alias[name='Handler']/type[function]", 1);

    claim("TypeScript generic alias keeps generic parameter and target type arguments",
        &mut tree,
        &multi_xpath(r#"
            //alias[name='Box']
                [generic[name='T']]
                [type[name='Array']
                    [generic]
                    [type[name='T']]]
        "#),
        1);

    claim("TypeScript class relation wrappers expose base types",
        &mut tree,
        "//class[name='Dog'][extends/type[name='Animal']][implements/type[name='Barker']]",
        1);

    claim("TypeScript function signature wraps parameter and return types",
        &mut tree,
        &multi_xpath(r#"
            //function[name='f']
                [parameter
                    [name='x']
                    [type[name='number']]]
                [returns/type[name='string']]
        "#),
        1);

    claim("generic-alias type parameter has <name> child holding T (not nested type)",
        &mut tree, "//generic[name='T']", 1);

    claim("no spurious <type> wrapper inside the <name> of a generic",
        &mut tree, "//generic/name/type", 0);
}

// ---- where_clause ---------------------------------------------------------

/// C# `where` clause constraints attach to the matching
/// <generic> element. Shape constraints (class / struct /
/// notnull / unmanaged / new) become empty markers that
/// compose; type bounds wrap in <extends><type>…</type></extends>.
#[test]
fn csharp_where() {
    claim("C# where constraints attach to the matching generic parameters",
        &mut parse_src("csharp", r#"
        using System;

        class Repo<T, U, V>
            where T : class, IComparable<T>, new()
            where U : struct
            where V : notnull
        {
        }
    "#),
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

/// Rust `impl Trait for Type` distinguishes the trait position from
/// the impl-target position via an `<implements>` slot wrapper —
/// matches Java/TS/C# vocabulary (Principle #5). Inherent
/// `impl Type {}` blocks lack the wrapper. Custom handler scopes
/// the wrap to `impl_item` so other tree-sitter `field="trait"`
/// uses (`dyn Trait` etc.) are unaffected.
#[test]
fn rust_impl_implements_slot() {
    claim("Rust `impl Drawable for Point` wraps trait in <implements>",
        &mut parse_src("rust", "impl Drawable for Point {}"),
        "//impl[implements/type/name='Drawable'][type/name='Point']",
        1);

    claim("Rust inherent `impl Point` keeps just the type (no <implements>)",
        &mut parse_src("rust", "impl Point {}"),
        "//impl[type/name='Point'][not(implements)]",
        1);

    claim("Rust `dyn Trait` uses `<type[dynamic]>` — NOT wrapped in <implements>",
        &mut parse_src("rust", "fn f() -> Box<dyn Drawable> { todo!() }"),
        "//type[dynamic]/implements",
        0);
}

/// Rust associated-type bindings `Drawable<Canvas = Vec<u8>>` reuse
/// the `<type[associated]>` shape from the trait declaration site
/// (`type Canvas;`) per Principle #5 — same concept, same name. The
/// use-site adds a `<type>` child for the bound value; the
/// declaration-site has only a `<name>`. Without this shape the
/// trait name and the binding key would both surface as `<name>`
/// siblings under `<type[generic]>`, colliding on the singleton
/// JSON `name` key and overflowing the second into `children`.
#[test]
fn rust_associated_type_binding() {
    claim("Rust trait declaration site `type Canvas;` is <type[associated]>",
        &mut parse_src("rust", "trait T { type Canvas; }"),
        "//trait/body/type[associated][name='Canvas'][not(type)]",
        1);

    claim("Rust use-site binding `Drawable<Canvas = Vec<u8>>` wraps key+value in <type[associated]>",
        &mut parse_src("rust", "fn f(d: &dyn Drawable<Canvas = Vec<u8>>) {}"),
        &multi_xpath(r#"
            //type[generic][name='Drawable']
                /type[associated]
                    [name='Canvas']
                    [type[generic][name='Vec']/type[name='u8']]
        "#),
        1);

    claim("trait name no longer collides with binding key as bare <name> siblings",
        &mut parse_src("rust", "fn f(d: &dyn Drawable<Canvas = Vec<u8>>) {}"),
        "//type[generic][name='Drawable']/name",
        1);
}
