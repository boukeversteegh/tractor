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

    claim("Rust file shape has exhaustive visibility on functions, structs, and consts",
        &mut tree,
        &multi_xpath(r#"
            //file
                [function[name='private_fn']
                    [private]]
                [function[name='public_fn']
                    [pub]]
                [function[name='crate_fn']
                    [pub[crate]]]
                [function[name='super_fn']
                    [pub[super]]]
                [struct[name='PrivateStruct']
                    [private]]
                [struct[name='PublicStruct']
                    [pub]]
                [const[name='PRIV']
                    [private]]
                [const[name='PUB']
                    [pub]]
        "#),
        1);
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

    claim("TypeScript class member shapes carry explicit or default visibility",
        &mut tree,
        &multi_xpath(r#"
            //class[name='X']
                [body
                    [method[name='foo']
                        [public]]
                    [method[name='bar']
                        [private]]
                    [method[name='baz']
                        [protected]]
                    [method[name='qux']
                        [public]]
                    [field[name='x']
                        [public]]
                    [field[name='y']
                        [private]]
                ]
        "#),
        1);
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

    claim("PHP class member shapes carry explicit or default visibility",
        &mut tree,
        &multi_xpath(r#"
            //class[name='X']
                [body
                    [method[name='foo']
                        [public]]
                    [method[name='bar']
                        [private]]
                    [method[name='baz']
                        [protected]]
                    [method[name='qux']
                        [public]]
                    [field[name='$a']
                        [public]]
                    [field[name='$b']
                        [public]]
                    [field[name='$c']
                        [private]]
                ]
        "#),
        1);
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

    claim("Python class member visibility follows naming convention",
        &mut class_tree,
        &multi_xpath(r#"
            //class[name='X']
                [body
                    [function[name='foo']
                        [public]]
                    [function[name='_bar']
                        [protected]]
                    [function[name='__baz']
                        [private]]
                    [function[name='__init__']
                        [public]]
                ]
        "#),
        1);

    let mut module_tree = parse_src("python", r#"
def foo(): pass
def _bar(): pass
"#);

    claim("module-level Python functions skip visibility injection",
        &mut module_tree, "//function[public or protected or private]", 0);
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

    claim("Go file shape applies exported/unexported markers to functions, types, and fields",
        &mut tree,
        &multi_xpath(r#"
            //file
                [function[name='Public']
                    [exported]]
                [function[name='private']
                    [unexported]]
                [type[name='Exported']
                    [exported]]
                [type[name='unexported']
                    [unexported]]
                [struct[name='T']
                    [field[name='Public']
                        [exported]]
                    [field[name='private']
                        [unexported]]
                ]
        "#),
        1);

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

    claim("C# interface body defaults methods and property to public",
        &mut tree,
        &multi_xpath(r#"
            //interface[name='IShape']/body
                [method[name='Area']
                    [public]]
                [method[name='Perimeter']
                    [public]]
                [property[name='Name']
                    [public]]
                [method[name='Stroke']
                    [public]]
        "#),
        1);

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

    claim("Java interface body marks abstract and explicit-public methods as public",
        &mut tree,
        &multi_xpath(r#"
            //interface[name='Shape']/body
                [method[name='area']
                    [public]
                    [not(body)]]
                [method[name='perimeter']
                    [public]
                    [not(body)]]
                [method[name='stroke']
                    [public]]
                [method[name='name']
                    [package]]
        "#),
        1);

    claim("`default` method is not classified as <public/> (current behaviour)",
        &mut tree,
        &multi_xpath(r#"
            //interface/body/method[name='name']
                [public]
        "#),
        0);
}
