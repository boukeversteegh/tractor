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

    claim("`*rest` destructure pattern carries <splat/>",
        &mut tree, "//pattern[splat]", 1);

    claim("`'yes' | 'y'` union pattern carries <union/>",
        &mut tree, "//pattern[union]", 1);
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

    claim("`o is T name` — declaration pattern carries <declaration/>",
        &mut tree, "//pattern[declaration]", 1);

    claim("`o is null` — constant pattern carries <constant/>",
        &mut tree, "//pattern[constant]", 1);
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

    claim("array destructuring pattern carries <array/>",
        &mut tree, "//pattern[array]", 1);

    claim("object destructuring pattern carries <object/>",
        &mut tree, "//pattern[object]", 1);
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

    claim("alternative pattern (`A | B`) carries <or/>",
        &mut tree, "//pattern[or]", 1);

    claim("struct destructure pattern carries <struct/>",
        &mut tree, "//pattern[struct]", 1);

    claim("each struct field in pattern carries <field/>",
        &mut tree, "//pattern[field]", 2);
}
