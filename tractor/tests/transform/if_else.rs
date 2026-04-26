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

    claim("C# methods show flattened if-chain and wrapped ternary shapes",
        &mut tree,
        &multi_xpath(r#"
            //class[name='Conditionals']/body
                [method[name='Classify']//if
                    [count(else_if)=2]
                    [count(else)=1]
                ]
                [method[name='Label']//ternary
                    [then]
                    [else]
                ]
        "#),
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

    claim("Go function shape has flattened if-chain and no ternary node",
        &mut tree,
        &multi_xpath(r#"
            //function[name='Classify']
                [.//if
                    [count(else_if)=2]
                    [count(else)=1]
                ]
                [not(.//ternary)]
        "#),
        1);

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

    claim("Java methods show flattened if-chain and wrapped ternary shapes",
        &mut tree,
        &multi_xpath(r#"
            //class[name='Conditionals']/body
                [method[name='classify']//if
                    [count(else_if)=2]
                    [count(else)=1]
                ]
                [method[name='label']//ternary
                    [then]
                    [else]
                ]
        "#),
        1);
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

    claim("Python functions show flattened elif-chain and flat ternary shape",
        &mut tree,
        &multi_xpath(r#"
            //module
                [function[name='classify']//if
                    [count(else_if)=2]
                    [count(else)=1]
                ]
                [function[name='label']//ternary
                    [not(then)]
                    [not(else)]
                ]
        "#),
        1);

    claim("no `elif` raw element leaks",
        &mut tree, "//elif", 0);
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

    claim("Ruby methods show flattened elsif-chain and conditional ternary shape",
        &mut tree,
        &multi_xpath(r#"
            //program
                [method[name='classify']//if
                    [count(else_if)=2]
                    [count(else)=1]
                ]
                [method[name='label']//conditional]
        "#),
        1);
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

    claim("Rust functions show flattened if-chain and expression-if then/else shape",
        &mut tree,
        &multi_xpath(r#"
            //file
                [function[name='classify']//if
                    [count(else_if)=2]
                    [count(else)=1]
                ]
                [function[name='label']//if
                    [then]
                    [else]
                ]
        "#),
        1);
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

    claim("TypeScript function and arrow shapes show flattened if-chain and wrapped ternary",
        &mut tree,
        &multi_xpath(r#"
            //program
                [function[name='classify']//if
                    [count(else_if)=2]
                    [count(else)=1]
                ]
                [variable[name='label']//ternary
                    [then]
                    [else]
                ]
        "#),
        1);
}
