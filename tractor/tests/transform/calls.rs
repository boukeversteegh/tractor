//! Cross-language: function and method call shapes.
//!
//! Both function calls and method calls render as <call>. Method
//! calls are distinguished by a <field> or <member> child that names
//! the receiver and method. Java `this(…)` / `super(…)` constructor
//! invocations render as <call> with a <this/> or <super/> marker —
//! uniform with other call sites; no `explicit_constructor_invocation`
//! raw kind leaks.

use crate::support::semantic::*;

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

    claim("Rust method call inverts to <object[access]> chain shape (iter 248)",
        &mut parse_src("rust", r#"
        fn use_calls(v: Vec<i32>) {
            let n = v.len();
        }
    "#),
        &multi_xpath(r#"
            //object[access]
                [name='v']
                [call/name='len']
        "#),
        1);

    claim("Rust method call on literal receiver inverts to <object[access]>",
        &mut parse_src("rust", r#"
        fn use_calls() {
            let s = "hi".to_string();
        }
    "#),
        &multi_xpath(r#"
            //object[access]
                [string]
                [call/name='to_string']
        "#),
        1);

    claim("Rust expression statement method call inverts to <object[access]>",
        &mut parse_src("rust", r#"
        fn use_calls(s: String) {
            s.to_uppercase();
        }
    "#),
        &multi_xpath(r#"
            //object[access]
                [name='s']
                [call/name='to_uppercase']
        "#),
        1);
}

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

    // `super.method()` exercises the wrapper-form of <super>: the
    // member-access object is `<super>super</super>` (text leaf), NOT
    // an empty <super/> marker. Pinning this exercises layer 2's
    // shape-contract assertion (`marker-stays-empty` + Super declared
    // DualUse iter 296), which catches the same archetype as iter
    // 294 Rust Crate/Super and iter 295 C# Struct.
    claim("Java super.method() member access wraps <super> as text leaf",
        &mut parse_src("java", r#"
        class X { Object f() { return super.toString(); } }
    "#),
        "//super[. = 'super']",
        1);
}
