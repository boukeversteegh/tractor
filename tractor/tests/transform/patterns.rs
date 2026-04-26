//! Cross-language: pattern shape markers for match arms,
//! destructuring, and is-tests.

use crate::support::semantic::*;

/// Python `match` patterns carry shape markers: `*rest` (splat /
/// list-tail destructure) and `'a' | 'b'` (union / alternation).
#[test]
fn python() {
    claim("Python list pattern exposes splat child",
        &mut parse_src("python", r#"
match seq:
    case [1, *rest]: pass
"#), "//pattern[splat][name='rest']", 1);

    claim("Python union pattern exposes both alternatives",
        &mut parse_src("python", r#"
match answer:
    case 'yes' | 'y': pass
"#),
        &multi_xpath(r#"
            //pattern[union]
                [string="'yes'"]
                [string="'y'"]
        "#),
        1);
}

/// C# pattern flavors all collapse to <pattern> but carry a
/// shape marker (declaration / recursive / constant / tuple).
#[test]
fn csharp() {
    claim("C# declaration pattern carries declaration marker",
        &mut parse_src("csharp", r#"
        class X {
            void F(object o) {
                if (o is Point p) {}
            }
        }
    "#), "//pattern[declaration]", 1);

    claim("C# constant pattern carries constant marker",
        &mut parse_src("csharp", r#"
        class X {
            void F(object o) {
                if (o is null) {}
            }
        }
    "#), "//pattern[constant]", 1);
}

/// TypeScript destructuring patterns collapse to <pattern> but
/// carry an <array/> / <object/> marker that distinguishes the
/// shape without requiring string matching on `[` vs `{`.
#[test]
fn typescript() {
    claim("TypeScript array destructuring pattern carries array marker and names",
        &mut parse_src("typescript", r#"
        const [a, b] = xs;
    "#),
        &multi_xpath(r#"
            //pattern
                [array]
                [name='a']
                [name='b']
        "#),
        1);

    claim("TypeScript object destructuring pattern carries object marker and names",
        &mut parse_src("typescript", r#"
        const { x, y } = pt;
    "#),
        &multi_xpath(r#"
            //pattern
                [object]
                [name='x']
                [name='y']
        "#),
        1);
}

/// Rust match arm patterns collapse to <pattern> but carry
/// <or/>, <struct/>, or <field/> markers so queries can pick out
/// the specific shape.
#[test]
fn rust() {
    claim("Rust or-pattern carries or marker",
        &mut parse_src("rust", r#"
        fn f(x: Shape) {
            match x {
                Shape::Square(_) | Shape::Circle(_) => {},
            }
        }
    "#), "//pattern[or]", 1);

    claim("Rust struct pattern exposes field-pattern children",
        &mut parse_src("rust", r#"
        fn f(x: Shape) {
            match x {
                Shape::Rect { w, h } => {},
            }
        }
    "#),
        &multi_xpath(r#"
            //pattern[struct]
                [pattern[field][name='w']]
                [pattern[field][name='h']]
        "#),
        1);
}
