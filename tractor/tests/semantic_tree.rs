//! Semantic-tree invariant tests.
//!
//! Each test pins down one design-principle invariant with an explicit
//! XPath assertion. When an assertion fails, consult the cited
//! principle and the invariant description before touching the test.
//! The goal is that a failing assertion names the violated principle
//! clearly enough that a reviewer (or a coding agent) cannot "fix" it
//! by simply flipping the expected value.
//!
//! See `specs/tractor-parse/semantic-tree/design.md` for the principle
//! catalogue referenced in the comments below.
//!
//! Each test owns a minimal inline source and a handful of assertions;
//! no shared fixture files. If coverage feels thin, add a test — the
//! helpers are designed for one-liners.

use std::sync::Arc;
use tractor::{parse, Match, ParseInput, ParseOptions, XPathEngine, XeeParseResult};

fn parse_src(lang: &str, source: &str) -> XeeParseResult {
    parse(
        ParseInput::Inline { content: source, file_label: "<semantic_tree_test>" },
        ParseOptions {
            language: Some(lang),
            tree_mode: None,
            ignore_whitespace: false,
            parse_depth: None,
        },
    )
    .expect("parse should succeed")
}

fn query(tree: &mut XeeParseResult, xpath: &str) -> Vec<Match> {
    let engine = XPathEngine::new();
    engine
        .query_documents(
            &mut tree.documents,
            tree.doc_handle,
            xpath,
            tree.source_lines.clone(),
            &tree.file_path,
        )
        .unwrap_or_else(|e| panic!("query `{}` failed: {:?}", xpath, e))
}

/// Assert the query matches exactly `expected` nodes. `invariant`
/// names the design rule being enforced — surfaces in the failure
/// message so reviewers know why the assertion exists.
#[track_caller]
fn assert_count(tree: &mut XeeParseResult, xpath: &str, expected: usize, invariant: &str) {
    let got = query(tree, xpath).len();
    assert_eq!(
        got, expected,
        "Invariant violated — {}\n  query: `{}`\n  matched {} nodes, expected {}",
        invariant, xpath, got, expected
    );
}

/// Reason-first shape claim — same effect as `assert_count` but the
/// reason reads before the technical XPath, which is much easier to
/// scan in lists of consecutive claims about a single tree.
///
/// Convention: `claim("reason it should hold", tree, xpath, expected)`.
#[track_caller]
fn claim(reason: &str, tree: &mut XeeParseResult, xpath: &str, expected: usize) {
    let got = query(tree, xpath).len();
    assert_eq!(
        got, expected,
        "Shape claim violated — {}\n  query: `{}`\n  matched {} nodes, expected {}",
        reason, xpath, got, expected
    );
}

/// Assert the query returns at least one match whose text value
/// equals `expected`.
#[track_caller]
fn assert_value(tree: &mut XeeParseResult, xpath: &str, expected: &str, invariant: &str) {
    let matches = query(tree, xpath);
    if matches.is_empty() {
        panic!(
            "Invariant violated — {}\n  query: `{}`\n  returned no matches (expected value {:?})",
            invariant, xpath, expected
        );
    }
    let got = &matches[0].value;
    assert_eq!(
        got, expected,
        "Invariant violated — {}\n  query: `{}`\n  first match value = {:?}, expected {:?}",
        invariant, xpath, got, expected
    );
}

/// Silence unused-Arc warning on platforms that don't see all helpers used.
#[allow(dead_code)]
fn _arc_sentinel(_: Arc<Vec<String>>) {}


// ===========================================================================
// Cross-language: decorator / annotation / attribute topology
//
// The element name is idiomatic per language (Python uses <decorator>,
// Java <annotation>, C#/PHP/Rust <attribute>) but the STRUCTURAL
// TOPOLOGY is shared: the thing lives as a direct child of the
// decorated/annotated declaration, with a <name> child holding the
// qualifier name as text. No language uses an enclosing wrapper like
// <decorated> or <attributes>.
// ===========================================================================

mod decorator_topology {
    use super::*;

    #[test]
    fn python_decorator_is_direct_child() {
        let mut tree = parse_src("python", "@dataclass\nclass X: pass\n");
        assert_count(
            &mut tree,
            "//class/decorator[name='dataclass']",
            1,
            "Python decorator is a direct child of the decorated <class>",
        );
        assert_count(
            &mut tree,
            "//decorated",
            0,
            "no <decorated> wrapper — topology matches Java/C#/Rust",
        );
    }

    #[test]
    fn java_annotation_is_direct_child() {
        let mut tree = parse_src(
            "java",
            "class X { @Override public void f() {} }",
        );
        assert_count(
            &mut tree,
            "//method/annotation[name='Override']",
            1,
            "Java annotation is a direct child of the annotated <method>",
        );
    }

    #[test]
    fn csharp_attribute_is_direct_child() {
        let mut tree = parse_src(
            "csharp",
            "class X { [Obsolete] [MaxLength(50)] public string Name; }",
        );

        claim("C# attribute is a direct child of the attributed declaration",
            &mut tree, "//field/attribute[name='Obsolete']", 1);

        claim("attribute with arguments still has exactly one <name> child",
            &mut tree, "//attribute[name='MaxLength']/name", 1);

        claim("attribute <name> holds the identifier as text (no nested <name>)",
            &mut tree, "//attribute[name='MaxLength']/name/*", 0);
    }

    #[test]
    fn rust_attribute_is_flat() {
        let mut tree = parse_src("rust", "#[derive(Debug)] struct S;\n");
        // #[derive] surfaces as a sibling `<attribute>` at the file
        // level — `attribute_item` wrapper was flattened.
        assert_count(
            &mut tree,
            "//attribute[name='derive']",
            1,
            "Rust attribute flattens: <attribute> with <name> child, not nested",
        );
        // Inner attributes (`#![…]`) carry an <inner/> marker to
        // distinguish from outer (`#[…]`) attributes.
        let mut inner = parse_src("rust", "#![allow(dead_code)]\nfn f() {}\n");
        assert_count(
            &mut inner,
            "//attribute[inner][name='allow']",
            1,
            "Rust inner attribute carries <inner/> marker",
        );
    }

    #[test]
    fn php_attribute_is_direct_child() {
        let mut tree = parse_src(
            "php",
            "<?php #[Deprecated] class X {}\n",
        );
        assert_count(
            &mut tree,
            "//class/attribute[name='Deprecated']",
            1,
            "PHP attribute is a direct child of the attributed <class>",
        );
    }
}

// ===========================================================================
// Cross-language: interpolated string shape
//
// Every language that supports string interpolation wraps the
// interpolated expression in an `<interpolation>` element inside
// `<string>` (or `<template>` in TS). The element name is shared;
// the delimiter tokens (`${` / `#{` / `{` / `$`) live as text inside
// the `<string>` (or, for some languages, inside the `<interpolation>`)
// but the queryable shape `//string/interpolation/<expr>` works
// uniformly across languages.
// ===========================================================================

mod interpolation_shape {
    use super::*;

    #[test]
    fn python_fstring() {
        let mut tree = parse_src("python", "x = f\"hi {name}!\"\n");
        assert_count(
            &mut tree,
            "//string/interpolation/name[.='name']",
            1,
            "Python f-string interpolation wraps the expression",
        );
    }

    #[test]
    fn typescript_template() {
        let mut tree = parse_src(
            "typescript",
            "const s = `hello ${name}!`;\n",
        );
        assert_count(
            &mut tree,
            "//template/interpolation/name[.='name']",
            1,
            "TypeScript template interpolation wraps the expression",
        );
    }

    #[test]
    fn ruby_double_quote() {
        let mut tree = parse_src(
            "ruby",
            "s = \"hi #{name}!\"\n",
        );
        assert_count(
            &mut tree,
            "//string/interpolation/name[.='name']",
            1,
            "Ruby double-quote interpolation wraps the expression",
        );
    }

    #[test]
    fn csharp_interpolated_string() {
        let mut tree = parse_src(
            "csharp",
            "class X { string s = $\"hi {Name}!\"; }",
        );
        assert_count(
            &mut tree,
            "//string/interpolation/name[.='Name']",
            1,
            "C# interpolated string wraps the expression",
        );
    }

    #[test]
    fn php_variable_interpolation() {
        let mut tree = parse_src(
            "php",
            "<?php $s = \"hi $name!\";\n",
        );
        assert_count(
            &mut tree,
            "//string/interpolation/variable/name[.='name']",
            1,
            "PHP variable interpolation wraps the expression",
        );
    }

    #[test]
    fn php_complex_interpolation() {
        let mut tree = parse_src(
            "php",
            "<?php $s = \"x {$obj->method()}\";\n",
        );
        assert_count(
            &mut tree,
            "//string/interpolation/call",
            1,
            "PHP complex interpolation wraps the expression",
        );
    }
}

// ===========================================================================
// Feature-grouped shape tests
//
// Reconsidered approach: instead of (or in addition to) per-language
// fixture files in `tests/integration/features/<feature>/`, assert the
// shape claim DIRECTLY in code, grouped by feature.
//
// Each `mod <feature>` collects every language's shape assertions for
// that feature, with multi-line indented XPath strings for legibility.
// XPath is whitespace-insensitive between path steps, so the
// indentation is purely a readability aid.
//
// Convention:
//   - Source code uses raw strings, indented to fit the test.
//   - **Be compact.** A shape claim should fit on one line whenever
//     the path is short and the predicates fit. Only break across
//     lines when the path is genuinely deep, or when several sibling
//     structural conditions need their own line for clarity.
//
//   - When breaking, indent so the path mirrors the tree. Two
//     equivalent styles — pick whichever reads better:
//
//     **Path** — counts the leaf:
//     ```
//     //class
//         /body
//             /method[public][returns/type[name='int']]
//     ```
//
//     **Bracket-predicate** — counts the root; nesting via `[…]`:
//     ```
//     //class[
//         body/method[public][returns/type[name='int']]
//     ]
//     ```
//
//   - Combine sibling predicates on the same node with `and`:
//     `comment[not(leading) and not(trailing)]` — not separate `[…]`
//     blocks. Bracket nesting is for HIERARCHY only.
//
//   - Don't mention things you don't care about. If the test is about
//     trailing comments, write `//comment[trailing]`, not
//     `//class/body/comment[trailing]` — unless the position matters.
// ===========================================================================

/// Pretty-print helper for multi-line XPath. Drops whitespace
/// OUTSIDE of `'…'` and `"…"` string literals so queries can be
/// written with indentation in source. Whitespace inside literals
/// (e.g. `[.='// instance counter']`) is preserved verbatim.
fn multi_xpath(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_quote: Option<char> = None;
    for c in s.chars() {
        match in_quote {
            Some(q) => {
                out.push(c);
                if c == q { in_quote = None; }
            }
            None if c == '\'' || c == '"' => {
                out.push(c);
                in_quote = Some(c);
            }
            None if c.is_whitespace() => {}
            None => out.push(c),
        }
    }
    out
}

mod comments {
    use super::*;

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

    /// TypeScript (and JS) currently emit bare `<comment>` with no
    /// leading/trailing classification — the C# attachment classifier
    /// hasn't been ported yet (see proposal C1). When it lands, add
    /// the classification claims here mirroring `csharp()`.
    #[test]
    fn typescript() {
        let mut tree = parse_src(
            "typescript",
            r#"
                // single
                class X {
                    x: number; // inline
                    /* block */
                    y: string;
                    /** JSDoc */
                    method() {}
                }
            "#,
        );

        claim("`//` line comment becomes <comment>",
            &mut tree, "//comment[.='// single']", 1);

        claim("`/* */` block becomes <comment>",
            &mut tree, "//comment[.='/* block */']", 1);

        claim("JSDoc `/** */` becomes <comment>",
            &mut tree, "//comment[starts-with(., '/**')][contains(., 'JSDoc')]", 1);

        claim("no raw tree-sitter `line_comment` / `block_comment` leaks",
            &mut tree, "//line_comment | //block_comment", 0);
    }

    /// Python `#` comments. Tree-sitter calls them `comment`; tractor
    /// renames to `<comment>` uniformly.
    #[test]
    fn python() {
        let mut tree = parse_src(
            "python",
            r#"
# module-level
class X:
    """docstring stays a string, not a comment"""
    x = 1  # inline
    # before y
    y = 2
"#,
        );

        claim("`#` line comment becomes <comment>",
            &mut tree, "//comment[.='# module-level']", 1);

        claim("inline `#` after code is still <comment>",
            &mut tree, "//comment[.='# inline']", 1);

        claim("docstring is a <string>, NOT a <comment>",
            &mut tree, "//comment[contains(., 'docstring')]", 0);

        claim("docstring lives as a <string> child of <class>",
            &mut tree, "//class//string[contains(., 'docstring')]", 1);
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
}

mod accessor_flattening {
    use super::*;

    /// Property accessors are direct siblings of <property>; no
    /// <accessor_list> wrapper. Each accessor carries an empty marker
    /// (<get/>/<set/>/<init/>) uniformly across auto-form and bodied
    /// form (Principles #12, #13).
    #[test]
    fn csharp() {
        let mut tree = parse_src("csharp", r#"
            class Accessors
            {
                public int AutoProp { get; set; }

                private int _backing;
                public int Manual
                {
                    get { return _backing; }
                    set { _backing = value; }
                }

                public int ReadOnly { get; }
                public int WriteOnly { set { _backing = value; } }
            }
        "#);

        claim("no <accessor_list> wrapper anywhere",
            &mut tree, "//accessor_list", 0);

        claim("auto-form get + bodied get + read-only get",
            &mut tree, "//accessor[get]", 3);

        claim("auto-form set + bodied set + write-only set",
            &mut tree, "//accessor[set]", 3);

        claim("AutoProp has 2 accessors as direct siblings of <property>",
            &mut tree, "//property[name='AutoProp']/accessor", 2);

        claim("Manual property has bodied accessors with block bodies",
            &mut tree, "//property[name='Manual']/accessor/body/block", 2);

        claim("ReadOnly has only get",
            &mut tree, "//property[name='ReadOnly']/accessor[set]", 0);
    }
}

mod accessors {
    use super::*;

    /// TypeScript `get foo()` / `set foo(v)` carry <get/>/<set/>
    /// markers on <method>. //method[get] picks them out uniformly
    /// regardless of body shape.
    #[test]
    fn typescript() {
        let mut tree = parse_src("typescript", r#"
            class Counter {
                private _value = 0;

                get value(): number { return this._value; }
                set value(v: number) { this._value = v; }
                static get singleton(): Counter { return new Counter(); }
            }
        "#);

        claim("two getter methods (instance + static)",
            &mut tree, "//method[get]", 2);

        claim("one setter method",
            &mut tree, "//method[set]", 1);

        claim("get/set on accessor methods imply <public/>",
            &mut tree, "//method[(get or set) and not(public)]", 0);

        claim("get and set markers are mutually exclusive on a method",
            &mut tree, "//method[get and set]", 0);
    }
}

mod arrow_function {
    use super::*;

    /// Principle #5 — `arrow_function` renames to <arrow> (JS-native
    /// vocabulary; distinct from <function> declarations).
    #[test]
    fn typescript() {
        let mut tree = parse_src("typescript", r#"
            const f = (x: number) => x + 1;
        "#);

        claim("arrow_function renames to <arrow>",
            &mut tree, "//arrow", 1);

        claim("no raw `arrow_function` kind leak",
            &mut tree, "//arrow_function", 0);
    }
}

mod async_generator {
    use super::*;

    /// async / generator lift to empty markers on <function> /
    /// <method>. Every async/generator declaration carries the
    /// applicable markers (Principle #9 exhaustive markers).
    #[test]
    fn typescript() {
        let mut tree = parse_src("typescript", r#"
            async function fetchOne(): Promise<number> { return 1; }
            function* counter(): Generator<number> { yield 1; }
            async function* stream(): AsyncGenerator<number> { yield 1; }
            class Service {
                async load(): Promise<void> {}
                *keys(): Generator<string> { yield "a"; }
            }
        "#);

        claim("async function fetchOne",
            &mut tree, "//function[async and not(generator)][name='fetchOne']", 1);

        claim("generator function counter",
            &mut tree, "//function[generator and not(async)][name='counter']", 1);

        claim("async generator function stream",
            &mut tree, "//function[async and generator][name='stream']", 1);

        claim("async method load",
            &mut tree, "//method[async and not(generator)][name='load']", 1);

        claim("generator method keys",
            &mut tree, "//method[generator and not(async)][name='keys']", 1);
    }
}

mod augmented_assign {
    use super::*;

    /// Goal #5: augmented_assignment unifies with plain assignment
    /// as <assign> plus an <op> child carrying the compound operator.
    /// A single //assign query matches every assignment.
    #[test]
    fn python() {
        let mut tree = parse_src("python", r#"
def ops():
    x = 0
    x += 1
    x -= 2
    x *= 3
    x //= 2
    x **= 2
    x &= 0xFF
    x |= 0x10
    x ^= 0x01
    x <<= 1
    x >>= 1
"#);

        claim("11 statement-level assignments (1 plain + 10 compound)",
            &mut tree, "//body/assign", 11);

        claim("plain `=` is the only top-level assign without an <op>",
            &mut tree, "//body/assign[not(op)]", 1);

        claim("10 compound assignments carry an <op> child",
            &mut tree, "//body/assign/op", 10);

        claim("`+=` carries assign[plus] marker",
            &mut tree, "//assign/op/assign[plus]", 1);

        claim("`-=` carries assign[minus] marker",
            &mut tree, "//assign/op/assign[minus]", 1);

        claim("`**=` carries assign[power] marker",
            &mut tree, "//assign/op/assign[power]", 1);

        claim("bitwise compound ops carry assign/bitwise[*] markers",
            &mut tree, "//assign/op/assign/bitwise[and] | //assign/op/assign/bitwise[or] | //assign/op/assign/bitwise[xor]", 3);

        claim("shift compound ops carry assign/shift[*] markers",
            &mut tree, "//assign/op/assign/shift[left] | //assign/op/assign/shift[right]", 2);
    }
}

mod collection_markers {
    use super::*;

    /// Python collection literals unify by produced type. <list>,
    /// <dict>, <set>, <generator> carry exhaustive <literal/> or
    /// <comprehension/> markers so queries can distinguish
    /// `[x for x in xs]` from `[1, 2, 3]` without kind-specific
    /// element names.
    #[test]
    fn python() {
        let mut tree = parse_src("python", r#"
nums = [1, 2, 3]
squares = [x * x for x in nums]
pairs = {"a": 1, "b": 2}
inverted = {v: k for k, v in pairs.items()}
unique = {1, 2, 3}
uniq_sq = {x * x for x in nums}
gen = (x for x in nums)
"#);

        claim("list literal carries <literal/>",
            &mut tree, "//list[literal]", 1);

        claim("list comprehension carries <comprehension/>",
            &mut tree, "//list[comprehension]", 1);

        claim("dict literal carries <literal/>",
            &mut tree, "//dict[literal]", 1);

        claim("dict comprehension carries <comprehension/>",
            &mut tree, "//dict[comprehension]", 1);

        claim("set literal carries <literal/>",
            &mut tree, "//set[literal]", 1);

        claim("set comprehension carries <comprehension/>",
            &mut tree, "//set[comprehension]", 1);

        claim("generator expression renders as <generator>",
            &mut tree, "//generator", 1);

        claim("literal and comprehension are mutually exclusive on collections",
            &mut tree, "//*[literal and comprehension]", 0);
    }
}

mod constructor_rename {
    use super::*;

    /// `ctor` -> `<constructor>` (Principle #2: full names over
    /// abbreviations).
    #[test]
    fn java() {
        let mut tree = parse_src("java", r#"
            class Point {
                int x, y;
                Point() { this(0, 0); }
                Point(int x, int y) { this.x = x; this.y = y; }
            }
        "#);

        claim("two constructors render as <constructor>",
            &mut tree, "//constructor", 2);

        claim("no abbreviated `ctor` element leaks",
            &mut tree, "//ctor", 0);

        claim("constructor name matches class name",
            &mut tree, "//constructor[name='Point']", 2);

        claim("zero-arg constructor's `this(...)` body is a <call>",
            &mut tree, "//constructor[not(parameter)]/body//call[this]", 1);
    }
}

mod defined_type_vs_alias {
    use super::*;

    /// Go distinguishes defined types (`type MyInt int`) from type
    /// aliases (`type Color = int`). Defined type -> <type>; alias
    /// -> <alias> (parallel with Rust / TS / C# / Java).
    #[test]
    fn go() {
        let mut tree = parse_src("go", r#"
            package main

            type MyInt int
            type Color = int
        "#);

        claim("defined type renders as <type>",
            &mut tree, "//type[name='MyInt']", 1);

        claim("alias renders as <alias>",
            &mut tree, "//alias[name='Color']", 1);

        claim("alias inner refers to underlying <type>",
            &mut tree, "//alias[name='Color']/type[name='int']", 1);

        claim("alias does NOT also render as <type> at the top level",
            &mut tree, "//file/type[name='Color']", 0);
    }
}

mod expression_list {
    use super::*;

    /// Principle #12: `expression_list` (tuple-like return/yield
    /// expressions) is a pure grouping node; drop it so the
    /// expressions become direct children of the enclosing
    /// <return>/<yield>/<assign>.
    #[test]
    fn python() {
        let mut tree = parse_src("python", r#"
def pair():
    return 1, 2

def triple():
    return "a", "b", "c"

def unpack():
    a, b = pair()
    return a + b
"#);

        claim("no <expression_list> wrapper leaks anywhere",
            &mut tree, "//expression_list", 0);

        claim("`return 1, 2` puts both ints as direct children of <return>",
            &mut tree, "//return[int='1' and int='2']", 1);

        claim("`return \"a\", \"b\", \"c\"` flattens 3 strings under <return>",
            &mut tree, "//return[count(string)=3]", 1);

        claim("tuple unpack `a, b = pair()` exposes both names directly under <assign>/left",
            &mut tree, "//assign/left[name='a' and name='b']", 1);
    }
}

mod f_strings {
    use super::*;

    /// F-strings render as <string> with <interpolation> children
    /// and bare literal text in between (Principle #12: grammar
    /// wrappers like string_start / string_content / string_end are
    /// flattened). Plain strings collapse to a text-only <string>.
    #[test]
    fn python() {
        let mut tree = parse_src("python", r#"
plain = "hello"
greeting = f"hello {name}"
status = f"hello {name}, you are {age}"
"#);

        claim("3 strings total",
            &mut tree, "//string", 3);

        claim("plain string has no <interpolation> child",
            &mut tree, "//string[not(interpolation)]", 1);

        claim("two f-strings carry interpolations",
            &mut tree, "//string[interpolation]", 2);

        claim("interpolation wraps a <name>",
            &mut tree, "//string/interpolation/name='name'", 1);

        claim("`status` f-string has 2 interpolations",
            &mut tree, "//string[count(interpolation)=2]", 1);

        claim("interpolation can match by interpolated name",
            &mut tree, "//string/interpolation[name='age']", 1);

        claim("string_content grammar wrapper flattens to text",
            &mut tree, "//string_content", 0);

        claim("string_start grammar wrapper flattens to text",
            &mut tree, "//string_start", 0);
    }
}

mod match_expression {
    use super::*;

    /// Principle #12: `match_block` (the `{ ... }` wrapper around
    /// match arms) is a pure grouping node; drop it so arms are
    /// direct siblings of <match> via <body>.
    #[test]
    fn rust() {
        let mut tree = parse_src("rust", r#"
            fn classify(n: i32) -> &'static str {
                match n {
                    0 => "zero",
                    1 | 2 | 3 => "small",
                    _ if n < 0 => "negative",
                    _ => "other",
                }
            }
        "#);

        claim("no `match_block` grammar leaf leaks",
            &mut tree, "//match_block", 0);

        claim("4 arms as siblings under <match>/<body>",
            &mut tree, "//match/body/arm", 4);

        claim("arm with literal pattern `0`",
            &mut tree, "//arm[pattern/int='0']", 1);

        claim("guard arm carries a <condition> child inside <pattern>",
            &mut tree, "//arm/pattern/condition", 1);

        claim("or-pattern uses pattern[or] markers (left-associative nesting)",
            &mut tree, "//arm/pattern/pattern[or]", 1);

        claim("each arm has a <pattern> and a <value>",
            &mut tree, "//arm[pattern and value]", 4);
    }
}

mod expression_statement {
    use super::*;

    /// Principle #5 — `expression_statement` renames to <expression>
    /// (not the raw tree-sitter kind).
    #[test]
    fn csharp() {
        let mut tree = parse_src("csharp", r#"
            class X {
                void F() {
                    int y = 0;
                    y = 1;
                }
            }
        "#);

        claim("no raw `expression_statement` kind leak",
            &mut tree, "//expression_statement", 0);

        claim("`y = 1` renders as <expression>",
            &mut tree, "//expression", 1);
    }
}

mod parenthesized_expression {
    use super::*;

    /// Principle #12 — `parenthesized_expression` is grammar
    /// bleed-through; drop the wrapper so inner expressions sit
    /// directly under their enclosing node. The parens remain as
    /// text children.
    #[test]
    fn java() {
        let mut tree = parse_src("java", r#"
            class X {
                boolean f(int n) { return (n + 1) > 0; }
            }
        "#);

        claim("no <parenthesized_expression> wrapper",
            &mut tree, "//parenthesized_expression", 0);
    }
}

mod variable_declarator {
    use super::*;

    /// Principle #2 — `variable_declarator` renames to <declarator>
    /// (no underscores in the final vocabulary, short but not
    /// abbreviated). Each declarator in a multi-variable declaration
    /// is its own <declarator>.
    #[test]
    fn java() {
        let mut tree = parse_src("java", r#"
            class X {
                void f() { int x = 1, y = 2; }
            }
        "#);

        claim("no raw `variable_declarator` kind leak",
            &mut tree, "//variable_declarator", 0);

        claim("each declarator in a multi-variable declaration is its own <declarator>",
            &mut tree, "//variable/declarator", 2);
    }
}

mod method_call {
    use super::*;

    /// Both function calls and method calls render as <call>. Method
    /// calls are distinguished by a <field> child that names the
    /// receiver and method (Rust uses field-call syntax).
    #[test]
    fn rust() {
        let mut tree = parse_src("rust", r#"
            fn use_calls() {
                let v: Vec<i32> = Vec::new();
                let n = v.len();
                let s = "hi".to_string();
                s.to_uppercase();
            }
        "#);

        claim("4 unified <call> nodes (1 path-call + 3 method-calls)",
            &mut tree, "//call", 4);

        claim("path-call `Vec::new()` has a <path> child",
            &mut tree, "//call[path[name='Vec' and name='new']]", 1);

        claim("3 method calls expose a <field> child for receiver.method",
            &mut tree, "//call/field", 3);

        claim("method `len` on receiver `v`",
            &mut tree, "//call/field[value/name='v' and name='len']", 1);

        claim("method `to_string` on a string-literal receiver",
            &mut tree, "//call/field[value/string and name='to_string']", 1);

        claim("no legacy <methodcall> element",
            &mut tree, "//methodcall", 0);
    }

    /// Java `this(…)` / `super(…)` in constructors render as <call>
    /// with a <this/> or <super/> marker — uniform with other call
    /// sites; no `explicit_constructor_invocation` raw kind leaks.
    #[test]
    fn java() {
        let mut tree = parse_src("java", r#"
            class X {
                X() { this(1); }
                X(int a) {}
                class Y extends X {
                    Y() { super(2); }
                }
            }
        "#);

        claim("`this(…)` renders as <call> with <this/> marker",
            &mut tree, "//call[this]", 1);

        claim("`super(…)` renders as <call> with <super/> marker",
            &mut tree, "//call[super]", 1);

        claim("no raw `explicit_constructor_invocation` kind leak",
            &mut tree, "//explicit_constructor_invocation", 0);
    }
}

mod modifiers {
    use super::*;

    /// Modifiers lift as empty markers on the declaration. Every
    /// access modifier is exhaustive — package-private gets an
    /// explicit <package/> marker. Markers appear in source order
    /// (source-reversibility), and the source keywords also survive
    /// as text so the enclosing node's string-value still reads like
    /// the source.
    #[test]
    fn java() {
        let mut tree = parse_src("java", r#"
            public abstract static class Modifiers {
                public static final int PUB = 1;
                private int priv = 2;
                protected int prot = 3;
                int pkg = 4;
                public synchronized void sync() {}
                public abstract static class AbsStatic {}
            }
        "#);

        claim("public static final field marks all 3 modifiers",
            &mut tree, "//field[public and static and final][declarator/name='PUB']", 1);

        claim("private field carries <private/>",
            &mut tree, "//field[private]", 1);

        claim("protected field carries <protected/>",
            &mut tree, "//field[protected]", 1);

        claim("implicit package-private surfaces as <package/>",
            &mut tree, "//field[package]", 1);

        claim("synchronized method also marks public",
            &mut tree, "//method[public and synchronized][name='sync']", 1);

        claim("nested class composes public + abstract + static markers",
            &mut tree, "//class[public and abstract and static][name='AbsStatic']", 1);

        claim("first marker on outer class is <public/> (source order)",
            &mut tree, "//class[name='Modifiers']/*[1][self::public]", 1);

        claim("second marker on outer class is <abstract/> (source order)",
            &mut tree, "//class[name='Modifiers']/*[2][self::abstract]", 1);

        claim("third marker on outer class is <static/> (source order)",
            &mut tree, "//class[name='Modifiers']/*[3][self::static]", 1);

        claim("source keywords preserved as dangling text (source-reversibility)",
            &mut tree, "//class[name='Modifiers'][contains(., 'public abstract static')]", 1);
    }
}

mod name_inlining {
    use super::*;

    /// (1) Every Ruby `identifier` becomes <name> unconditionally.
    /// (2) When a <name> wrapper sits inside method/class/module and
    /// contains a single identifier, the transform inlines its text
    /// directly, so <method><name>foo</name>… not
    /// <method><name><identifier>foo</identifier></name>….
    #[test]
    fn ruby() {
        let mut tree = parse_src("ruby", r#"
            class Calculator
              def add(a, b)
                a + b
              end
            end

            module Utils
              def self.greet(name)
                "hi, #{name}"
              end
            end
        "#);

        claim("class name is inlined text on <name>",
            &mut tree, "//class/name='Calculator'", 1);

        claim("class <name> has no <identifier> child",
            &mut tree, "//class/name/identifier", 0);

        claim("class <name> has no nested <constant> child",
            &mut tree, "//class/name/constant", 0);

        claim("module name is inlined text on <name>",
            &mut tree, "//module/name='Utils'", 1);

        claim("method name `add` is inlined text on <name>",
            &mut tree, "//method/name='add'", 1);

        claim("singleton method `self.greet` carries [singleton] marker",
            &mut tree, "//method[singleton][name='greet']", 1);

        claim("method parameters are <name> elements (identifier renamed)",
            &mut tree, "//method[name='add']/name[. ='a' or .='b']", 2);

        claim("identifiers in expressions render as <name>",
            &mut tree, "//binary/left/name[.='a']", 1);

        claim("no raw <identifier> nodes leak from Ruby grammar",
            &mut tree, "//identifier", 0);
    }
}

mod parameter_marking {
    use super::*;

    /// Every <param> carries an exhaustive marker: <required/> or
    /// <optional/>. Covers required, optional (?), defaulted, and
    /// rest parameters; also the JS-style untyped param shape.
    #[test]
    fn typescript() {
        let mut tree = parse_src("typescript", r#"
            function call(
                required: string,
                optional?: number,
                defaulted: boolean = true,
                ...rest: string[]
            ): void {}

            function noTypes(x, y) {}
        "#);

        claim("every parameter is either required or optional",
            &mut tree, "//parameter[not(required) and not(optional)]", 0);

        claim("required and optional are mutually exclusive",
            &mut tree, "//parameter[required and optional]", 0);

        claim("required: 1 (required) + defaulted + rest + 2 untyped = 5",
            &mut tree, "//parameter[required]", 5);

        claim("optional `?` is the only <parameter[optional]>",
            &mut tree, "//parameter[optional]", 1);

        claim("rest parameter exposes a <rest> child",
            &mut tree, "//parameter[rest]", 1);

        claim("defaulted parameter has a <value> child",
            &mut tree, "//parameter[name='defaulted'][value]", 1);
    }

    /// Ruby splat parameters distinguish iterable `*args` (list) from
    /// mapping `**kwargs` (dict); keyword parameters (`key:`) carry a
    /// <keyword/> marker distinguishing them from positional ones.
    #[test]
    fn ruby() {
        let mut tree = parse_src("ruby", r#"
            def f(a, *xs, key: 1, **kw)
            end
        "#);

        claim("`*xs` carries spread[list]",
            &mut tree, "//spread[list]", 1);

        claim("`**kw` carries spread[dict]",
            &mut tree, "//spread[dict]", 1);

        claim("`key:` keyword parameter carries <keyword/>",
            &mut tree, "//parameter[keyword]", 1);
    }
}

mod reference_type {
    use super::*;

    /// Reference types `&T` / `&mut T` / `&'a T` render as a single
    /// <type> with a <borrowed/> marker (Principles #14 + #13). The
    /// inner referenced type is a nested <type> child.
    #[test]
    fn rust() {
        let mut tree = parse_src("rust", r#"
            fn read(s: &str) -> &str { s }
            fn write(buf: &mut Vec<u8>) {}
            fn static_ref() -> &'static str { "" }
        "#);

        claim("4 reference types: 2x &str (param + return) + &mut Vec<u8> + &'static str",
            &mut tree, "//type[borrowed]", 4);

        claim("only the &mut Vec<u8> carries the mut marker",
            &mut tree, "//type[borrowed and mut]", 1);

        claim("borrowed type wraps the referenced type as a nested <type>",
            &mut tree, "//type[borrowed]/type", 4);

        claim("`&'static` exposes a <lifetime> child",
            &mut tree, "//type[borrowed]/lifetime[name='static']", 1);

        claim("inner type of &mut is the generic Vec<u8>",
            &mut tree, "//type[borrowed and mut]/type[generic][name='Vec']", 1);

        claim("no legacy <ref> element",
            &mut tree, "//ref", 0);
    }
}

mod strings {
    use super::*;

    /// Go strings: interpreted (double-quoted, escapes) and raw
    /// (backtick, no escapes). Both render as <string>; raw strings
    /// carry a <raw/> marker (Principle #13).
    #[test]
    fn go() {
        let mut tree = parse_src("go", r#"
            package main

            const normal = "hello\nworld"
            const raw = `hello world`
            const pattern = `^\d+$`
        "#);

        claim("3 strings total — bare //string catches both forms",
            &mut tree, "//string", 3);

        claim("interpreted string has no <raw/> marker",
            &mut tree, "//string[not(raw)]", 1);

        claim("two backtick strings carry <raw/>",
            &mut tree, "//string[raw]", 2);

        claim("raw and not-raw partition the strings",
            &mut tree, "//string[raw and not(raw)]", 0);
    }

    /// Rust strings: regular `"..."` and raw `r"..."`. Both render as
    /// <string>; raw forms carry a <raw/> marker.
    #[test]
    fn rust() {
        let mut tree = parse_src("rust", r#"
            fn f() {
                let _ = r"raw";
                let _ = "normal";
            }
        "#);

        claim("raw string carries <raw/> marker",
            &mut tree, "//string[raw]", 1);

        claim("both raw and normal strings use <string>",
            &mut tree, "//string", 2);
    }
}

mod array_literals {
    use super::*;

    /// Ruby percent-literal arrays collapse to <array> with a
    /// <string/> / <symbol/> marker so the element name matches a
    /// normal array while the flavor stays queryable.
    #[test]
    fn ruby() {
        let mut tree = parse_src("ruby", r#"
            A = %w[one two]
            B = %i[alpha beta]
            C = [1, 2]
        "#);

        claim("%w[…] carries <string/>",
            &mut tree, "//array[string]", 1);

        claim("%i[…] carries <symbol/>",
            &mut tree, "//array[symbol]", 1);

        claim("all three forms collapse to <array>",
            &mut tree, "//array", 3);
    }
}

mod spread {
    use super::*;

    /// `*args` and `**kwargs` collapse to <spread> but carry a
    /// <list/> / <dict/> marker that survives argument, pattern, and
    /// literal contexts so shape queries work without string matching
    /// on `*` / `**` operator text.
    #[test]
    fn python() {
        let mut tree = parse_src("python", r#"
def f(*args, **kwargs): pass
g(*xs, **kw)
[*a, *b]
{**a, **b}
"#);

        claim("`*args`, `g(*xs)`, `[*a]`, `[*b]` all carry spread[list]",
            &mut tree, "//spread[list]", 4);

        claim("`**kwargs`, `g(**kw)`, `{**a}`, `{**b}` all carry spread[dict]",
            &mut tree, "//spread[dict]", 4);
    }
}

mod pattern_markers {
    use super::*;

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
}

mod type_markers {
    use super::*;

    /// Rust type flavors all collapse to <type> with a shape marker —
    /// function, tuple, array, pointer, never, unit, dyn. (The `[T]`
    /// inside `&[T]` is treated as `array_type` by tree-sitter-rust,
    /// so `slice` markers only appear for explicit slice forms — which
    /// the cross-file blueprint snapshot covers separately.)
    #[test]
    fn rust() {
        let mut tree = parse_src("rust", r#"
            fn f(cb: fn(i32) -> i32, t: (i32, i32), a: [u8; 4], p: *const u8) -> ! { loop {} }
            fn g() -> () {}
            fn h(d: &dyn Drawable) {}
        "#);

        claim("fn type carries <function/>",
            &mut tree, "//type[function]", 1);

        claim("tuple type carries <tuple/>",
            &mut tree, "//type[tuple]", 1);

        claim("array type carries <array/>",
            &mut tree, "//type[array]", 1);

        claim("pointer type carries <pointer/>",
            &mut tree, "//type[pointer]", 1);

        claim("never type carries <never/>",
            &mut tree, "//type[never]", 1);

        claim("unit type carries <unit/>",
            &mut tree, "//type[unit]", 1);

        claim("dyn trait object carries <dynamic/>",
            &mut tree, "//type[dynamic]", 1);
    }

    /// C# type flavors — array/tuple/nullable — all collapse to
    /// <type> with a shape marker. `nullable_type` gets its
    /// <nullable/> marker via a direct rewrite (not the map) but the
    /// end shape is the same.
    #[test]
    fn csharp() {
        let mut tree = parse_src("csharp", r#"
            class X {
                int[] a;
                (int, string) t;
                int? n;
            }
        "#);

        claim("array type carries <array/>",
            &mut tree, "//type[array]", 1);

        claim("tuple type carries <tuple/>",
            &mut tree, "//type[tuple]", 1);

        claim("nullable type carries <nullable/>",
            &mut tree, "//type[nullable]", 1);
    }

    /// TypeScript type flavors all collapse to <type> with a shape
    /// marker (Principle #9) so `//type[union]`, `//type[tuple]`,
    /// etc. work uniformly without matching on text.
    #[test]
    fn typescript() {
        let mut tree = parse_src("typescript", r#"
            type A = string | number;
            type B = string & object;
            type C = [string, number];
            type D = string[];
            type E = 'idle';
            type F = (x: number) => number;
            type G = { x: number };
            type H = readonly number[];
        "#);

        claim("union type carries <union/>",
            &mut tree, "//type[union]", 1);

        claim("intersection type carries <intersection/>",
            &mut tree, "//type[intersection]", 1);

        claim("tuple type carries <tuple/>",
            &mut tree, "//type[tuple]", 1);

        // `number[]` is array_type; `readonly number[]` wraps in readonly_type.
        claim("array types carry <array/> (number[] + readonly number[])",
            &mut tree, "//type[array]", 2);

        claim("literal type carries <literal/>",
            &mut tree, "//type[literal]", 1);

        claim("function type carries <function/>",
            &mut tree, "//type[function]", 1);

        claim("object type carries <object/>",
            &mut tree, "//type[object]", 1);

        claim("readonly type carries <readonly/>",
            &mut tree, "//type[readonly]", 1);
    }

    /// Java `void` carries an additional <void/> marker on top of the
    /// `<name>void</name>` text leaf — the marker is a query
    /// shortcut, not a replacement. Other primitives keep just the
    /// name child.
    #[test]
    fn java() {
        let mut tree = parse_src("java", r#"
            class X {
                void f() {}
                int g() { return 0; }
            }
        "#);

        claim("void type has both <void/> marker AND <name>void</name>",
            &mut tree, "//type[void][name='void']", 1);

        claim("exactly one void type in the source",
            &mut tree, "//type[void]", 1);

        claim("non-void types have no <void/> marker",
            &mut tree, "//type[not(void)]", 1);
    }
}

mod struct_expression {
    use super::*;

    /// Struct construction `Point { x: 1, y: 2 }` renders as
    /// <literal> with a <name> child for the struct name and
    /// <field> siblings for each initializer. Symmetric with JS/C#
    /// object construction: //literal[name='Point'] finds every
    /// Point construction site.
    #[test]
    fn rust() {
        let mut tree = parse_src("rust", r#"
            struct Point { x: i32, y: i32 }

            fn make() {
                let p = Point { x: 1, y: 2 };
                let q = Point { x: 0, ..p };
            }
        "#);

        claim("two Point construction sites",
            &mut tree, "//literal[name='Point']", 2);

        claim("struct name lives as <name> on <literal> (NOT a <type>)",
            &mut tree, "//literal/type", 0);

        claim("first construction has 2 plain fields, no [base]",
            &mut tree, "//literal[name='Point'][not(body/field[base])]/body/field", 2);

        claim("second construction has a [base] field for `..p`",
            &mut tree, "//literal/body/field[base][name='p']", 1);

        claim("field initializers carry <value> children",
            &mut tree, "//literal/body/field[name='x']/value/int", 2);
    }
}

mod struct_interface_hoist {
    use super::*;

    /// Goal #5 mental model — `type Foo struct { … }` and
    /// `type Foo interface { … }` hoist: the outer element becomes
    /// <struct> or <interface> directly instead of the Go-spec
    /// `<type>` wrapper.
    #[test]
    fn go() {
        let mut tree = parse_src("go", r#"
            package main

            type Config struct {
                Host string
                Port int
            }

            type Greeter interface {
                Greet() string
            }
        "#);

        claim("struct hoists to top level (no enclosing <type>)",
            &mut tree, "//file/struct[name='Config']", 1);

        claim("interface hoists to top level (no enclosing <type>)",
            &mut tree, "//file/interface[name='Greeter']", 1);

        claim("uppercase struct name carries <exported/>",
            &mut tree, "//struct[exported][name='Config']", 1);

        claim("uppercase interface name carries <exported/>",
            &mut tree, "//interface[exported][name='Greeter']", 1);

        claim("the `type` wrapper does NOT also surface a <type> for the struct",
            &mut tree, "//file/type[name='Config']", 0);
    }
}

mod spec_flattening {
    use super::*;

    /// Principle #12 — Go's `const_spec` / `var_spec` / `import_spec`
    /// are grammar wrappers around `name = value` / `path`. Flatten
    /// so a declaration reads as `<const>const<name>x</name>=<value>1</value></const>`
    /// rather than burying the assignment inside an opaque spec
    /// element.
    #[test]
    fn go() {
        let mut tree = parse_src("go", r#"
            package main

            const x = 1
            var y = 2
        "#);

        claim("no <const_spec> wrapper",
            &mut tree, "//const_spec", 0);

        claim("no <var_spec> wrapper",
            &mut tree, "//var_spec", 0);

        claim("const's name is a direct child, not buried under const_spec",
            &mut tree, "//const[name='x']", 1);
    }
}

mod switch_markers {
    use super::*;

    /// `switch x.(type) { … }` and a regular `switch x { … }` both
    /// collapse to <switch>. The type switch carries a <type/>
    /// marker so `//switch[type]` picks out every type switch.
    #[test]
    fn go() {
        let mut tree = parse_src("go", r#"
            package main

            func f(x interface{}) {
                switch x.(type) { case int: }
                switch x { case 1: }
            }
        "#);

        claim("type switch carries <type/> marker",
            &mut tree, "//switch[type]", 1);

        claim("both regular and type switch collapse to <switch>",
            &mut tree, "//switch", 2);
    }
}

mod type_declaration {
    use super::*;

    /// Go's `type_declaration` wrapper is dropped; `type_spec`
    /// renders as <type> directly. Parallel with struct/interface
    /// declarations so //type queries find every declared type.
    #[test]
    fn go() {
        let mut tree = parse_src("go", r#"
            package main

            type ID uint64

            type User struct {
                Name string
                Age  int
            }

            type Greeter interface {
                Greet() string
            }
        "#);

        claim("plain `type ID uint64` renders as <type>",
            &mut tree, "//file/type[name='ID']", 1);

        claim("struct/interface forms do NOT also produce a <type> wrapper",
            &mut tree, "//file/type[name='User'] | //file/type[name='Greeter']", 0);

        claim("no `type_declaration` grammar wrapper leaks",
            &mut tree, "//type_declaration", 0);

        claim("inner referenced type of `type ID uint64`",
            &mut tree, "//type[name='ID']/type[name='uint64']", 1);
    }
}

mod typedef {
    use super::*;

    /// Rust `type_item` renders as <alias> (parallel with
    /// TS / Java / C#).
    #[test]
    fn rust() {
        let mut tree = parse_src("rust", r#"
            type Id = u32;
            type Mapping<T> = std::collections::HashMap<String, T>;
        "#);

        claim("two aliases declared",
            &mut tree, "//alias", 2);

        claim("no raw `type_item` grammar leaf leaks",
            &mut tree, "//type_item", 0);

        claim("aliases default to <private/>",
            &mut tree, "//alias[private]", 2);

        claim("simple alias resolves to <type>",
            &mut tree, "//alias[name='Id']/type[name='u32']", 1);

        claim("generic alias declares a <generic> parameter",
            &mut tree, "//alias[name='Mapping']/generic[name='T']", 1);

        claim("no legacy <typedef> element",
            &mut tree, "//typedef", 0);
    }
}

mod visibility {
    use super::*;

    /// Visibility is exhaustive: every declaration carries either
    /// <private/> (implicit default) or <pub/> with optional
    /// restriction details.
    #[test]
    fn rust() {
        let mut tree = parse_src("rust", r#"
            fn private_fn() {}
            pub fn public_fn() {}
            pub(crate) fn crate_fn() {}
            pub(super) fn super_fn() {}

            struct PrivateStruct;
            pub struct PublicStruct;

            const PRIV: i32 = 1;
            pub const PUB: i32 = 2;
        "#);

        claim("4 functions total, every one has visibility info",
            &mut tree, "//function[private or pub]", 4);

        claim("plain `pub` produces a <pub/> marker (no restriction)",
            &mut tree, "//function[pub][name='public_fn']", 1);

        claim("`pub(crate)` exposes <pub><crate/></pub>",
            &mut tree, "//function/pub[crate]", 1);

        claim("`pub(super)` exposes <pub><super/></pub>",
            &mut tree, "//function/pub[super]", 1);

        claim("private struct carries <private/>",
            &mut tree, "//struct[private][name='PrivateStruct']", 1);

        claim("private const carries <private/>",
            &mut tree, "//const[private][name='PRIV']", 1);
    }

    /// TypeScript class members carry an exhaustive visibility marker:
    /// explicit `public/private/protected` keywords lift to markers,
    /// and members without a keyword get an implicit <public/> (TS
    /// default). Fields and methods follow the same rules.
    #[test]
    fn typescript() {
        let mut tree = parse_src("typescript", r#"
            class X {
                foo() {}
                private bar() {}
                protected baz() {}
                public qux() {}

                x = 1;
                private y = 2;
            }
        "#);

        claim("implicit default and explicit public both carry <public/> on methods",
            &mut tree, "//method[public]", 2);

        claim("explicit private method carries <private/>",
            &mut tree, "//method[private]", 1);

        claim("explicit protected method carries <protected/>",
            &mut tree, "//method[protected]", 1);

        claim("unmarked field defaults to <public/>",
            &mut tree, "//field[public]", 1);

        claim("explicit private field carries <private/>",
            &mut tree, "//field[private]", 1);
    }

    /// PHP class members carry an exhaustive visibility marker:
    /// explicit `public/private/protected` keywords lift to markers,
    /// and members without a keyword get implicit <public/> (PHP
    /// default). Methods and properties follow the same rules.
    #[test]
    fn php() {
        let mut tree = parse_src("php", r#"<?php
            class X {
                function foo() {}
                private function bar() {}
                protected function baz() {}
                public function qux() {}

                public $a;
                $b;
                private $c;
            }
        "#);

        claim("implicit default and explicit public both carry <public/> on methods",
            &mut tree, "//method[public]", 2);

        claim("explicit private method carries <private/>",
            &mut tree, "//method[private]", 1);

        claim("explicit protected method carries <protected/>",
            &mut tree, "//method[protected]", 1);

        claim("explicit and implicit public properties both carry <public/>",
            &mut tree, "//field[public]", 2);

        claim("explicit private property carries <private/>",
            &mut tree, "//field[private]", 1);
    }

    /// Python visibility uses naming convention: bare → public,
    /// `_x` → protected, `__x` → private. Dunders (`__init__`) are
    /// conventional protocol hooks and count as public. The
    /// convention applies ONLY to class members; module-level
    /// functions are not classified.
    #[test]
    fn python() {
        let mut class_tree = parse_src("python", r#"
class X:
    def foo(self): pass
    def _bar(self): pass
    def __baz(self): pass
    def __init__(self): pass
"#);

        claim("bare name and dunder both count as public",
            &mut class_tree, "//function[public]", 2);

        claim("single-underscore prefix means protected",
            &mut class_tree, "//function[protected]", 1);

        claim("double-underscore prefix means private",
            &mut class_tree, "//function[private]", 1);

        let mut module_tree = parse_src("python", r#"
def foo(): pass
def _bar(): pass
"#);

        claim("module-level functions skip the visibility injection (no public)",
            &mut module_tree, "//function[public]", 0);

        claim("module-level functions skip the visibility injection (no protected)",
            &mut module_tree, "//function[protected]", 0);
    }

    /// Go uses Go's name-capitalization export rule for visibility:
    /// every declaration carries an exhaustive <exported/> or
    /// <unexported/> marker. Applies to functions, types, and struct
    /// fields uniformly.
    #[test]
    fn go() {
        let mut tree = parse_src("go", r#"
            package main

            func Public() {}
            func private() {}

            type Exported int
            type unexported int

            type T struct {
                Public string
                private string
            }
        "#);

        claim("exported function carries <exported/>",
            &mut tree, "//function[exported]", 1);

        claim("unexported function carries <unexported/>",
            &mut tree, "//function[unexported]", 1);

        claim("every function carries one of the two markers",
            &mut tree, "//function[not(exported) and not(unexported)]", 0);

        claim("exported type carries <exported/>",
            &mut tree, "//type[exported]", 1);

        claim("unexported type carries <unexported/>",
            &mut tree, "//type[unexported]", 1);

        claim("capitalised struct field carries <exported/>",
            &mut tree, "//field[exported]", 1);

        claim("lower-case struct field carries <unexported/>",
            &mut tree, "//field[unexported]", 1);
    }
}

mod where_clause {
    use super::*;

    /// C# `where` clause constraints attach to the matching
    /// <generic> element. Shape constraints (class / struct /
    /// notnull / unmanaged / new) become empty markers that
    /// compose; type bounds wrap in <extends><type>…</type></extends>.
    #[test]
    fn csharp() {
        let mut tree = parse_src("csharp", r#"
            using System;

            class Repo<T, U, V>
                where T : class, IComparable<T>, new()
                where U : struct
                where V : notnull
            {
            }
        "#);

        claim("3 generics declared on Repo (T, U, V)",
            &mut tree, "//class[name='Repo']/generic", 3);

        claim("T composes class + new shape markers",
            &mut tree, "//generic[class and new][name='T']", 1);

        claim("U has the struct constraint",
            &mut tree, "//generic[struct][name='U']", 1);

        claim("V has the notnull constraint",
            &mut tree, "//generic[notnull][name='V']", 1);

        claim("T's IComparable<T> bound wraps in <extends><type>...",
            &mut tree, "//generic[name='T']/extends/type[name='IComparable']", 1);

        claim("U has no <extends> bound",
            &mut tree, "//generic[name='U']/extends", 0);
    }
}

mod flat_lists {
    use super::*;

    /// Principle #12: parameters / arguments / generics / accessors
    /// render as flat siblings — no <parameters> / <accessor_list> /
    /// <argument_list> / <type_parameters> wrapper element.

    #[test]
    fn csharp() {
        let mut tree = parse_src("csharp", r#"
            class FlatLists
            {
                public T First<T, U>(T a, U b, int c) where T : class
                {
                    return a;
                }

                public int Count { get; set; }

                public void Caller()
                {
                    First<string, int>("x", 1, 2);
                }
            }
        "#);

        claim("no parameter-list wrapper element",
            &mut tree, "//parameter_list | //parameters", 0);

        claim("no argument-list wrapper element",
            &mut tree, "//argument_list | //arguments", 0);

        claim("no accessor-list wrapper element",
            &mut tree, "//accessor_list", 0);

        claim("First has 3 parameters as direct siblings",
            &mut tree, "//method[name='First']/parameter", 3);

        claim("First has 2 generics as direct siblings",
            &mut tree, "//method[name='First']/generic", 2);

        claim("Property accessors are direct siblings of <property>",
            &mut tree, "//property[name='Count']/accessor", 2);
    }

    #[test]
    fn go() {
        let mut tree = parse_src("go", r#"
            package main

            func First(a string, b int, c bool) string { return a }

            func Caller() { First("x", 1, true) }

            type Config struct { Host string; Port int; Tls bool }
        "#);

        claim("no parameter-list wrapper",
            &mut tree, "//parameter_list | //parameters", 0);

        claim("no argument-list wrapper",
            &mut tree, "//argument_list | //arguments", 0);

        claim("no field-list wrapper around struct fields",
            &mut tree, "//field_declaration_list | //field_list", 0);

        claim("First has 3 parameters as direct siblings",
            &mut tree, "//function[name='First']/parameter", 3);

        claim("Caller's call has 3 argument siblings (not wrapped)",
            &mut tree, "//function[name='Caller']//call/*[self::string or self::int or self::true]", 3);

        claim("Config struct has 3 fields",
            &mut tree, "//struct[name='Config']/field", 3);
    }

    #[test]
    fn java() {
        let mut tree = parse_src("java", r#"
            class FlatLists {
                <T, U extends Comparable<U>> T first(T a, U b, int c) { return a; }

                void caller() { first("x", "y", 1); }
            }
        "#);

        claim("no parameter-list wrapper",
            &mut tree, "//parameter_list | //parameters", 0);

        claim("no type-parameter wrapper",
            &mut tree, "//type_parameters", 0);

        claim("no argument-list wrapper",
            &mut tree, "//argument_list | //arguments", 0);

        claim("first has 3 parameters as direct siblings",
            &mut tree, "//method[name='first']/parameter", 3);

        claim("first has 2 generics as direct siblings",
            &mut tree, "//method[name='first']/generic", 2);
    }

    #[test]
    fn rust() {
        let mut tree = parse_src("rust", r#"
            fn first<T, U: Clone>(a: T, b: U, c: i32) -> T { a }

            fn caller() {
                first::<String, i32>(String::from("x"), 1, 2);
            }
        "#);

        claim("no parameter-list wrapper",
            &mut tree, "//parameters | //parameter_list", 0);

        claim("no type-parameter wrapper",
            &mut tree, "//type_parameters", 0);

        claim("no argument-list wrapper",
            &mut tree, "//argument_list | //arguments", 0);

        claim("first has 3 parameters as direct siblings",
            &mut tree, "//function[name='first']/parameter", 3);

        claim("first has 2 generics as direct siblings",
            &mut tree, "//function[name='first']/generic", 2);
    }

    /// TypeScript currently retains a thin <generics> grouping
    /// element — pin that as current behaviour. Parameters and
    /// arguments still flatten.
    #[test]
    fn typescript() {
        let mut tree = parse_src("typescript", r#"
            function first<T, U>(a: T, b: U, c: number): T { return a; }
            first<string, number>("x", 1, 2);
        "#);

        claim("no parameter-list wrapper",
            &mut tree, "//parameters | //parameter_list", 0);

        claim("no argument-list wrapper",
            &mut tree, "//argument_list | //arguments", 0);

        claim("first has 3 parameters as direct siblings",
            &mut tree, "//function[name='first']/parameter", 3);

        claim("TS keeps a thin <generics> wrapper for type parameters",
            &mut tree, "//function[name='first']/generics/generic", 2);
    }
}

mod interface_public {
    use super::*;

    /// Interface members without an explicit access modifier default
    /// to <public/>. C# and Java both lift this to an exhaustive
    /// marker so a single //method[public] hits every visible
    /// interface method.

    #[test]
    fn csharp() {
        let mut tree = parse_src("csharp", r#"
            interface IShape
            {
                double Area();
                double Perimeter();
                string Name => "shape";
                public void Stroke();
            }
        "#);

        claim("3 interface methods all carry <public/>",
            &mut tree, "//interface/body/method[public]", 3);

        claim("expression-bodied property carries <public/>",
            &mut tree, "//interface/body/property[public]", 1);

        claim("no interface member is missing visibility",
            &mut tree, "//interface/body/*[(self::method or self::property) and not(public)]", 0);
    }

    /// Java pins current behaviour: implicit-public abstract methods
    /// surface as <public/>; the explicit `public void stroke()` also
    /// gets <public/>. Default methods (`default String name() {...}`)
    /// currently render with <package/> rather than <public/> — pin
    /// that as the actual current shape rather than aspiration.
    #[test]
    fn java() {
        let mut tree = parse_src("java", r#"
            interface Shape {
                double area();
                double perimeter();
                default String name() { return "shape"; }
                public void stroke();
            }
        "#);

        claim("implicit-public abstract methods carry <public/>",
            &mut tree, "//interface/body/method[public][not(body)]", 3);

        claim("explicit `public` method `stroke` also carries <public/>",
            &mut tree, "//interface/body/method[public][name='stroke']", 1);

        claim("`default` method is not classified as <public/> (current behaviour)",
            &mut tree, "//interface/body/method[name='name'][public]", 0);
    }
}

mod type_vocabulary {
    use super::*;

    /// Principle #14: every type reference wraps its name in a
    /// <name> child. No bare-text <type> nodes; type parameters use
    /// <generic>; bounds wrap in <extends>; collection-of-T uses
    /// <type[generic]> with nested <type> children.

    #[test]
    fn csharp() {
        let mut tree = parse_src("csharp", r#"
            using System.Collections.Generic;

            interface IBarker { void Bark(); }
            class Animal {}

            class Dog<T> : Animal, IBarker where T : Animal
            {
                public T Owner;
                public List<string> Tags;
                public void Bark() {}
            }
        "#);

        claim("every <type> has a <name> child (no bare-text types)",
            &mut tree, "//type[not(name)]", 0);

        claim("Dog declares one <generic> type parameter",
            &mut tree, "//class[name='Dog']/generic[name='T']", 1);

        claim("generic T with where-clause `: Animal` exposes <extends><type>",
            &mut tree, "//class[name='Dog']/generic[name='T']/extends/type[name='Animal']", 1);

        claim("class extends list combines base + interface as siblings",
            &mut tree, "//class[name='Dog']/extends/type[name='Animal' or name='IBarker']", 2);

        claim("List<string> field uses generic type with inner <type>",
            &mut tree, "//field//type[generic][name='List']/type[name='string']", 1);
    }

    #[test]
    fn java() {
        let mut tree = parse_src("java", r#"
            import java.util.List;

            class Animal {}
            interface Barker { void bark(); }
            interface Runner { void run(); }

            class Dog<T extends Animal> extends Animal implements Barker, Runner {
                int a;
                double b;
                boolean c;
                Foo e;
                List l;
                T owner;
                List<String> tags;

                public void bark() {}
                public void run() {}
            }
        "#);

        claim("every <type> has a <name> child",
            &mut tree, "//type[not(name)]", 0);

        claim("type parameter T has an <extends> bound on Animal",
            &mut tree, "//class[name='Dog']/generic[name='T']/extends/type[name='Animal']", 1);

        claim("extends list points to <type[name='Animal']>",
            &mut tree, "//class[name='Dog']/extends/type[name='Animal']", 1);

        claim("implements list has 2 <type> entries",
            &mut tree, "//class[name='Dog']/implements/type", 2);

        claim("List<String> field uses generic type with inner <type>",
            &mut tree, "//field//type[generic][name='List']/type[name='String']", 1);

        claim("primitive `int` carries name as text",
            &mut tree, "//type[name='int']", 1);

        claim("primitive `double` carries name as text",
            &mut tree, "//type[name='double']", 1);

        claim("primitive `boolean` carries name as text",
            &mut tree, "//type[name='boolean']", 1);

        claim("user-defined type `Foo` carries name as text",
            &mut tree, "//type[name='Foo']", 1);

        claim("built-in capitalized type `List` carries name as text (bare + generic forms)",
            &mut tree, "//type[name='List']", 2);
    }

    #[test]
    fn rust() {
        let mut tree = parse_src("rust", r#"
            use std::collections::HashMap;

            trait Barker { fn bark(&self); }

            struct Dog<T: Barker> {
                owner: T,
                tags: Vec<String>,
                scores: HashMap<String, i32>,
                parent: Option<Box<Dog<T>>>,
            }

            fn make(x: i32) -> String { String::new() }
        "#);

        claim("every <type> has a <name> child",
            &mut tree, "//type[not(name)]", 0);

        claim("Dog declares <generic> with a `: Barker` bound",
            &mut tree, "//struct[name='Dog']/generic[name='T']/bounds/type[name='Barker']", 1);

        claim("Vec<String>: generic with inner <type>",
            &mut tree, "//field[name='tags']/type[generic][name='Vec']/type[name='String']", 1);

        claim("HashMap<String, i32>: generic with two inner <type> children",
            &mut tree, "//field[name='scores']/type[generic][name='HashMap']/type", 2);

        claim("Option<Box<Dog<T>>> nests 3 levels of <type[generic]>",
            &mut tree, "//field[name='parent']/type[generic]/type[generic]/type[generic]", 1);

        claim("parameter type wraps name in <name>",
            &mut tree, "//parameter/type[name='i32']", 1);

        claim("return type wraps name in <name>",
            &mut tree, "//returns/type[name='String']", 1);
    }

    #[test]
    fn typescript() {
        let mut tree = parse_src("typescript", r#"
            type Id = number;
            type Handler = (x: number) => void;
            type Box<T> = Array<T>;

            class Animal {}
            interface Barker { bark(): void; }
            class Dog extends Animal implements Barker {
                bark(): void {}
            }

            function f(x: number): string { return ""; }
        "#);

        claim("only <type[function]> may lack a <name> (it's defined by signature)",
            &mut tree, "//type[not(name) and not(function)]", 0);

        claim("plain alias points at a single <type>",
            &mut tree, "//alias[name='Id']/type[name='number']", 1);

        claim("function-type alias carries <type[function]>",
            &mut tree, "//alias[name='Handler']/type[function]", 1);

        claim("generic alias carries a <generic> child via <generics> wrapper",
            &mut tree, "//alias[name='Box']/generics/generic[name='T']", 1);

        claim("Dog extends and implements both wrap base types",
            &mut tree, "//class[name='Dog']/extends/type[name='Animal'] | //class[name='Dog']/implements/type[name='Barker']", 2);

        claim("function declaration's parameter type wraps name in <name>",
            &mut tree, "//function[name='f']/parameter/type[name='number']", 1);

        claim("function declaration's return type wraps name in <name>",
            &mut tree, "//function[name='f']/returns/type[name='string']", 1);

        claim("generic-alias type parameter has <name> child holding T (not nested type)",
            &mut tree, "//generic[name='T']", 1);

        claim("no spurious <type> wrapper inside the <name> of a generic",
            &mut tree, "//generic/name/type", 0);

        claim("no raw `function_type` kind leak (renamed to <type[function]>)",
            &mut tree, "//function_type", 0);
    }
}

mod conditionals {
    use super::*;

    /// Conditional shape: `else if` chains collapse to flat
    /// <else_if> siblings of <if>; ternary keeps <then>/<else>
    /// wrappers via surgical field-wrap in languages that have a
    /// dedicated <ternary> node. Python ternary is FLAT (no
    /// then/else wrappers); Ruby uses <conditional> rather than
    /// <ternary>.

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

        claim("no raw `elsif` element leaks",
            &mut tree, "//elsif", 0);

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
}
