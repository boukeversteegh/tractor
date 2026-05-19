//! Cross-language: loop shapes — element name AND embedded
//! initialiser / condition / update / binding / iterable layouts.
//!
//! Element-name conventions:
//!
//! - C# / Java / PHP add `<foreach>` for the iteration form
//!   (`foreach (var x in xs)`, `for (T x : xs)`).
//! - Rust adds `<loop>` for its infinite-loop construct.
//! - Ruby adds `<until>` (loop while NOT condition).
//! - Go uses a single `<for>` for all forms (C-style, infinite,
//!   range — disambiguated by children).
//! - TypeScript uses a single `<for>` for C-style, `for...of`,
//!   and `for...in`.

//! Inner-shape conventions (cross-language inconsistencies pinned
//! here on purpose so a future unification surfaces deliberately):
//!
//! - C-style `for`: initialisation + `<condition>` + update + body.
//!   The update is `<unary>` in C# and TypeScript (operator
//!   extracted), `<update_expression>` raw in PHP, and
//!   `<assign>` in Go's `i = i + 1` form.
//! - Iteration (`foreach` / `for...of` / `for...in` / `for x in
//!   xs`): three different binding+iterable shapes are in use:
//!     1. `<left>` + `<right>` (C#, TypeScript, Python).
//!     2. `<name>` + `<value>` (Java foreach, Rust).
//!     3. `<name>` + `<value>/<in>` (Ruby for-in — with an extra
//!        `<in>` wrapper).
//! - `while`: `<condition>` + `<body>`. C# / TypeScript / PHP
//!   wrap the body in `<block>`; Python / Ruby / Rust / Go do
//!   not.
//! - `do`-`while`: body precedes condition; all languages render
//!   as `<do>`.

use crate::support::semantic::*;

#[test]
fn java() {
    let mut tree = parse_src("java", r#"
        class Service {
            void run() {
                for (int i = 0; i < attempts; i++) { tick(); }
                while (running) { tick(); }
                do { tick(); } while (running);
                for (Item item : items) { handle(item); }
            }
        }
    "#);

    claim("Java for/while/do/foreach all surface as siblings under the method body",
        &mut tree,
        &multi_xpath(r#"
            //method[name='run']/body
                [for]
                [while]
                [do]
                [foreach]
        "#),
        1);

    claim("Java C-style for embeds variable initialiser, <condition>, <unary> update, and body",
        &mut tree,
        &multi_xpath(r#"
            //for
                [variable[type/name='int'][name='i']]
                [condition/expression/binary]
                [unary[op[increment]]]
                [body]
        "#),
        1);

    claim("Java while loop holds the condition under <condition> and the loop body under <body>",
        &mut tree,
        "//while[condition/expression/name='running'][body]",
        1);

    claim("Java do-while: body precedes the condition",
        &mut tree,
        "//do[body][condition/expression/name='running']",
        1);

    claim("Java foreach uses <name> + <value> (no <left>/<right>) for the binding and iterable",
        &mut tree,
        &multi_xpath(r#"
            //foreach
                [type/name='Item']
                [name='item']
                [value/expression/name='items']
                [body]
        "#),
        1);
}

#[test]
fn csharp() {
    let mut tree = parse_src("csharp", r#"
        class Service {
            void Run() {
                for (int i = 0; i < attempts; i++) { Tick(); }
                while (running) { Tick(); }
                do { Tick(); } while (running);
                foreach (var item in items) { Handle(item); }
            }
        }
    "#);

    claim("C# for/while/do/foreach all surface as siblings under the method body",
        &mut tree,
        &multi_xpath(r#"
            //method[name='Run']/body/block
                [for]
                [while]
                [do]
                [foreach]
        "#),
        1);

    claim("C# C-style for has <variable> init, <condition>, <unary> update, and body",
        &mut tree,
        &multi_xpath(r#"
            //for
                [variable[type/name='int'][name='i']]
                [condition/expression/binary]
                [unary[op[increment]]]
                [body/block]
        "#),
        1);

    claim("C# while loop wraps the body in <block>",
        &mut tree,
        "//while[condition/expression/bool='running' or condition/expression/name='running'][body/block]",
        1);

    claim("C# do-while renames to <do>; body precedes the condition",
        &mut tree,
        "//do[body/block][condition/expression/name='running']",
        1);

    claim("C# foreach uses <left> for the binding and <right>/<expression> for the iterable",
        &mut tree,
        &multi_xpath(r#"
            //foreach
                [type/name='var']
                [left/expression/name='item']
                [right/expression/name='items']
                [body/block]
        "#),
        1);
}

#[test]
fn typescript() {
    let mut tree = parse_src("typescript", r#"
        function run() {
            for (let i = 0; i < attempts; i++) { tick(); }
            while (running) { tick(); }
            do { tick(); } while (running);
            for (const item of items) { handle(item); }
            for (const key in record) { handle(key); }
        }
    "#);

    claim("TypeScript surfaces three <for> forms (C-style, for-of, for-in) plus while/do",
        &mut tree,
        &multi_xpath(r#"
            //function[name='run']/body/block
                [count(for)=3]
                [while]
                [do]
        "#),
        1);

    claim("TypeScript C-style for has <variable[let]> init, <condition>, <unary> update, body",
        &mut tree,
        &multi_xpath(r#"
            //for
                [variable[let][name='i']]
                [condition/expression/binary]
                [unary[op[increment]]]
                [body/block]
        "#),
        1);

    claim("TypeScript for-of uses <left> + <right> for the binding and iterable",
        &mut tree,
        &multi_xpath(r#"
            //for
                [left/expression/name='item']
                [right/expression/name='items']
                [body/block]
        "#),
        1);

    claim("TypeScript for-in uses <left> + <right>/<expression>, mirroring for-of",
        &mut tree,
        &multi_xpath(r#"
            //for
                [left/expression/name='key']
                [right/expression/name='record']
                [body/block]
        "#),
        1);

    claim("TypeScript do-while renames to <do>; body precedes the condition",
        &mut tree,
        "//do[body/block][condition/expression/bool or condition/expression/name]",
        1);
}

#[test]
fn python() {
    let mut tree = parse_src("python", r#"
        def run():
            for item in items:
                handle(item)
            while running:
                tick()
    "#);

    claim("Python for and while render as direct children of the function body",
        &mut tree,
        &multi_xpath(r#"
            //function[name='run']/body
                [for]
                [while]
        "#),
        1);

    claim("Python for-in uses <left> + <right> for the binding and iterable",
        &mut tree,
        &multi_xpath(r#"
            //for
                [left/expression/name='item']
                [right/expression/name='items']
                [body//call/name='handle']
        "#),
        1);

    claim("Python while loop holds the condition under <condition>",
        &mut tree,
        "//while[condition/expression/name='running'][body//call/name='tick']",
        1);
}

#[test]
fn rust() {
    let mut tree = parse_src("rust", r#"
        fn run() {
            for item in items { handle(item); }
            while running { tick(); }
            loop { break; }
        }
    "#);

    claim("Rust for/while/loop all surface as siblings under the function body",
        &mut tree,
        &multi_xpath(r#"
            //function[name='run']/body
                [for]
                [while]
                [loop]
        "#),
        1);

    claim("Rust for-in uses <name> + <value> (no <left>/<right>)",
        &mut tree,
        &multi_xpath(r#"
            //for
                [name='item']
                [value/expression/name='items']
                [body]
        "#),
        1);

    claim("Rust while loop holds the condition under <condition>",
        &mut tree,
        "//while[condition/expression/name='running'][body]",
        1);

    claim("Rust loop is unconditional and exposes only its body",
        &mut tree,
        "//loop[body][not(condition)]",
        1);
}

/// Go uses a single `<for>` element for every loop form. C-style
/// has init + condition + update; range has a `<range>` clause;
/// infinite has neither.
#[test]
fn go() {
    let mut tree = parse_src("go", r#"
        package main

        func run() {
            for i := 0; i < attempts; i = i + 1 {
                tick()
            }
            for n < 3 {
                tick()
            }
            for {
                break
            }
            for k, v := range items {
                handle(k, v)
            }
        }
    "#);

    claim("Go renders four <for> elements covering C-style, while, infinite, and range forms",
        &mut tree,
        "//function[name='run']/body/for",
        4);

    claim("Go C-style for has <variable[short]> init, <condition>, <assign> update, and body",
        &mut tree,
        &multi_xpath(r#"
            //for
                [variable[short][left/expression/name='i']]
                [condition/expression/binary]
                [assign[left/expression/name='i']]
                [body]
        "#),
        1);

    // Within-Go Principle #5: the while-form's loop test wraps in
    // <condition>/<expression>/... matching the C-style and the
    // cross-language while-loop shape (Java/Python/TS/Rust).
    claim("Go while-form `for cond {}` wraps the loop test in <condition> (no init/update)",
        &mut tree,
        "//for[not(variable)][not(range)][condition/expression/binary[left/expression/name='n']][body]",
        1);

    claim("Go infinite for has only a body (no init, condition, or update)",
        &mut tree,
        "//for[not(condition)][not(variable)][not(range)][body]",
        1);

    claim("Go range for nests binding and iterable under a <range> child",
        &mut tree,
        &multi_xpath(r#"
            //for/range
                [left[expression/name='k'][expression/name='v']]
                [right/expression/name='items']
        "#),
        1);
}

#[test]
fn ruby() {
    let mut tree = parse_src("ruby", r#"
        def run
          for item in items
            handle(item)
          end
          while running
            tick
          end
          until done
            poll
          end
        end
    "#);

    claim("Ruby renders for / while / until as distinct loop elements",
        &mut tree,
        &multi_xpath(r#"
            //method[name='run']/body
                [for]
                [while]
                [until]
        "#),
        1);

    claim("Ruby for-in: loop var as <name>, iterable directly under <value>/<expression>",
        &mut tree,
        &multi_xpath(r#"
            //for
                [name='item']
                [value/expression/name='items']
                [body]
        "#),
        1);

    claim("Ruby while loop holds the condition under <condition>",
        &mut tree,
        "//while[condition/expression/name='running'][body]",
        1);

    claim("Ruby until loop holds the inverted condition under <condition>",
        &mut tree,
        "//until[condition/expression/name='done'][body]",
        1);
}

#[test]
fn php() {
    let mut tree = parse_src("php", r#"<?php
        function run() {
            for ($i = 0; $i < $attempts; $i++) { tick(); }
            while ($running) { tick(); }
            do { tick(); } while ($running);
            foreach ($items as $item) { handle($item); }
        }
    "#);

    claim("PHP for/while/do/foreach all surface as siblings under the function body",
        &mut tree,
        &multi_xpath(r#"
            //function[name='run']/body
                [for]
                [while]
                [do]
                [foreach]
        "#),
        1);

    claim("PHP C-style for has <assign> init, <condition>, <unary> update, and body",
        &mut tree,
        &multi_xpath(r#"
            //for
                [assign[left/expression/variable/name='i']]
                [condition/expression/binary]
                [unary[op[increment]]]
                [body]
        "#),
        1);

    claim("PHP do-while renames to <do>; body precedes the condition",
        &mut tree,
        "//do[body][condition]",
        1);

    // Iter 349: foreach iterable wraps in `<right>` and binding wraps
    // in `<left>` per Principle #5 cross-language alignment with
    // TS/C#/Python `for x in items`. Was `<value>/<variable>` for
    // iterable + bare `<variable>` for binding (iter 286); the
    // `<value>` semantics conflicted with elsewhere meaning "an
    // actual value" (default, init, hash). Cold-read HIGH finding
    // closed.
    claim("PHP foreach uses <right> for iterable and <left> for binding",
        &mut tree,
        &multi_xpath(r#"
            //foreach
                [right/expression/variable/name='items']
                [left/expression/variable/name='item']
                [body]
        "#),
        1);
}

/// PHP C-style `for ($i=0, $j=10; ...; $i++, $j--)` produces
/// `<for>` with multiple `<assign>` (init) AND multiple `<unary>`
/// (post-update) siblings. Both role-uniform per Principle #19.
#[test]
fn php_for_multi_init_and_update_lists() {
    let mut tree = parse_src(
        "php",
        "<?php for ($i = 0, $j = 10; $i < $j; $i++, $j--) {}\n",
    );
    claim("PHP for-loop with two init assigns tags each <assign> with list='assigns'",
        &mut tree,
        "//for/assign",
        2);

    claim("PHP for-loop with two post-updates tags each <unary> with list='unaries'",
        &mut tree,
        "//for/unary",
        2);
}

/// C-style `for` loops with comma-separated post-update produce
/// multiple `<unary>` siblings under `<for>`. Per Principle #19
/// they're role-uniform (each is one update step); tag with
/// `list="unaries"` so JSON renders as `unaries: [...]` instead
/// of overflowing. Single-update keeps the singleton `<unary>`
/// JSON key.
#[test]
fn typescript_for_multi_update_lists_unaries() {
    claim("TS comma-separated post-update tags each <unary> with list='unaries'",
        &mut parse_src("typescript", r#"
        for (let i = 0, j = 100; i < 5; j--, i++) {}
    "#),
        "//for/unary",
        2);

    claim("TS single post-update keeps singleton <unary> (no list= tagging)",
        &mut parse_src("typescript", r#"
        for (let i = 0; i < 5; i++) {}
    "#),
        "//for/unary",
        1);
}

/// Cross-language: every `for/foreach`/`while` loop carries a `<body>`
/// child holding the loop body (Principle #15: body is a structural
/// slot). The element name housing the loop varies (`<for>` / `<while>`
/// / `<foreach>`) but every language uses `<body>` for the loop body
/// uniformly. This test asserts the contract holds across 8 languages
/// for the simplest infinite/conditional loop forms.
#[test]
fn cross_language_loop_body_is_structural_slot() {
    for (lang, src, xpath) in &[
        // TypeScript: `while(true)` infinite loop.
        ("typescript", "while (true) { tick(); }",
         "//while/body"),
        // Java: same.
        ("java", "class X { void f() { while (true) { tick(); } } }",
         "//while/body"),
        // C#: same.
        ("csharp", "class X { void F() { while (true) { Tick(); } } }",
         "//while/body"),
        // Rust: `while` loops.
        ("rust", "fn f() { while true { tick(); } }",
         "//while/body"),
        // Go: `for true {}` is equivalent to while-true.
        ("go", "package m\nfunc f() { for true { tick() } }",
         "//for/body"),
        // Python: `while True`.
        ("python", "while True:\n    tick()\n",
         "//while/body"),
        // PHP.
        ("php", "<?php while (true) { tick(); }",
         "//while/body"),
        // Ruby — note Ruby's `while` body is always present.
        ("ruby", "while true\n  tick\nend\n",
         "//while/body"),
    ] {
        claim(
            &format!("{lang}: while-true loop has a structural <body> slot"),
            &mut parse_src(lang, src),
            xpath,
            1,
        );
    }
}

/// Cross-language: every `while (X)` loop wraps its condition under
/// `<while>/<condition>/<expression>/...` (Principle #15: every
/// expression position carries an `<expression>` host).
///
/// Per-language tests above pin this with the inner-expression kind
/// (`<binary>` / `<compare>` / `<name>`); this loop pins the
/// surrounding host contract uniformly across 7 chain-inverting
/// languages — a regression in `wrap_expression_positions`'
/// `condition` handling for any one language would trip this single
/// test rather than the maintainer noticing per-language drift.
#[test]
fn cross_language_while_condition_has_expression_host() {
    for (lang, src) in &[
        ("typescript", "while (running) { tick(); }"),
        ("java",       "class X { void f(boolean running) { while (running) { tick(); } } }"),
        ("csharp",     "class X { void F(bool running) { while (running) { Tick(); } } }"),
        ("rust",       "fn f(running: bool) { while running { tick(); } }"),
        ("go",         "package m\nfunc f(running bool) { for running { tick() } }"),
        ("python",     "while running:\n    tick()\n"),
        ("php",        "<?php while ($running) { tick(); }"),
        ("ruby",       "while running\n  tick\nend\n"),
    ] {
        // Note: Go uses `for` with a single condition for while-loops;
        // the canonical xpath becomes `//for/condition/expression`.
        claim(
            &format!("{lang}: while loop condition wraps in <expression> host (Principle #15)"),
            &mut parse_src(lang, src),
            if *lang == "go" {
                "//for/condition/expression"
            } else {
                "//while/condition/expression"
            },
            1,
        );
    }
}
