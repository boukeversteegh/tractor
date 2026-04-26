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

    claim("single-line `//` after `;` on same line is trailing",
        &mut tree, "//comment[trailing][.='// trailing single']", 1);

    claim("adjacent `//` comments merge into one <comment>",
        &mut tree, &multi_xpath("
            //comment[leading]
                [contains(., 'leading first')]
                [contains(., 'leading second')]
        "), 1);

    claim("block `/* */` immediately before a decl is leading",
        &mut tree, "//comment[leading][.='/* leading block */']", 1);

    claim("blank-line break: floating comment has no marker",
        &mut tree, "//comment[.='// floating'][not(leading) and not(trailing)]", 1);

    claim("trailing and leading are mutually exclusive",
        &mut tree, "//comment[trailing and leading]", 0);

    claim("no raw tree-sitter `line_comment` / `block_comment` leaks",
        &mut tree, "//line_comment | //block_comment", 0);
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

    claim("`//` line comment becomes <comment>",
        &mut tree, "//comment[.='// floating one']", 1);

    claim("`/* */` block becomes <comment>",
        &mut tree, "//comment[.='/* block */']", 1);

    claim("JSDoc `/** */` becomes <comment>",
        &mut tree, "//comment[starts-with(., '/**')][contains(., 'JSDoc')]", 1);

    claim("no raw tree-sitter `line_comment` / `block_comment` leaks",
        &mut tree, "//line_comment | //block_comment", 0);

    claim("inline `//` after `;` is trailing",
        &mut tree, "//comment[trailing][.='// inline']", 1);

    claim("two adjacent `//` comments before `y` merge into one <comment>",
        &mut tree, &multi_xpath("
            //comment[leading]
                [contains(., 'before y')]
                [contains(., 'also before y')]
        "), 1);

    claim("block comment `/* block */` is leading on `z`",
        &mut tree, "//comment[leading][.='/* block */']", 1);

    claim("JSDoc is leading on the method",
        &mut tree, "//comment[leading][starts-with(., '/**')]", 1);

    claim("blank-line break: `// floating one` carries no marker",
        &mut tree, "//comment[.='// floating one'][not(leading) and not(trailing)]", 1);

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

    claim("`#` line comment becomes <comment>",
        &mut tree, "//comment[.='# floating']", 1);

    claim("inline `#` after code is still <comment>",
        &mut tree, "//comment[.='# inline']", 1);

    claim("docstring is a <string>, NOT a <comment>",
        &mut tree, "//comment[contains(., 'docstring')]", 0);

    claim("docstring lives as a <string> child of <class>",
        &mut tree, "//class//string[contains(., 'docstring')]", 1);

    claim("inline `#` after `x = 1` is trailing",
        &mut tree, "//comment[trailing][.='# inline']", 1);

    claim("two adjacent `#` comments before y merge into one <comment>",
        &mut tree, &multi_xpath("
            //comment[leading]
                [contains(., 'before y')]
                [contains(., 'also before y')]
        "), 1);

    claim("`# leading on z` is leading on the assignment",
        &mut tree, "//comment[leading][.='# leading on z']", 1);

    claim("blank-line break: floating `# floating` has no marker",
        &mut tree, "//comment[.='# floating'][not(leading) and not(trailing)]", 1);

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

    claim("`//` line comment becomes <comment>",
        &mut tree, "//comment[.='// floating one']", 1);

    claim("`/* */` block becomes <comment>",
        &mut tree, "//comment[.='/* block before B */']", 1);

    claim("inline `//` after `:=` is trailing",
        &mut tree, "//comment[trailing][.='// trailing single']", 1);

    claim("two adjacent `//` comments before func A merge into one <comment>",
        &mut tree, &multi_xpath("
            //comment[leading]
                [contains(., 'before func')]
                [contains(., 'also before func')]
        "), 1);

    claim("block `/* */` immediately before a decl is leading",
        &mut tree, "//comment[leading][.='/* block before B */']", 1);

    claim("blank-line break: floating comment has no marker",
        &mut tree, "//comment[.='// floating one'][not(leading) and not(trailing)]", 1);

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

    claim("`//` line comment becomes <comment>",
        &mut tree, "//comment[.='// floating one']", 1);

    claim("`/* */` block becomes <comment>",
        &mut tree, "//comment[.='/* leading block */']", 1);

    claim("inline `//` after `;` is trailing",
        &mut tree, "//comment[trailing][.='// trailing single']", 1);

    claim("two adjacent `//` comments merge into one <comment>",
        &mut tree, &multi_xpath("
            //comment[leading]
                [contains(., 'before A')]
                [contains(., 'also before A')]
        "), 1);

    claim("two adjacent `#` comments merge into one <comment>",
        &mut tree, &multi_xpath("
            //comment[leading]
                [contains(., 'before B')]
                [contains(., 'also before B')]
        "), 1);

    claim("block `/* */` immediately before a decl is leading",
        &mut tree, "//comment[leading][.='/* leading block */']", 1);

    claim("blank-line break: floating comment has no marker",
        &mut tree, "//comment[.='// floating one'][not(leading) and not(trailing)]", 1);

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

    claim("`#` line comment becomes <comment>",
        &mut tree, "//comment[.='# floating one']", 1);

    claim("inline `#` after code is trailing",
        &mut tree, "//comment[trailing][.='# trailing single']", 1);

    claim("two adjacent `#` comments merge into one <comment>",
        &mut tree, &multi_xpath("
            //comment[leading]
                [contains(., 'leading one')]
                [contains(., 'leading two')]
        "), 1);

    claim("`# leading on bare` is leading on the assignment",
        &mut tree, "//comment[leading][.='# leading on bare']", 1);

    claim("blank-line break: floating comment has no marker",
        &mut tree, "//comment[.='# floating one'][not(leading) and not(trailing)]", 1);

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

    claim("single-line `//` after `;` on same line is trailing",
        &mut tree, "//comment[trailing][.='// trailing single']", 1);

    claim("adjacent `//` comments merge into one <comment>",
        &mut tree, &multi_xpath("
            //comment[leading]
                [contains(., 'leading first')]
                [contains(., 'leading second')]
        "), 1);

    claim("block `/* */` immediately before a decl is leading",
        &mut tree, "//comment[leading][.='/* leading block */']", 1);

    claim("blank-line break: floating comment has no marker",
        &mut tree, "//comment[.='// floating'][not(leading) and not(trailing)]", 1);

    claim("trailing and leading are mutually exclusive",
        &mut tree, "//comment[trailing and leading]", 0);

    claim("no raw tree-sitter `line_comment` / `block_comment` leaks",
        &mut tree, "//line_comment | //block_comment", 0);
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

    claim("`//` line comment becomes <comment>",
        &mut tree, "//comment[.='// line']", 1);

    claim("`/* */` block becomes <comment>",
        &mut tree, "//comment[.='/* block */']", 1);

    claim("`///` outer doc becomes <comment>",
        &mut tree, "//comment[starts-with(., '///')]", 1);

    claim("`//!` inner doc becomes <comment>",
        &mut tree, "//comment[starts-with(., '//!')]", 1);

    claim("no raw tree-sitter `line_comment` / `block_comment` / `doc_comment` leaks",
        &mut tree, "//line_comment | //block_comment | //doc_comment", 0);

    claim("`//` after `,` on same line is trailing",
        &mut tree, "//comment[trailing][.='// trailing single']", 1);

    claim("two adjacent `///` doc lines merge into one <comment>",
        &mut tree, &multi_xpath("
            //comment[leading]
                [contains(., 'outer doc line one')]
                [contains(., 'outer doc line two')]
        "), 1);

    claim("blank-line break: floating `// floating` has no marker",
        &mut tree, "//comment[.='// floating'][not(leading) and not(trailing)]", 1);

    claim("trailing and leading are mutually exclusive",
        &mut tree, "//comment[trailing and leading]", 0);
}
