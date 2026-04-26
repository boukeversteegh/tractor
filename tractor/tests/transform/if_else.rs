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
    let mut tree = parse_src("csharp", r#"
        class Conditionals
        {
            public string Classify(int n)
            {
                if (n < 0) { return "neg"; }
                else if (n == 0) { return "zero"; }
                else if (n < 10) { return "small"; }
                else { return "big"; }
            }

            public string Label(int n) => n > 0 ? "positive" : "non-positive";
        }
    "#);

    claim("one <if> at the chain root",
        &mut tree, "//if", 1);

    claim("two <else_if> siblings flattened under <if>",
        &mut tree, "//if/else_if", 2);

    claim("one trailing <else> sibling under <if>",
        &mut tree, "//if/else", 1);

    claim("ternary surgically wraps then/else",
        &mut tree, "//ternary[then and else]", 1);
}

#[test]
fn go() {
    let mut tree = parse_src("go", r#"
        package main

        func Classify(n int) string {
            if n < 0 { return "neg" } else if n == 0 { return "zero" } else if n < 10 { return "small" } else { return "big" }
        }
    "#);

    claim("one <if> at the chain root",
        &mut tree, "//if", 1);

    claim("two flat <else_if> siblings",
        &mut tree, "//if/else_if", 2);

    claim("one <else> sibling",
        &mut tree, "//if/else", 1);

    claim("Go has no <ternary> (no ternary in the language)",
        &mut tree, "//ternary", 0);
}

#[test]
fn java() {
    let mut tree = parse_src("java", r#"
        class Conditionals {
            String classify(int n) {
                if (n < 0) { return "neg"; }
                else if (n == 0) { return "zero"; }
                else if (n < 10) { return "small"; }
                else { return "big"; }
            }

            String label(int n) {
                return n > 0 ? "positive" : "non-positive";
            }
        }
    "#);

    claim("one <if> + 2 <else_if> + 1 <else>",
        &mut tree, "//if[count(else_if)=2 and count(else)=1]", 1);

    claim("ternary has <then> and <else> via surgical wrap",
        &mut tree, "//ternary[then and else]", 1);
}

#[test]
fn python() {
    let mut tree = parse_src("python", r#"
def classify(n):
    if n < 0:
        return "neg"
    elif n == 0:
        return "zero"
    elif n < 10:
        return "small"
    else:
        return "big"


def label(n):
    return "positive" if n > 0 else "non-positive"
"#);

    claim("one <if> at the chain root",
        &mut tree, "//if", 1);

    claim("`elif` becomes <else_if> (underscore naming)",
        &mut tree, "//if/else_if", 2);

    claim("no `elif` raw element leaks",
        &mut tree, "//elif", 0);

    claim("Python ternary is FLAT (no then/else wrappers)",
        &mut tree, "//ternary[then or else]", 0);

    claim("Python ternary still produces a <ternary> node",
        &mut tree, "//ternary", 1);
}

#[test]
fn ruby() {
    let mut tree = parse_src("ruby", r#"
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

        def label(n)
          n > 0 ? "positive" : "non-positive"
        end
    "#);

    claim("one <if> with 2 flat <else_if> siblings",
        &mut tree, "//if[count(else_if)=2]", 1);

    claim("`elsif` renames to <else_if>",
        &mut tree, "//if/else_if", 2);

    claim("Ruby ternary uses <conditional> (not <ternary>)",
        &mut tree, "//conditional", 1);
}

#[test]
fn rust() {
    let mut tree = parse_src("rust", r#"
        fn classify(n: i32) -> &'static str {
            if n < 0 { "neg" }
            else if n == 0 { "zero" }
            else if n < 10 { "small" }
            else { "big" }
        }

        fn label(n: i32) -> &'static str {
            if n > 0 { "positive" } else { "non-positive" }
        }
    "#);

    claim("classify: one <if> with 2 <else_if> + 1 <else>",
        &mut tree, "//function[name='classify']/body/if[count(else_if)=2 and count(else)=1]", 1);

    claim("label: if-expression as ternary keeps <then>/<else>",
        &mut tree, "//function[name='label']//if[then and else]", 1);
}

#[test]
fn typescript() {
    let mut tree = parse_src("typescript", r#"
        function classify(n: number): string {
            if (n < 0) { return "neg"; }
            else if (n === 0) { return "zero"; }
            else if (n < 10) { return "small"; }
            else { return "big"; }
        }

        const label = (n: number) => n > 0 ? "positive" : "non-positive";
    "#);

    claim("one <if> + 2 <else_if> + 1 <else>",
        &mut tree, "//if[count(else_if)=2 and count(else)=1]", 1);

    claim("ternary surgically wraps then/else",
        &mut tree, "//ternary[then and else]", 1);
}
