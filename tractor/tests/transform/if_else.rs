//! Cross-language: conditional shape (if / else if / else
//! chains) and ternary expressions.
//!
//! Conditional shape: `else if` chains collapse to flat
//! <else_if> siblings of <if>; ternary keeps <then>/<else>
//! wrappers via surgical field-wrap in languages that have a
//! dedicated <ternary> node. Python ternary is FLAT (no
//! then/else wrappers); Ruby uses <conditional> rather than
//! <ternary>.

use crate::support::semantic::*;

#[test]
fn csharp() {
    claim("C# else-if chain collapses to flat else_if siblings",
        &mut parse_src("csharp", r#"
        class X {
            string Classify(int n) {
                if (n < 0) return "neg";
                else if (n == 0) return "zero";
                else if (n < 10) return "small";
                else return "big";
            }
        }
    "#),
        &multi_xpath(r#"
            //if
                [count(else_if)=2]
                [count(else)=1]
        "#),
        1);

    claim("C# ternary keeps then and else wrappers",
        &mut parse_src(
            "csharp",
            "class X { string Label(int n) => n > 0 ? \"positive\" : \"non-positive\"; }",
        ),
        "//ternary[then][else]",
        1);
}

#[test]
fn go() {
    let mut tree = parse_src("go", r#"
        package main

        func Classify(n int) string {
            if n < 0 { return "neg" } else if n == 0 { return "zero" } else if n < 10 { return "small" } else { return "big" }
        }
    "#);

    claim("Go else-if chain collapses to flat else_if siblings",
        &mut tree,
        &multi_xpath(r#"
            //if
                [count(else_if)=2]
                [count(else)=1]
        "#),
        1);

    claim("Go has no ternary node",
        &mut tree, "//ternary", 0);

}

#[test]
fn java() {
    claim("Java else-if chain collapses to flat else_if siblings",
        &mut parse_src("java", r#"
        class X {
            String classify(int n) {
                if (n < 0) { return "neg"; }
                else if (n == 0) { return "zero"; }
                else if (n < 10) { return "small"; }
                else { return "big"; }
            }
        }
    "#),
        &multi_xpath(r#"
            //if
                [count(else_if)=2]
                [count(else)=1]
        "#),
        1);

    claim("Java ternary keeps then and else wrappers",
        &mut parse_src("java", r#"
        class X {
            String label(int n) {
                return n > 0 ? "positive" : "non-positive";
            }
        }
    "#),
        "//ternary[then][else]",
        1);
}

#[test]
fn python() {
    let mut if_chain = parse_src("python", r#"
def classify(n):
    if n < 0:
        return "neg"
    elif n == 0:
        return "zero"
    elif n < 10:
        return "small"
    else:
        return "big"
"#);

    claim("Python elif chain is already flat else_if siblings",
        &mut if_chain,
        &multi_xpath(r#"
            //if
                [count(else_if)=2]
                [count(else)=1]
        "#),
        1);

    claim("no `elif` raw element leaks",
        &mut if_chain, "//elif", 0);

    claim("Python ternary wraps then/condition/else slots (iter 219)",
        &mut parse_src("python", "\"positive\" if n > 0 else \"non-positive\"\n"),
        &multi_xpath(r#"
            //ternary
                [then/string]
                [condition/expression/compare]
                [else/string]
        "#),
        1);
}

#[test]
fn ruby() {
    claim("Ruby elsif chain collapses to flat else_if siblings",
        &mut parse_src("ruby", r#"
        def classify(n)
          if n < 0
            "neg"
          elsif n == 0
            "zero"
          elsif n < 10
            "small"
          else
            "big"
          end
        end
    "#),
        &multi_xpath(r#"
            //if
                [count(else_if)=2]
                [count(else)=1]
        "#),
        1);

    claim("Ruby ternary expression renames to <ternary> with then/else role wrappers (iter 179)",
        &mut parse_src("ruby", "n > 0 ? \"positive\" : \"non-positive\"\n"),
        &multi_xpath(r#"
            //ternary
                [condition/expression/binary]
                [then/expression/string]
                [else/expression/string]
        "#),
        1);
}

#[test]
fn rust() {
    claim("Rust else-if chain collapses to flat else_if siblings",
        &mut parse_src("rust", r#"
        fn classify(n: i32) -> &'static str {
            if n < 0 { "neg" }
            else if n == 0 { "zero" }
            else if n < 10 { "small" }
            else { "big" }
        }
    "#),
        &multi_xpath(r#"
            //if
                [count(else_if)=2]
                [count(else)=1]
        "#),
        1);

    claim("Rust expression-if keeps then and else wrappers",
        &mut parse_src("rust", r#"
        fn label(n: i32) -> &'static str {
            if n > 0 { "positive" } else { "non-positive" }
        }
    "#),
        "//if[then][else]",
        1);
}

#[test]
fn typescript() {
    claim("TypeScript else-if chain collapses to flat else_if siblings",
        &mut parse_src("typescript", r#"
        function classify(n: number): string {
            if (n < 0) { return "neg"; }
            else if (n === 0) { return "zero"; }
            else if (n < 10) { return "small"; }
            else { return "big"; }
        }
    "#),
        &multi_xpath(r#"
            //if
                [count(else_if)=2]
                [count(else)=1]
        "#),
        1);

    claim("TypeScript ternary keeps then and else wrappers",
        &mut parse_src("typescript", "n > 0 ? \"positive\" : \"non-positive\";\n"),
        "//ternary[then][else]",
        1);
}

/// Multi-elseif chains tag each `<else_if>` sibling with
/// `list="else_ifs"` so JSON renders them as `else_ifs: [...]`
/// array. Single-elseif keeps the singleton `else_if` JSON key.
/// The tagging happens in the shared `collapse_else_if_chain`
/// helper, so every language that uses it benefits uniformly.
#[test]
fn multi_elseif_chain_lists_else_ifs() {
    claim("TypeScript 2+ elseifs tag with list='else_ifs'",
        &mut parse_src("typescript", r#"
        if (a) x = 1;
        else if (b) x = 2;
        else if (c) x = 3;
        else x = 4;
    "#),
        "//if/else_if[@list='else_ifs']",
        2);

    claim("TypeScript single elseif stays singleton (no list= tagging)",
        &mut parse_src("typescript", r#"
        if (a) x = 1;
        else if (b) x = 2;
    "#),
        "//if/else_if[not(@list)]",
        1);

    claim("Java 2+ elseifs tag with list='else_ifs'",
        &mut parse_src("java", r#"
        class T { void f() {
            if (a) x = 1;
            else if (b) x = 2;
            else if (c) x = 3;
        } }
    "#),
        "//if/else_if[@list='else_ifs']",
        2);
}
