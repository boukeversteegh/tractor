//! Cross-language: functions, methods, arrow forms, async/generator
//! markers, constructor naming, method calls, and parameter shape
//! markers.

use crate::support::semantic::*;

// ---- arrow_function -------------------------------------------------------

/// Principle #5 — `arrow_function` renames to <arrow> (JS-native
/// vocabulary; distinct from <function> declarations).
#[test]
fn typescript_arrow() {
    let mut tree = parse_src("typescript", r#"
        const f = (x: number) => x + 1;
    "#);

    claim("const f shape has an arrow value with parameter and expression body",
        &mut tree,
        &multi_xpath(r#"
            //variable[name='f']
                [const]
                [value/arrow
                    [parameter[name='x']
                        [required]
                        [type/name='number']
                    ]
                    [body/binary]
                ]
        "#),
        1);
}

// ---- async_generator ------------------------------------------------------

/// async / generator lift to empty markers on <function> /
/// <method>. Every async/generator declaration carries the
/// applicable markers (Principle #9 exhaustive markers).
#[test]
fn typescript_async_generator() {
    let mut tree = parse_src("typescript", r#"
        async function fetchOne(): Promise<number> { return 1; }
        function* counter(): Generator<number> { yield 1; }
        async function* stream(): AsyncGenerator<number> { yield 1; }
        class Service {
            async load(): Promise<void> {}
            *keys(): Generator<string> { yield "a"; }
        }
    "#);

    claim("fetchOne shape is async function with no generator marker",
        &mut tree,
        &multi_xpath(r#"
            //function[name='fetchOne']
                [async]
                [not(generator)]
        "#),
        1);

    claim("counter shape is generator function with no async marker",
        &mut tree,
        &multi_xpath(r#"
            //function[name='counter']
                [generator]
                [not(async)]
        "#),
        1);

    claim("stream shape composes async and generator markers",
        &mut tree,
        &multi_xpath(r#"
            //function[name='stream']
                [async]
                [generator]
        "#),
        1);

    claim("load shape is async method with no generator marker",
        &mut tree,
        &multi_xpath(r#"
            //method[name='load']
                [async]
                [not(generator)]
        "#),
        1);

    claim("keys shape is generator method with no async marker",
        &mut tree,
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
    let mut tree = parse_src("java", r#"
        class Point {
            int x, y;
            Point() { this(0, 0); }
            Point(int x, int y) { this.x = x; this.y = y; }
        }
    "#);

    claim("Point constructor shapes include zero-arg chain call and two-arg initializer",
        &mut tree,
        &multi_xpath(r#"
            //class[name='Point']/body
                [constructor[name='Point']
                    [not(parameter)]
                    [body//call[this]]
                ]
                [constructor[name='Point']
                    [count(parameter)=2]
                ]
                [count(constructor)=2]
        "#),
        1);

    claim("no abbreviated `ctor` element leaks",
        &mut tree, "//ctor", 0);
}

// ---- method_call ----------------------------------------------------------

/// Both function calls and method calls render as <call>. Method
/// calls are distinguished by a <field> child that names the
/// receiver and method (Rust uses field-call syntax).
#[test]
fn rust_method_call() {
    let mut tree = parse_src("rust", r#"
        fn use_calls() {
            let v: Vec<i32> = Vec::new();
            let n = v.len();
            let s = "hi".to_string();
            s.to_uppercase();
        }
    "#);

    claim("use_calls body has one path call and three field-call shapes",
        &mut tree,
        &multi_xpath(r#"
            //function[name='use_calls']/body
                [.//call/path
                    [name='Vec']
                    [name='new']]
                [.//call/field
                    [value/name='v']
                    [name='len']]
                [.//call/field
                    [value/string]
                    [name='to_string']]
                [.//call/field
                    [value/name='s']
                    [name='to_uppercase']]
                [count(.//call)=4]
        "#),
        1);

    claim("no legacy <methodcall> element",
        &mut tree, "//methodcall", 0);
}

/// Java `this(…)` / `super(…)` in constructors render as <call>
/// with a <this/> or <super/> marker — uniform with other call
/// sites; no `explicit_constructor_invocation` raw kind leaks.
#[test]
fn java_method_call() {
    let mut tree = parse_src("java", r#"
        class X {
            X() { this(1); }
            X(int a) {}
            class Y extends X {
                Y() { super(2); }
            }
        }
    "#);

    claim("constructor chain calls use call[this] and call[super] shapes",
        &mut tree,
        &multi_xpath(r#"
            //class[name='X']/body
                [constructor[name='X']
                    [not(parameter)]
                    [body//call[this]]
                ]
                [class[name='Y']
                    [body/constructor[name='Y']
                        [body//call[super]]]
                ]
        "#),
        1);
}

// ---- parameter_marking ----------------------------------------------------

/// Every <param> carries an exhaustive marker: <required/> or
/// <optional/>. Covers required, optional (?), defaulted, and
/// rest parameters; also the JS-style untyped param shape.
#[test]
fn typescript_parameter_marking() {
    let mut tree = parse_src("typescript", r#"
        function call(
            required: string,
            optional?: number,
            defaulted: boolean = true,
            ...rest: string[]
        ): void {}

        function noTypes(x, y) {}
    "#);

    claim("call parameter list covers required, optional, defaulted, and rest shapes",
        &mut tree,
        &multi_xpath(r#"
            //function[name='call']
                [parameter[name='required']
                    [required]
                    [type/name='string']
                ]
                [parameter[name='optional']
                    [optional]
                    [type/name='number']
                ]
                [parameter[name='defaulted']
                    [required]
                    [value]
                ]
                [parameter[rest/name='rest']
                    [required]
                    [rest]
                ]
                [count(parameter)=4]
        "#),
        1);

    claim("untyped noTypes parameters are still required parameters",
        &mut tree,
        &multi_xpath(r#"
            //function[name='noTypes']
                [parameter[name='x']
                    [required]
                ]
                [parameter[name='y']
                    [required]
                ]
                [count(parameter)=2]
        "#),
        1);

    claim("required and optional markers are exhaustive and mutually exclusive",
        &mut tree, "//parameter[(not(required) and not(optional)) or (required and optional)]", 0);
}

/// Ruby splat parameters distinguish iterable `*args` (list) from
/// mapping `**kwargs` (dict); keyword parameters (`key:`) carry a
/// <keyword/> marker distinguishing them from positional ones.
#[test]
fn ruby_parameter_marking() {
    let mut tree = parse_src("ruby", r#"
        def f(a, *xs, key: 1, **kw)
        end
    "#);

    claim("Ruby parameter shape covers positional, splat, keyword, and kwargs",
        &mut tree,
        &multi_xpath(r#"
            //method[name='f']
                [name='a']
                [spread[list][name='xs']]
                [parameter[name='key']
                    [keyword]
                    [value]
                ]
                [spread[dict][name='kw']]
        "#),
        1);
}
