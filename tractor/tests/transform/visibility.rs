//! Cross-language: visibility / access modifiers.
//!
//! Visibility is exhaustive: every declaration carries either a
//! language-appropriate access marker (<public/>, <private/>,
//! <protected/>, <pub/>, <package/>, <exported/>, <unexported/>),
//! including the implicit defaults.

use crate::support::semantic::*;

/// Visibility is exhaustive: every declaration carries either
/// <private/> (implicit default) or <pub/> with optional
/// restriction details.
#[test]
fn rust() {
    let mut tree = parse_src("rust", r#"
        fn private_fn() {}
        pub fn public_fn() {}
        pub(crate) fn crate_fn() {}
        pub(super) fn super_fn() {}

        struct PrivateStruct;
        pub struct PublicStruct;

        const PRIV: i32 = 1;
        pub const PUB: i32 = 2;
    "#);

    claim("4 functions total, every one has visibility info",
        &mut tree, "//function[private or pub]", 4);

    claim("plain `pub` produces a <pub/> marker (no restriction)",
        &mut tree, "//function[pub][name='public_fn']", 1);

    claim("`pub(crate)` exposes <pub><crate/></pub>",
        &mut tree, "//function/pub[crate]", 1);

    claim("`pub(super)` exposes <pub><super/></pub>",
        &mut tree, "//function/pub[super]", 1);

    claim("private struct carries <private/>",
        &mut tree, "//struct[private][name='PrivateStruct']", 1);

    claim("private const carries <private/>",
        &mut tree, "//const[private][name='PRIV']", 1);
}

/// TypeScript class members carry an exhaustive visibility marker:
/// explicit `public/private/protected` keywords lift to markers,
/// and members without a keyword get an implicit <public/> (TS
/// default). Fields and methods follow the same rules.
#[test]
fn typescript() {
    let mut tree = parse_src("typescript", r#"
        class X {
            foo() {}
            private bar() {}
            protected baz() {}
            public qux() {}

            x = 1;
            private y = 2;
        }
    "#);

    claim("implicit default and explicit public both carry <public/> on methods",
        &mut tree, "//method[public]", 2);

    claim("explicit private method carries <private/>",
        &mut tree, "//method[private]", 1);

    claim("explicit protected method carries <protected/>",
        &mut tree, "//method[protected]", 1);

    claim("unmarked field defaults to <public/>",
        &mut tree, "//field[public]", 1);

    claim("explicit private field carries <private/>",
        &mut tree, "//field[private]", 1);
}

/// PHP class members carry an exhaustive visibility marker:
/// explicit `public/private/protected` keywords lift to markers,
/// and members without a keyword get implicit <public/> (PHP
/// default). Methods and properties follow the same rules.
#[test]
fn php() {
    let mut tree = parse_src("php", r#"<?php
        class X {
            function foo() {}
            private function bar() {}
            protected function baz() {}
            public function qux() {}

            public $a;
            $b;
            private $c;
        }
    "#);

    claim("implicit default and explicit public both carry <public/> on methods",
        &mut tree, "//method[public]", 2);

    claim("explicit private method carries <private/>",
        &mut tree, "//method[private]", 1);

    claim("explicit protected method carries <protected/>",
        &mut tree, "//method[protected]", 1);

    claim("explicit and implicit public properties both carry <public/>",
        &mut tree, "//field[public]", 2);

    claim("explicit private property carries <private/>",
        &mut tree, "//field[private]", 1);
}

/// Python visibility uses naming convention: bare → public,
/// `_x` → protected, `__x` → private. Dunders (`__init__`) are
/// conventional protocol hooks and count as public. The
/// convention applies ONLY to class members; module-level
/// functions are not classified.
#[test]
fn python() {
    let mut class_tree = parse_src("python", r#"
class X:
    def foo(self): pass
    def _bar(self): pass
    def __baz(self): pass
    def __init__(self): pass
"#);

    claim("bare name and dunder both count as public",
        &mut class_tree, "//function[public]", 2);

    claim("single-underscore prefix means protected",
        &mut class_tree, "//function[protected]", 1);

    claim("double-underscore prefix means private",
        &mut class_tree, "//function[private]", 1);

    let mut module_tree = parse_src("python", r#"
def foo(): pass
def _bar(): pass
"#);

    claim("module-level functions skip the visibility injection (no public)",
        &mut module_tree, "//function[public]", 0);

    claim("module-level functions skip the visibility injection (no protected)",
        &mut module_tree, "//function[protected]", 0);
}

/// Go uses Go's name-capitalization export rule for visibility:
/// every declaration carries an exhaustive <exported/> or
/// <unexported/> marker. Applies to functions, types, and struct
/// fields uniformly.
#[test]
fn go() {
    let mut tree = parse_src("go", r#"
        package main

        func Public() {}
        func private() {}

        type Exported int
        type unexported int

        type T struct {
            Public string
            private string
        }
    "#);

    claim("exported function carries <exported/>",
        &mut tree, "//function[exported]", 1);

    claim("unexported function carries <unexported/>",
        &mut tree, "//function[unexported]", 1);

    claim("every function carries one of the two markers",
        &mut tree, "//function[not(exported) and not(unexported)]", 0);

    claim("exported type carries <exported/>",
        &mut tree, "//type[exported]", 1);

    claim("unexported type carries <unexported/>",
        &mut tree, "//type[unexported]", 1);

    claim("capitalised struct field carries <exported/>",
        &mut tree, "//field[exported]", 1);

    claim("lower-case struct field carries <unexported/>",
        &mut tree, "//field[unexported]", 1);
}

// ---- interface_public -----------------------------------------------------

/// Interface members without an explicit access modifier default
/// to <public/>. C# and Java both lift this to an exhaustive
/// marker so a single //method[public] hits every visible
/// interface method.
#[test]
fn csharp_interface() {
    let mut tree = parse_src("csharp", r#"
        interface IShape
        {
            double Area();
            double Perimeter();
            string Name => "shape";
            public void Stroke();
        }
    "#);

    claim("3 interface methods all carry <public/>",
        &mut tree, "//interface/body/method[public]", 3);

    claim("expression-bodied property carries <public/>",
        &mut tree, "//interface/body/property[public]", 1);

    claim("no interface member is missing visibility",
        &mut tree, "//interface/body/*[(self::method or self::property) and not(public)]", 0);
}

/// Java pins current behaviour: implicit-public abstract methods
/// surface as <public/>; the explicit `public void stroke()` also
/// gets <public/>. Default methods (`default String name() {...}`)
/// currently render with <package/> rather than <public/> — pin
/// that as the actual current shape rather than aspiration.
#[test]
fn java_interface() {
    let mut tree = parse_src("java", r#"
        interface Shape {
            double area();
            double perimeter();
            default String name() { return "shape"; }
            public void stroke();
        }
    "#);

    claim("implicit-public abstract methods carry <public/>",
        &mut tree, "//interface/body/method[public][not(body)]", 3);

    claim("explicit `public` method `stroke` also carries <public/>",
        &mut tree, "//interface/body/method[public][name='stroke']", 1);

    claim("`default` method is not classified as <public/> (current behaviour)",
        &mut tree, "//interface/body/method[name='name'][public]", 0);
}
