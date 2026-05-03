//! Cross-language: function/method/arrow/lambda declaration shapes,
//! async/generator markers, constructor naming.
//!
//! Parameter shape markers live in `parameters.rs`; call-site shapes
//! live in `calls.rs`.

use crate::support::semantic::*;

// ---- arrow_function -------------------------------------------------------

/// Principle #5 — `arrow_function` renames to <arrow> (JS-native
/// vocabulary; distinct from <function> declarations).
#[test]
fn typescript_arrow() {
    claim("TypeScript arrow node has parameter and expression body",
        &mut parse_src("typescript", r#"
        const f = (x: number) => x + 1;
    "#),
        &multi_xpath(r#"
            //arrow
                [parameter[name='x']
                    [required]
                    [type/name='number']]
                [value/expression/binary]
        "#),
        1);
}

// ---- async_generator ------------------------------------------------------

/// async / generator lift to empty markers on <function> /
/// <method>. Every async/generator declaration carries the
/// applicable markers (Principle #9 exhaustive markers).
#[test]
fn typescript_async_generator() {
    claim("fetchOne shape is async function with no generator marker",
        &mut parse_src("typescript", r#"
        async function fetchOne(): Promise<number> { return 1; }
    "#),
        &multi_xpath(r#"
            //function[name='fetchOne']
                [async]
                [not(generator)]
        "#),
        1);

    claim("counter shape is generator function with no async marker",
        &mut parse_src("typescript", r#"
        function* counter(): Generator<number> { yield 1; }
    "#),
        &multi_xpath(r#"
            //function[name='counter']
                [generator]
                [not(async)]
        "#),
        1);

    claim("stream shape composes async and generator markers",
        &mut parse_src("typescript", r#"
        async function* stream(): AsyncGenerator<number> { yield 1; }
    "#),
        &multi_xpath(r#"
            //function[name='stream']
                [async]
                [generator]
        "#),
        1);

    claim("load shape is async method with no generator marker",
        &mut parse_src("typescript", r#"
        class Service {
            async load(): Promise<void> {}
        }
    "#),
        &multi_xpath(r#"
            //method[name='load']
                [async]
                [not(generator)]
        "#),
        1);

    claim("keys shape is generator method with no async marker",
        &mut parse_src("typescript", r#"
        class Service {
            *keys(): Generator<string> { yield "a"; }
        }
    "#),
        &multi_xpath(r#"
            //method[name='keys']
                [generator]
                [not(async)]
        "#),
        1);
}

// ---- constructor_rename ---------------------------------------------------

/// `ctor` -> `<constructor>` (Principle #2: full names over
/// abbreviations).
#[test]
fn java_constructor_rename() {
    claim("zero-argument Java constructor is named constructor and contains this-call",
        &mut parse_src("java", r#"
        class Point {
            Point() { this(0, 0); }
            Point(int x, int y) {}
        }
    "#),
        &multi_xpath(r#"
            //constructor[name='Point']
                [not(parameter)]
                [body//call[this]]
        "#),
        1);

    claim("two-argument Java constructor is named constructor with flat parameters",
        &mut parse_src("java", r#"
        class Point {
            Point(int x, int y) {}
        }
    "#),
        &multi_xpath(r#"
            //constructor[name='Point']
                [count(parameter)=2]
        "#),
        1);
}

// ---- multi-value return ---------------------------------------------------

/// Languages that allow returning multiple values (Go's tuple
/// returns, Python's `return a, b`) produce `<return>` with multiple
/// `<expression>` sibling children. Per Principle #19 they are
/// role-uniform (each is a return value); tag with
/// `list="expressions"` so JSON renders as `expressions: [...]`
/// array. Single-return is left as a singleton.
#[test]
fn go_multi_value_return_lists_expressions() {
    let mut tree = parse_src("go", r#"
        package m
        func f() (int, error) { return 0, ErrNotFound }
        func g() int { return 42 }
    "#);

    claim("Go multi-value return tags each <expression> with list='expressions'",
        &mut tree,
        "//return/expression[@list='expressions']",
        2);

    claim("Go single-value return keeps singleton <expression> (no list= tagging)",
        &mut tree,
        "//return[count(expression)=1]/expression[not(@list)]",
        1);
}

#[test]
fn python_multi_value_return_lists_expressions() {
    // Companion to iter 265 — pinned here cross-language alongside
    // Go for the same archetype. Detail-level pin already in
    // transform/python/expression_list.rs.
    claim("Python multi-value return tags each <expression> with list='expressions'",
        &mut parse_src("python", "def f():\n    return a, b, c\n"),
        "//return/expression[@list='expressions']",
        3);
}
