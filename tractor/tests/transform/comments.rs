//! Cross-language: comment classification (trailing / leading /
//! floating) and grouping for adjacent line comments.

use crate::support::semantic::*;

/// One test per language. The source snippet is a deliberate
/// kitchen-sink for THIS feature — every comment variant the
/// language has appears once, and the assertions probe the
/// resulting shape. One parse, many claims.
#[test]
fn csharp() {
    let mut tree = parse_src(
        "csharp",
        r#"
            class Demo {
                private int _count; // trailing single

                // leading first
                // leading second
                public string Config { get; set; }

                /* leading block */
                public void Run() {}

                // floating

                public int Solo() => 0;
            }
        "#,
    );

    claim("Demo body has trailing, merged leading, block-leading, and floating comments",
        &mut tree,
        &multi_xpath(r#"
            //class[name='Demo']/body
                [field[.//name='_count']]
                [comment[.='// trailing single']
                    [trailing]
                ]
                [comment[contains(., 'leading first')]
                    [contains(., 'leading second')]
                    [leading]
                ]
                [property[name='Config']]
                [comment[.='/* leading block */']
                    [leading]
                ]
                [method[name='Run']]
                [comment[.='// floating']
                    [not(leading)]
                    [not(trailing)]
                ]
                [method[name='Solo']]
        "#),
        1);

    claim("trailing and leading are mutually exclusive",
        &mut tree, "//comment[trailing and leading]", 0);
}

#[test]
fn csharp_comment_edges() {
    let mut adjacent = parse_src("csharp", r#"
        class Foo {
            int x; // trailing
            // block 1
            // block 2
            int y;
        }
    "#);

    claim("C# trailing comment does not absorb the following leading line-comment group",
        &mut adjacent,
        &multi_xpath(r#"
            //class[name='Foo']/body
                [count(comment)=2]
                [field[.//name='x']]
                [comment[.='// trailing']
                    [trailing]]
                [comment[contains(., 'block 1')]
                    [contains(., 'block 2')]
                    [leading]]
                [field[.//name='y']]
        "#),
        1);

    claim("C# merged line-comment group preserves the full source span",
        &mut adjacent,
        "//comment[contains(., 'block 1') and contains(., 'block 2')]",
        1);
    assert_eq!(
        query(&mut adjacent, "//comment[contains(., 'block 1')]")[0].extract_source_snippet(),
        "// block 1\n            // block 2"
    );

    claim("C# block comments do not group with adjacent line comments",
        &mut parse_src("csharp", r#"
            class Foo {
                /* block */
                // line
                int y;
            }
        "#),
        &multi_xpath(r#"
            //class[name='Foo']/body
                [count(comment)=2]
                [comment[.='/* block */']
                    [not(leading)]
                    [not(trailing)]]
                [comment[.='// line']
                    [leading]]
                [field[.//name='y']]
        "#),
        1);

    claim("C# top-level comment immediately before a class is leading",
        &mut parse_src("csharp", "// describes Foo\npublic class Foo { }\n"),
        &multi_xpath(r#"
            //unit
                [comment[.='// describes Foo']
                    [leading]]
                [class[name='Foo']]
        "#),
        1);
}

/// TypeScript (and JS) share C#'s trailing / leading / floating
/// classifier — `//` and `/* */` are both tree-sitter `comment`
/// nodes; the shared classifier handles them uniformly.
#[test]
fn typescript() {
    let mut tree = parse_src(
        "typescript",
        r#"
            // floating one

            class X {
                x: number; // inline
                // before y
                // also before y
                y: string;
                /* block */
                z: boolean;
                /** JSDoc */
                method() {}
            }
        "#,
    );

    claim("X body has trailing, merged leading, block-leading, and JSDoc-leading comments",
        &mut tree,
        &multi_xpath(r#"
            //class[name='X']/body
                [field[name='x']]
                [comment[.='// inline']
                    [trailing]
                ]
                [comment[contains(., 'before y')]
                    [contains(., 'also before y')]
                    [leading]
                ]
                [field[name='y']]
                [comment[.='/* block */']
                    [leading]
                ]
                [field[name='z']]
                [comment[starts-with(., '/**')]
                    [contains(., 'JSDoc')]
                    [leading]
                ]
                [method[name='method']]
        "#),
        1);

    claim("blank-line break leaves the top-level TS comment floating",
        &mut tree,
        &multi_xpath(r#"
            //comment[.='// floating one']
                [not(leading)]
                [not(trailing)]
        "#),
        1);

    claim("trailing and leading are mutually exclusive",
        &mut tree, "//comment[trailing and leading]", 0);
}

/// Python `#` comments. Tree-sitter calls them `comment`; tractor
/// renames to `<comment>` uniformly and runs the shared trailing /
/// leading / floating classifier (with `#` as the line-comment
/// prefix).
#[test]
fn python() {
    let mut tree = parse_src(
        "python",
        r#"
# floating

class X:
    """docstring stays a string, not a comment"""
    x = 1  # inline
    # before y
    # also before y
    y = 2

    # leading on z
    z = 3
"#,
    );

    claim("class X keeps docstring as string and class-body comments classified",
        &mut tree,
        &multi_xpath(r#"
            //class[name='X']
                [.//string[contains(., 'docstring')]]
                [.//comment[.='# inline']
                    [trailing]
                ]
                [.//comment[contains(., 'before y')]
                    [contains(., 'also before y')]
                    [leading]
                ]
                [.//comment[.='# leading on z']
                    [leading]
                ]
        "#),
        1);

    claim("docstring is not classified as a comment",
        &mut tree, "//comment[contains(., 'docstring')]", 0);

    claim("blank-line break leaves the module-level Python comment floating",
        &mut tree,
        &multi_xpath(r#"
            //comment[.='# floating']
                [not(leading)]
                [not(trailing)]
        "#),
        1);

    claim("trailing and leading are mutually exclusive",
        &mut tree, "//comment[trailing and leading]", 0);
}

/// Go has both `//` and `/* */` (single tree-sitter `comment`
/// kind). Same trailing / leading / floating classification +
/// `//` line-comment grouping.
#[test]
fn go() {
    let mut tree = parse_src(
        "go",
        r#"
            package main

            // floating one

            // before func
            // also before func
            func A() int {
                return 1
            }

            /* block before B */
            func B() int {
                x := 1 // trailing single
                return x
            }
        "#,
    );

    claim("Go file shape has merged leading func comment, block-leading comment, and trailing local comment",
        &mut tree,
        &multi_xpath(r#"
            //file
                [comment[contains(., 'before func')]
                    [contains(., 'also before func')]
                    [leading]
                ]
                [function[name='A']]
                [comment[.='/* block before B */']
                    [leading]
                ]
                [function[name='B']
                    [.//comment[.='// trailing single']
                        [trailing]
                    ]
                ]
        "#),
        1);

    claim("blank-line break leaves the top-level Go comment floating",
        &mut tree,
        &multi_xpath(r#"
            //comment[.='// floating one']
                [not(leading)]
                [not(trailing)]
        "#),
        1);

    claim("trailing and leading are mutually exclusive",
        &mut tree, "//comment[trailing and leading]", 0);
}

/// PHP supports `//` and `#` line comments plus `/* */` blocks.
/// All collapse to <comment>; the shared classifier handles
/// trailing / leading / floating with both `//` and `#` as
/// line-comment prefixes for grouping.
#[test]
fn php() {
    let mut tree = parse_src(
        "php",
        r#"<?php
// floating one

// before A
// also before A
class A {
    public int $x = 1; // trailing single
}

# before B
# also before B
class B {}

/* leading block */
class C {}
"#,
    );

    claim("PHP file shape covers // grouping, # grouping, block leading, and trailing property comment",
        &mut tree,
        &multi_xpath(r#"
            //program
                [comment[contains(., 'before A')]
                    [contains(., 'also before A')]
                    [leading]
                ]
                [class[name='A']
                    [.//comment[.='// trailing single']
                        [trailing]
                    ]
                ]
                [comment[contains(., 'before B')]
                    [contains(., 'also before B')]
                    [leading]
                ]
                [class[name='B']]
                [comment[.='/* leading block */']
                    [leading]
                ]
                [class[name='C']]
        "#),
        1);

    claim("blank-line break leaves the top-level PHP comment floating",
        &mut tree,
        &multi_xpath(r#"
            //comment[.='// floating one']
                [not(leading)]
                [not(trailing)]
        "#),
        1);

    claim("trailing and leading are mutually exclusive",
        &mut tree, "//comment[trailing and leading]", 0);
}

/// Ruby uses `#` line comments. Same trailing / leading /
/// floating classification + adjacent-line grouping. (Block
/// `=begin`/`=end` comments are rare and out of scope.)
#[test]
fn ruby() {
    let mut tree = parse_src(
        "ruby",
        r#"
# floating one

# leading one
# leading two
class Demo
  attr_reader :name # trailing single
end

# leading on bare
x = 1
"#,
    );

    claim("Ruby file shape has class-leading group, trailing attr comment, and leading assignment comment",
        &mut tree,
        &multi_xpath(r#"
            //program
                [comment[contains(., 'leading one')]
                    [contains(., 'leading two')]
                    [leading]
                ]
                [class[name='Demo']
                    [.//comment[.='# trailing single']
                        [trailing]
                    ]
                ]
                [comment[.='# leading on bare']
                    [leading]
                ]
                [assign]
        "#),
        1);

    claim("blank-line break leaves the top-level Ruby comment floating",
        &mut tree,
        &multi_xpath(r#"
            //comment[.='# floating one']
                [not(leading)]
                [not(trailing)]
        "#),
        1);

    claim("trailing and leading are mutually exclusive",
        &mut tree, "//comment[trailing and leading]", 0);
}

/// Java mirrors C# (both `//` and `/* */`). Same trailing /
/// leading / floating classification + line-comment grouping.
#[test]
fn java() {
    let mut tree = parse_src(
        "java",
        r#"
            class Demo {
                private int count; // trailing single

                // leading first
                // leading second
                public String name;

                /* leading block */
                public void run() {}

                // floating

                public int solo() { return 0; }
            }
        "#,
    );

    claim("Demo body has trailing, merged leading, block-leading, and floating Java comments",
        &mut tree,
        &multi_xpath(r#"
            //class[name='Demo']/body
                [field[declarator/name='count']]
                [comment[.='// trailing single']
                    [trailing]
                ]
                [comment[contains(., 'leading first')]
                    [contains(., 'leading second')]
                    [leading]
                ]
                [field[declarator/name='name']]
                [comment[.='/* leading block */']
                    [leading]
                ]
                [method[name='run']]
                [comment[.='// floating']
                    [not(leading)]
                    [not(trailing)]
                ]
                [method[name='solo']]
        "#),
        1);

    claim("trailing and leading are mutually exclusive",
        &mut tree, "//comment[trailing and leading]", 0);
}

/// Rust has 4 comment kinds: `//`, `/* */`, doc `///`, inner doc
/// `//!`. All collapse to `<comment>` and go through the same
/// trailing / leading / floating classifier as C# / Java.
#[test]
fn rust() {
    let mut tree = parse_src(
        "rust",
        r#"
            //! crate-level inner doc

            /// outer doc line one
            /// outer doc line two
            fn x() {}

            // line
            /* block */
            fn y() {}

            struct S {
                a: i32, // trailing single
                b: i32,
            }

            // floating

            fn z() {}
        "#,
    );

    claim("Rust file shape covers inner doc, merged outer doc, block-leading, trailing, and floating comments",
        &mut tree,
        &multi_xpath(r#"
            //file
                [comment[starts-with(., '//!')]
                    [not(leading)]
                    [not(trailing)]
                ]
                [comment[contains(., 'outer doc line one')]
                    [contains(., 'outer doc line two')]
                    [leading]
                ]
                [function[name='x']]
                [comment[.='// line']
                    [not(leading)]
                    [not(trailing)]
                ]
                [comment[.='/* block */']
                    [leading]
                ]
                [function[name='y']]
                [struct[name='S']
                    [.//comment[.='// trailing single']
                        [trailing]
                    ]
                ]
                [comment[.='// floating']
                    [not(leading)]
                    [not(trailing)]
                ]
                [function[name='z']]
        "#),
        1);

    claim("trailing and leading are mutually exclusive",
        &mut tree, "//comment[trailing and leading]", 0);
}
