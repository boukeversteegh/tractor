//! Cross-language: Principle #12 — parameters / arguments /
//! generics / accessors render as flat siblings — no <parameters> /
//! <accessor_list> / <argument_list> / <type_parameters> wrapper
//! element.

use crate::support::semantic::*;

#[test]
fn csharp() {
    let mut tree = parse_src("csharp", r#"
        class FlatLists
        {
            public T First<T, U>(T a, U b, int c) where T : class
            {
                return a;
            }

            public int Count { get; set; }

            public void Caller()
            {
                First<string, int>("x", 1, 2);
            }
        }
    "#);

    claim("C# method uses flat parameter and generic siblings",
        &mut tree,
        &multi_xpath(r#"
            //method[name='First']
                [count(parameter)=3]
                [count(generic)=2]
        "#),
        1);

    claim("C# property uses flat accessor siblings",
        &mut tree,
        &multi_xpath(r#"
            //property[name='Count']
                [get]
                [set]
        "#),
        1);

    claim("C# flat-list grammar wrappers do not leak",
        &mut tree, "//parameter_list | //parameters | //argument_list | //arguments | //accessor_list", 0);
}

#[test]
fn go() {
    let mut tree = parse_src("go", r#"
        package main

        func First(a string, b int, c bool) string { return a }

        func Caller() { First("x", 1, true) }

        type Config struct { Host string; Port int; Tls bool }
    "#);

    claim("Go function uses flat parameter siblings",
        &mut tree,
        &multi_xpath(r#"
            //function[name='First']
                [count(parameter)=3]
        "#),
        1);

    claim("Go call uses flat argument siblings",
        &mut tree, "//call[count(string|int|true)=3]", 1);

    claim("Go struct uses flat field siblings",
        &mut tree, "//struct[name='Config'][count(field)=3]", 1);

    claim("Go flat-list grammar wrappers do not leak",
        &mut tree, "//parameter_list | //parameters | //argument_list | //arguments | //field_declaration_list | //field_list", 0);
}

#[test]
fn java() {
    let mut tree = parse_src("java", r#"
        class FlatLists {
            <T, U extends Comparable<U>> T first(T a, U b, int c) { return a; }

            void caller() { first("x", "y", 1); }
        }
    "#);

    claim("Java generic method shape uses flat parameter and generic siblings",
        &mut tree,
        &multi_xpath(r#"
            //method[name='first']
                [count(parameter)=3]
                [count(generic)=2]
        "#),
        1);

    claim("Java flat-list grammar wrappers do not leak",
        &mut tree, "//parameter_list | //parameters | //type_parameters | //argument_list | //arguments", 0);
}

#[test]
fn rust() {
    let mut tree = parse_src("rust", r#"
        fn first<T, U: Clone>(a: T, b: U, c: i32) -> T { a }

        fn caller() {
            first::<String, i32>(String::from("x"), 1, 2);
        }
    "#);

    claim("Rust generic function shape uses flat parameter and generic siblings",
        &mut tree,
        &multi_xpath(r#"
            //function[name='first']
                [count(parameter)=3]
                [count(generic)=2]
        "#),
        1);

    claim("Rust flat-list grammar wrappers do not leak",
        &mut tree, "//parameters | //parameter_list | //type_parameters | //argument_list | //arguments", 0);
}

/// TypeScript currently retains a thin <generics> grouping
/// element — pin that as current behaviour. Parameters and
/// arguments still flatten.
#[test]
fn typescript() {
    let mut tree = parse_src("typescript", r#"
        function first<T, U>(a: T, b: U, c: number): T { return a; }
        first<string, number>("x", 1, 2);
    "#);

    claim("TypeScript function shape flattens parameters but keeps generics wrapper",
        &mut tree,
        &multi_xpath(r#"
            //function[name='first']
                [count(parameter)=3]
                [generics[count(generic)=2]]
        "#),
        1);

    claim("TypeScript parameter and argument grammar wrappers do not leak",
        &mut tree, "//parameters | //parameter_list | //argument_list | //arguments", 0);
}
