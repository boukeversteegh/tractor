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

    claim("no parameter-list wrapper element",
        &mut tree, "//parameter_list | //parameters", 0);

    claim("no argument-list wrapper element",
        &mut tree, "//argument_list | //arguments", 0);

    claim("no accessor-list wrapper element",
        &mut tree, "//accessor_list", 0);

    claim("First has 3 parameters as direct siblings",
        &mut tree, "//method[name='First']/parameter", 3);

    claim("First has 2 generics as direct siblings",
        &mut tree, "//method[name='First']/generic", 2);

    claim("Property accessors are direct siblings of <property>",
        &mut tree, "//property[name='Count']/accessor", 2);
}

#[test]
fn go() {
    let mut tree = parse_src("go", r#"
        package main

        func First(a string, b int, c bool) string { return a }

        func Caller() { First("x", 1, true) }

        type Config struct { Host string; Port int; Tls bool }
    "#);

    claim("no parameter-list wrapper",
        &mut tree, "//parameter_list | //parameters", 0);

    claim("no argument-list wrapper",
        &mut tree, "//argument_list | //arguments", 0);

    claim("no field-list wrapper around struct fields",
        &mut tree, "//field_declaration_list | //field_list", 0);

    claim("First has 3 parameters as direct siblings",
        &mut tree, "//function[name='First']/parameter", 3);

    claim("Caller's call has 3 argument siblings (not wrapped)",
        &mut tree, "//function[name='Caller']//call/*[self::string or self::int or self::true]", 3);

    claim("Config struct has 3 fields",
        &mut tree, "//struct[name='Config']/field", 3);
}

#[test]
fn java() {
    let mut tree = parse_src("java", r#"
        class FlatLists {
            <T, U extends Comparable<U>> T first(T a, U b, int c) { return a; }

            void caller() { first("x", "y", 1); }
        }
    "#);

    claim("no parameter-list wrapper",
        &mut tree, "//parameter_list | //parameters", 0);

    claim("no type-parameter wrapper",
        &mut tree, "//type_parameters", 0);

    claim("no argument-list wrapper",
        &mut tree, "//argument_list | //arguments", 0);

    claim("first has 3 parameters as direct siblings",
        &mut tree, "//method[name='first']/parameter", 3);

    claim("first has 2 generics as direct siblings",
        &mut tree, "//method[name='first']/generic", 2);
}

#[test]
fn rust() {
    let mut tree = parse_src("rust", r#"
        fn first<T, U: Clone>(a: T, b: U, c: i32) -> T { a }

        fn caller() {
            first::<String, i32>(String::from("x"), 1, 2);
        }
    "#);

    claim("no parameter-list wrapper",
        &mut tree, "//parameters | //parameter_list", 0);

    claim("no type-parameter wrapper",
        &mut tree, "//type_parameters", 0);

    claim("no argument-list wrapper",
        &mut tree, "//argument_list | //arguments", 0);

    claim("first has 3 parameters as direct siblings",
        &mut tree, "//function[name='first']/parameter", 3);

    claim("first has 2 generics as direct siblings",
        &mut tree, "//function[name='first']/generic", 2);
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

    claim("no parameter-list wrapper",
        &mut tree, "//parameters | //parameter_list", 0);

    claim("no argument-list wrapper",
        &mut tree, "//argument_list | //arguments", 0);

    claim("first has 3 parameters as direct siblings",
        &mut tree, "//function[name='first']/parameter", 3);

    claim("TS keeps a thin <generics> wrapper for type parameters",
        &mut tree, "//function[name='first']/generics/generic", 2);
}
