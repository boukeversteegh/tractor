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

/// Multi-alternative patterns produce same-element-name siblings
/// (multiple `<int>`, `<string>`, or `<name>` under
/// `<pattern[union]>` / `<pattern[alternative]>`). Per Principle
/// #19 they're role-uniform — each is one alternative option.
/// Tag with `list="ints"` / `list="strings"` / `list="names"` so
/// JSON renders as a homogeneous array. Single-alternative
/// patterns and role-mixed patterns stay singleton (cardinality
/// discriminator).
#[test]
fn python_union_pattern_lists_alternative_ints() {
    claim("Python case-union of three ints tags each with list='ints'",
        &mut parse_src("python", "match x:\n    case 1 OR 2 OR 3:\n        pass\n".replace("OR", "|").as_str()),
        "//pattern/int[@list='ints']",
        3);
}

#[test]
fn python_union_pattern_lists_alternative_strings() {
    claim("Python case-union of two strings tags each with list='strings'",
        &mut parse_src("python", "match x:\n    case \"a\" OR \"b\":\n        pass\n".replace("OR", "|").as_str()),
        "//pattern/string[@list='strings']",
        2);
}

#[test]
fn ruby_alternative_pattern_lists_alternative_ints() {
    claim("Ruby in-alternative of three ints tags each with list='ints'",
        &mut parse_src("ruby", "case x\nin 1 OR 2 OR 3\n  :small\nend\n".replace("OR", "|").as_str()),
        "//pattern/int[@list='ints']",
        3);
}

#[test]
fn ruby_array_pattern_lists_names() {
    claim("Ruby array pattern with two name slots tags each with list='names'",
        &mut parse_src("ruby", "case xs\nin [first, *, last]\n  first\nend\n"),
        "//pattern/name[@list='names']",
        2);
}

/// Java type patterns (`case Integer i ->`) and C# type patterns
/// (`case Integer i:`) used to emit a `[type]` marker alongside
/// the structural `<type>` child. The marker collided with the
/// wrapper on the JSON `type` key (boolean `type: true` vs
/// wrapper `type: {...}`), forcing the wrapper into `children`
/// overflow. Iter 275 dropped the redundant marker — the
/// structural `<type>` child already signals "this is a type
/// pattern", and `//pattern[type]` works via that child rather
/// than via the marker.
#[test]
fn java_type_pattern_no_marker_collision() {
    let mut tree = parse_src("java", r#"
        class T {
            String f(Object o) {
                return switch (o) {
                    case Integer i -> "int";
                    default -> "other";
                };
            }
        }
    "#);

    claim("Java type pattern has structural <type> child",
        &mut tree,
        "//pattern[type/name='Integer'][name='i']",
        1);

    claim("Java type pattern has NO empty <type/> marker",
        &mut tree,
        "//pattern/type[not(*) and not(text())]",
        0);
}

#[test]
fn csharp_type_pattern_no_marker_collision() {
    let mut tree = parse_src("csharp", r#"
        class T {
            string F(object o) {
                return o switch {
                    int i => "int",
                    _ => "other",
                };
            }
        }
    "#);

    claim("C# type pattern has structural <type> child",
        &mut tree,
        "//pattern[type/name='int'][name='i']",
        1);

    claim("C# type pattern has NO empty <type/> marker",
        &mut tree,
        "//pattern/type[not(*) and not(text())]",
        0);
}

/// C# `var (cnt, tg) = pair;` (tuple deconstruction) produces
/// `<pattern[tuple]>` with multiple `<name>` siblings (one per
/// binding). Per Principle #19 they're role-uniform; tag with
/// `list="names"`. Mirrors Ruby/Python iter 273.
#[test]
fn csharp_tuple_deconstruction_pattern_lists_names() {
    claim("C# tuple deconstruction tags each <name> with list='names'",
        &mut parse_src("csharp", r#"
        class T { void M() {
            var (cnt, tg) = pair;
        } }
    "#),
        "//pattern[tuple]/name[@list='names']",
        2);
}

/// C# multi-argument indexer `arr[1, 2, 3]` produces `<index>`
/// with multiple `<argument>` siblings. Per Principle #19 they're
/// role-uniform — tagged with `list="arguments"` so JSON renders
/// as a uniform array.
#[test]
fn csharp_multi_arg_indexer_lists_arguments() {
    claim("C# multi-arg indexer tags each <argument> with list='arguments'",
        &mut parse_src("csharp", "class T { void M() { var x = arr[1, 2, 3]; } }"),
        "//index/argument[@list='arguments']",
        3);
}

/// TypeScript object destructuring with multiple aliased entries
/// (`{ a: aa = 1, b: bb }`) produces `<pattern[object]>` with
/// multiple `<pair>` siblings. Per Principle #19 they're
/// role-uniform — each is one destructuring entry. Tag with
/// `list="pairs"` so JSON renders consistently.
#[test]
fn typescript_destructuring_pattern_lists_pairs() {
    claim("TS object-destructure with two aliased entries tags pairs",
        &mut parse_src("typescript", "function f({ a: aa, b: bb }: T) {}"),
        "//pattern[object]/pair[@list='pairs']",
        2);
}

/// Rust tuple-pattern bindings `(a, b, c)` produce
/// `<pattern[tuple]>` with multiple `<name>` siblings. Per
/// Principle #19 each name is a positional binding.
#[test]
fn rust_tuple_pattern_lists_names() {
    claim("Rust tuple-pattern with three bindings tags each <name>",
        &mut parse_src("rust", "fn f() { match t { (a, b, c) => () } }"),
        "//pattern[tuple]/name[@list='names']",
        3);
}
