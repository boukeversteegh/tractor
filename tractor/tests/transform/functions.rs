//! Cross-language: functions, methods, accessors, arrow forms,
//! async/generator markers, constructor naming, method calls, and
//! parameter shape markers.

use crate::support::semantic::*;

// ---- accessor_flattening --------------------------------------------------

/// Property accessors are direct siblings of <property>; no
/// <accessor_list> wrapper. Each accessor carries an empty marker
/// (<get/>/<set/>/<init/>) uniformly across auto-form and bodied
/// form (Principles #12, #13).
#[test]
fn csharp_accessor_flattening() {
    let mut tree = parse_src("csharp", r#"
        class Accessors
        {
            public int AutoProp { get; set; }

            private int _backing;
            public int Manual
            {
                get { return _backing; }
                set { _backing = value; }
            }

            public int ReadOnly { get; }
            public int WriteOnly { set { _backing = value; } }
        }
    "#);

    claim("no <accessor_list> wrapper anywhere",
        &mut tree, "//accessor_list", 0);

    claim("auto-form get + bodied get + read-only get",
        &mut tree, "//accessor[get]", 3);

    claim("auto-form set + bodied set + write-only set",
        &mut tree, "//accessor[set]", 3);

    claim("AutoProp has 2 accessors as direct siblings of <property>",
        &mut tree, "//property[name='AutoProp']/accessor", 2);

    claim("Manual property has bodied accessors with block bodies",
        &mut tree, "//property[name='Manual']/accessor/body/block", 2);

    claim("ReadOnly has only get",
        &mut tree, "//property[name='ReadOnly']/accessor[set]", 0);
}

// ---- accessors ------------------------------------------------------------

/// TypeScript `get foo()` / `set foo(v)` carry <get/>/<set/>
/// markers on <method>. //method[get] picks them out uniformly
/// regardless of body shape.
#[test]
fn typescript_accessors() {
    let mut tree = parse_src("typescript", r#"
        class Counter {
            private _value = 0;

            get value(): number { return this._value; }
            set value(v: number) { this._value = v; }
            static get singleton(): Counter { return new Counter(); }
        }
    "#);

    claim("two getter methods (instance + static)",
        &mut tree, "//method[get]", 2);

    claim("one setter method",
        &mut tree, "//method[set]", 1);

    claim("get/set on accessor methods imply <public/>",
        &mut tree, "//method[(get or set) and not(public)]", 0);

    claim("get and set markers are mutually exclusive on a method",
        &mut tree, "//method[get and set]", 0);
}

// ---- arrow_function -------------------------------------------------------

/// Principle #5 — `arrow_function` renames to <arrow> (JS-native
/// vocabulary; distinct from <function> declarations).
#[test]
fn typescript_arrow() {
    let mut tree = parse_src("typescript", r#"
        const f = (x: number) => x + 1;
    "#);

    claim("arrow_function renames to <arrow>",
        &mut tree, "//arrow", 1);

    claim("no raw `arrow_function` kind leak",
        &mut tree, "//arrow_function", 0);
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

    claim("async function fetchOne",
        &mut tree, "//function[async and not(generator)][name='fetchOne']", 1);

    claim("generator function counter",
        &mut tree, "//function[generator and not(async)][name='counter']", 1);

    claim("async generator function stream",
        &mut tree, "//function[async and generator][name='stream']", 1);

    claim("async method load",
        &mut tree, "//method[async and not(generator)][name='load']", 1);

    claim("generator method keys",
        &mut tree, "//method[generator and not(async)][name='keys']", 1);
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

    claim("two constructors render as <constructor>",
        &mut tree, "//constructor", 2);

    claim("no abbreviated `ctor` element leaks",
        &mut tree, "//ctor", 0);

    claim("constructor name matches class name",
        &mut tree, "//constructor[name='Point']", 2);

    claim("zero-arg constructor's `this(...)` body is a <call>",
        &mut tree, "//constructor[not(parameter)]/body//call[this]", 1);
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

    claim("4 unified <call> nodes (1 path-call + 3 method-calls)",
        &mut tree, "//call", 4);

    claim("path-call `Vec::new()` has a <path> child",
        &mut tree, "//call[path[name='Vec' and name='new']]", 1);

    claim("3 method calls expose a <field> child for receiver.method",
        &mut tree, "//call/field", 3);

    claim("method `len` on receiver `v`",
        &mut tree, "//call/field[value/name='v' and name='len']", 1);

    claim("method `to_string` on a string-literal receiver",
        &mut tree, "//call/field[value/string and name='to_string']", 1);

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

    claim("`this(…)` renders as <call> with <this/> marker",
        &mut tree, "//call[this]", 1);

    claim("`super(…)` renders as <call> with <super/> marker",
        &mut tree, "//call[super]", 1);

    claim("no raw `explicit_constructor_invocation` kind leak",
        &mut tree, "//explicit_constructor_invocation", 0);
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

    claim("every parameter is either required or optional",
        &mut tree, "//parameter[not(required) and not(optional)]", 0);

    claim("required and optional are mutually exclusive",
        &mut tree, "//parameter[required and optional]", 0);

    claim("required: 1 (required) + defaulted + rest + 2 untyped = 5",
        &mut tree, "//parameter[required]", 5);

    claim("optional `?` is the only <parameter[optional]>",
        &mut tree, "//parameter[optional]", 1);

    claim("rest parameter exposes a <rest> child",
        &mut tree, "//parameter[rest]", 1);

    claim("defaulted parameter has a <value> child",
        &mut tree, "//parameter[name='defaulted'][value]", 1);
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

    claim("`*xs` carries spread[list]",
        &mut tree, "//spread[list]", 1);

    claim("`**kw` carries spread[dict]",
        &mut tree, "//spread[dict]", 1);

    claim("`key:` keyword parameter carries <keyword/>",
        &mut tree, "//parameter[keyword]", 1);
}
