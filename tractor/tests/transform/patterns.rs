//! Cross-language: pattern shape markers for match arms,
//! destructuring, and is-tests.

use crate::support::semantic::*;

/// Python `match` patterns carry shape markers: `*rest` (splat /
/// list-tail destructure) and `'a' | 'b'` (union / alternation).
#[test]
fn python() {
    let mut tree = parse_src("python", r#"
match seq:
    case [1, *rest]: pass
    case 'yes' | 'y': pass
"#);

    claim("Python match arms show list-splat and union pattern shapes",
        &mut tree,
        &multi_xpath(r#"
            //match[name='seq']/body
                [arm/pattern//pattern[splat][name='rest']]
                [arm/pattern//pattern[union]
                    [string="'yes'"]
                    [string="'y'"]
                ]
                [count(arm)=2]
        "#),
        1);
}

/// C# pattern flavors all collapse to <pattern> but carry a
/// shape marker (declaration / recursive / constant / tuple).
#[test]
fn csharp() {
    let mut tree = parse_src("csharp", r#"
        class X {
            void F(object o) {
                if (o is Point p) {}
                if (o is null) {}
            }
        }
    "#);

    claim("C# is-pattern conditions show declaration and constant pattern shapes",
        &mut tree,
        &multi_xpath(r#"
            //method[name='F']/body/block
                [if[condition//pattern[declaration]]]
                [if[condition//pattern[constant]]]
        "#),
        1);
}

/// TypeScript destructuring patterns collapse to <pattern> but
/// carry an <array/> / <object/> marker that distinguishes the
/// shape without requiring string matching on `[` vs `{`.
#[test]
fn typescript() {
    let mut tree = parse_src("typescript", r#"
        const [a, b] = xs;
        const { x, y } = pt;
    "#);

    claim("TypeScript destructuring variables carry array and object pattern shapes",
        &mut tree,
        &multi_xpath(r#"
            //program
                [variable
                    [const]
                    [pattern
                        [array]
                        [name='a']
                        [name='b']]
                    [value/name='xs']
                ]
                [variable
                    [const]
                    [pattern
                        [object]
                        [name='x']
                        [name='y']]
                    [value/name='pt']
                ]
        "#),
        1);
}

/// Rust match arm patterns collapse to <pattern> but carry
/// <or/>, <struct/>, or <field/> markers so queries can pick out
/// the specific shape.
#[test]
fn rust() {
    let mut tree = parse_src("rust", r#"
        fn f(x: Shape) {
            match x {
                Shape::Square(_) | Shape::Circle(_) => {},
                Shape::Rect { w, h } => {},
                _ => {},
            }
        }
    "#);

    claim("Rust match arms show or-pattern, struct pattern, and field pattern shapes",
        &mut tree,
        &multi_xpath(r#"
            //match/body
                [arm/pattern//pattern[or]]
                [arm/pattern//pattern[struct]
                    [pattern[field][name='w']]
                    [pattern[field][name='h']]
                ]
                [count(arm)=3]
        "#),
        1);
}
