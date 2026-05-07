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
        "//pattern/int",
        3);
}

#[test]
fn python_union_pattern_lists_alternative_strings() {
    claim("Python case-union of two strings tags each with list='strings'",
        &mut parse_src("python", "match x:\n    case \"a\" OR \"b\":\n        pass\n".replace("OR", "|").as_str()),
        "//pattern/string",
        2);
}

/// Dict patterns `case {"k": v, ...}:` produce `<pattern[dict]>`
/// with multiple `<value>` siblings. Already-listed `<string>`
/// keys mirror via the global same-name whitelist; the `<value>`
/// children become role-uniform `list='values'` so JSON renders
/// as a homogeneous array instead of one value taking the
/// singleton key and the rest overflowing into `children`.
#[test]
fn python_dict_pattern_lists_values() {
    let mut tree = parse_src(
        "python",
        "match m:\n    case {\"a\": 1, \"b\": 2}:\n        pass\n",
    );

    claim("Python dict pattern keys tag as list='strings'",
        &mut tree,
        "//pattern[dict]/string",
        2);

    claim("Python dict pattern values tag as list='values'",
        &mut tree,
        "//pattern[dict]/value",
        2);
}

/// Python match-arm guard `case <pat> if <expr>:` — the guard wraps in
/// `<guard>` slot. Iter 300 cold-read flagged the previous shape (bare
/// `<compare>` floating as a sibling of `<pattern>`) for ambiguous
/// cross-language alignment. Comprehension if-clauses are NOT
/// affected; they keep the existing flatten-into-`<compare>` shape.
#[test]
fn python_match_arm_guard_wraps_in_guard_slot() {
    claim("Python match-arm guard wraps the comparison in <guard>",
        &mut parse_src("python", "match x:\n    case [a, b] if a > 0:\n        pass\n"),
        "//arm[guard/compare/name='a']",
        1);

    claim("Python match-arm guard does NOT leave a bare <compare> sibling",
        &mut parse_src("python", "match x:\n    case [a, b] if a > 0:\n        pass\n"),
        "//arm/compare",
        0);

    claim("Python comprehension if-clause flattens into <compare> (unchanged)",
        &mut parse_src("python", "xs = [a for a in data if a > 0]"),
        "//list[comprehension]/compare",
        1);
}

/// C# switch — both `switch_expression_arm` (`pat => expr,`) and
/// `switch_section` (`case pat: ...; break;`) render as `<arm>`. Same
/// element name for the same user-mental-model "switch case." Mirrors
/// Java which uses `<arm>` for both forms. Closes iter 300 cold-read
/// finding (#9 — C# arm/section duality).
#[test]
fn csharp_switch_statement_section_renders_as_arm() {
    let mut tree = parse_src("csharp", r#"
        class T {
            int F(object o) {
                switch (o) {
                    case int i: return i;
                    default: return 0;
                }
            }
        }
    "#);

    claim("C# switch-statement case renders as <arm>",
        &mut tree,
        "//switch/body/arm",
        2);

    claim("no <section> element survives the C# transform",
        &mut tree,
        "//section",
        0);
}

#[test]
fn csharp_switch_expression_arm_renders_as_arm() {
    claim("C# switch-expression arm renders as <arm>",
        &mut parse_src("csharp", r#"
        class T {
            string F(int n) => n switch {
                0 => "zero",
                _ => "other",
            };
        }
    "#),
        "//switch/arm",
        2);
}

#[test]
fn ruby_alternative_pattern_lists_alternative_ints() {
    claim("Ruby in-alternative of three ints tags each with list='ints'",
        &mut parse_src("ruby", "case x\nin 1 OR 2 OR 3\n  :small\nend\n".replace("OR", "|").as_str()),
        "//pattern/int",
        3);
}

#[test]
fn ruby_array_pattern_lists_names() {
    claim("Ruby array pattern with two name slots tags each with list='names'",
        &mut parse_src("ruby", "case xs\nin [first, *, last]\n  first\nend\n"),
        "//pattern/name",
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
        "//pattern[tuple]/name",
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
        "//index/argument",
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
        "//pattern[object]/pair",
        2);
}

/// Rust tuple-pattern bindings `(a, b, c)` produce
/// `<pattern[tuple]>` with multiple `<name>` siblings. Per
/// Principle #19 each name is a positional binding.
#[test]
fn rust_tuple_pattern_lists_names() {
    claim("Rust tuple-pattern with three bindings tags each <name>",
        &mut parse_src("rust", "fn f() { match t { (a, b, c) => () } }"),
        "//pattern[tuple]/name",
        3);
}
