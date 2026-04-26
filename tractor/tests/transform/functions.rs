//! Cross-language: functions, methods, arrow forms, async/generator
//! markers, constructor naming, method calls, and parameter shape
//! markers.

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
                [body/binary]
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

// ---- method_call ----------------------------------------------------------

/// Both function calls and method calls render as <call>. Method
/// calls are distinguished by a <field> child that names the
/// receiver and method (Rust uses field-call syntax).
#[test]
fn rust_method_call() {
    claim("Rust associated function call keeps path callee shape",
        &mut parse_src("rust", r#"
        fn use_calls() {
            let v: Vec<i32> = Vec::new();
        }
    "#),
        &multi_xpath(r#"
            //call/path
                [name='Vec']
                [name='new']
        "#),
        1);

    claim("Rust field-call callee carries receiver and method name",
        &mut parse_src("rust", r#"
        fn use_calls(v: Vec<i32>) {
            let n = v.len();
        }
    "#),
        &multi_xpath(r#"
            //call/field
                [value/name='v']
                [name='len']
        "#),
        1);

    claim("Rust field-call callee can use a literal receiver",
        &mut parse_src("rust", r#"
        fn use_calls() {
            let s = "hi".to_string();
        }
    "#),
        &multi_xpath(r#"
            //call/field
                [value/string]
                [name='to_string']
        "#),
        1);

    claim("Rust expression statement method call keeps field-call shape",
        &mut parse_src("rust", r#"
        fn use_calls(s: String) {
            s.to_uppercase();
        }
    "#),
        &multi_xpath(r#"
            //call/field
                [value/name='s']
                [name='to_uppercase']
        "#),
        1);
}

/// Java `this(…)` / `super(…)` in constructors render as <call>
/// with a <this/> or <super/> marker — uniform with other call
/// sites; no `explicit_constructor_invocation` raw kind leaks.
#[test]
fn java_method_call() {
    claim("Java this constructor invocation renders as call[this]",
        &mut parse_src("java", r#"
        class X {
            X() { this(1); }
            X(int a) {}
        }
    "#),
        "//call[this]",
        1);

    claim("Java super constructor invocation renders as call[super]",
        &mut parse_src("java", r#"
        class X {
            X(int a) {}
            class Y extends X {
                Y() { super(2); }
            }
        }
    "#),
        "//call[super]",
        1);
}

// ---- parameter_marking ----------------------------------------------------

/// Every <param> carries an exhaustive marker: <required/> or
/// <optional/>. Covers required, optional (?), defaulted, and
/// rest parameters; also the JS-style untyped param shape.
#[test]
fn typescript_parameter_marking() {
    claim("required TypeScript parameter carries required marker and type",
        &mut parse_src("typescript", "function call(required: string): void {}\n"),
        &multi_xpath(r#"
            //parameter[name='required']
                [required]
                [type/name='string']
        "#),
        1);

    claim("optional TypeScript parameter carries optional marker and type",
        &mut parse_src("typescript", "function call(optional?: number): void {}\n"),
        &multi_xpath(r#"
            //parameter[name='optional']
                [optional]
                [type/name='number']
        "#),
        1);

    claim("defaulted TypeScript parameter remains required and has value",
        &mut parse_src("typescript", "function call(defaulted: boolean = true): void {}\n"),
        &multi_xpath(r#"
            //parameter[name='defaulted']
                [required]
                [value]
        "#),
        1);

    claim("rest TypeScript parameter carries required and rest markers",
        &mut parse_src("typescript", "function call(...rest: string[]): void {}\n"),
        &multi_xpath(r#"
            //parameter[rest/name='rest']
                [required]
                [rest]
        "#),
        1);

    claim("untyped noTypes parameters are still required parameters",
        &mut parse_src("typescript", "function noTypes(x, y) {}\n"),
        "//function[name='noTypes']/parameter[required]",
        2);
}

/// Ruby splat parameters distinguish iterable `*args` (list) from
/// mapping `**kwargs` (dict); keyword parameters (`key:`) carry a
/// <keyword/> marker distinguishing them from positional ones.
#[test]
fn ruby_parameter_marking() {
    claim("Ruby splat parameter carries list spread marker",
        &mut parse_src("ruby", "def f(*xs)\nend\n"), "//spread[list][name='xs']", 1);

    claim("Ruby keyword parameter carries keyword marker and default value",
        &mut parse_src("ruby", "def f(key: 1)\nend\n"),
        &multi_xpath(r#"
            //parameter[name='key']
                [keyword]
                [value]
        "#),
        1);

    claim("Ruby kwargs parameter carries dict spread marker",
        &mut parse_src("ruby", "def f(**kw)\nend\n"), "//spread[dict][name='kw']", 1);
}
