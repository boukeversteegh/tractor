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

    claim("Rust default function visibility is private",
        &mut tree, "//function[name='private_fn'][private]", 1);

    claim("Rust public function visibility is pub",
        &mut tree, "//function[name='public_fn'][pub]", 1);

    claim("Rust restricted pub(crate) keeps crate detail",
        &mut tree, "//function[name='crate_fn']/pub[crate]", 1);

    claim("Rust restricted pub(super) keeps super detail",
        &mut tree, "//function[name='super_fn']/pub[super]", 1);

    claim("Rust default struct visibility is private",
        &mut tree, "//struct[name='PrivateStruct'][private]", 1);

    claim("Rust public struct visibility is pub",
        &mut tree, "//struct[name='PublicStruct'][pub]", 1);

    claim("Rust default const visibility is private",
        &mut tree, "//const[name='PRIV'][private]", 1);

    claim("Rust public const visibility is pub",
        &mut tree, "//const[name='PUB'][pub]", 1);
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

    claim("TypeScript default method visibility is public",
        &mut tree, "//method[name='foo'][public]", 1);

    claim("TypeScript private method visibility is explicit",
        &mut tree, "//method[name='bar'][private]", 1);

    claim("TypeScript protected method visibility is explicit",
        &mut tree, "//method[name='baz'][protected]", 1);

    claim("TypeScript public method visibility is explicit",
        &mut tree, "//method[name='qux'][public]", 1);

    claim("TypeScript default field visibility is public",
        &mut tree, "//field[name='x'][public]", 1);

    claim("TypeScript private field visibility is explicit",
        &mut tree, "//field[name='y'][private]", 1);
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

    claim("PHP default method visibility is public",
        &mut tree, "//method[name='foo'][public]", 1);

    claim("PHP private method visibility is explicit",
        &mut tree, "//method[name='bar'][private]", 1);

    claim("PHP protected method visibility is explicit",
        &mut tree, "//method[name='baz'][protected]", 1);

    claim("PHP public method visibility is explicit",
        &mut tree, "//method[name='qux'][public]", 1);

    claim("PHP public field visibility is explicit",
        &mut tree, "//field[name='$a'][public]", 1);

    claim("PHP default field visibility is public",
        &mut tree, "//field[name='$b'][public]", 1);

    claim("PHP private field visibility is explicit",
        &mut tree, "//field[name='$c'][private]", 1);
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

    claim("Python bare class member is public",
        &mut class_tree, "//function[name='foo'][public]", 1);

    claim("Python single-underscore class member is protected",
        &mut class_tree, "//function[name='_bar'][protected]", 1);

    claim("Python double-underscore class member is private",
        &mut class_tree, "//function[name='__baz'][private]", 1);

    claim("Python dunder class member is public",
        &mut class_tree, "//function[name='__init__'][public]", 1);

    claim("module-level Python functions skip visibility injection",
        &mut parse_src("python", r#"
def foo(): pass
def _bar(): pass
"#),
        "//function[public or protected or private]",
        0);
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

    claim("Go exported function visibility follows capitalization",
        &mut tree, "//function[name='Public'][exported]", 1);

    claim("Go unexported function visibility follows capitalization",
        &mut tree, "//function[name='private'][unexported]", 1);

    claim("Go exported type visibility follows capitalization",
        &mut tree, "//type[name='Exported'][exported]", 1);

    claim("Go unexported type visibility follows capitalization",
        &mut tree, "//type[name='unexported'][unexported]", 1);

    claim("Go exported struct field visibility follows capitalization",
        &mut tree, "//field[name='Public'][exported]", 1);

    claim("Go unexported struct field visibility follows capitalization",
        &mut tree, "//field[name='private'][unexported]", 1);

    claim("every Go function carries one of the two visibility markers",
        &mut tree, "//function[not(exported) and not(unexported)]", 0);
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

    claim("C# interface Area method defaults to public",
        &mut tree, "//method[name='Area'][public]", 1);

    claim("C# interface Perimeter method defaults to public",
        &mut tree, "//method[name='Perimeter'][public]", 1);

    claim("C# explicit public interface method stays public",
        &mut tree, "//method[name='Stroke'][public]", 1);

    claim("C# interface property defaults to public",
        &mut tree, "//property[name='Name'][public]", 1);

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

    claim("Java abstract interface area method defaults to public",
        &mut tree, "//method[name='area'][public][not(body)]", 1);

    claim("Java abstract interface perimeter method defaults to public",
        &mut tree, "//method[name='perimeter'][public][not(body)]", 1);

    claim("Java explicit public interface method stays public",
        &mut tree, "//method[name='stroke'][public]", 1);

    claim("Java default interface method currently stays package",
        &mut tree, "//method[name='name'][package]", 1);

    claim("`default` method is not classified as <public/> (current behaviour)",
        &mut tree,
        &multi_xpath(r#"
            //interface/body/method[name='name']
                [public]
        "#),
        0);
}
