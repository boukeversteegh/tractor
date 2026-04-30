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
//! - Java and TypeScript currently leak `do_statement` as a raw
//!   tree-sitter kind for `do { … } while (…)`. C#'s do-while
//!   does render as `<do>`. PHP renders it as `<do>`.
//!
//! Inner-shape conventions (cross-language inconsistencies pinned
//! here on purpose so a future unification surfaces deliberately):
//!
//! - C-style `for`: initialisation + `<condition>` + update + body.
//!   The update is `<unary>` in C# and TypeScript (operator
//!   extracted), `<update_expression>` raw in Java and PHP, and
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
//! - `do`-`while`: body precedes condition; C# uses `<do>`, Java
//!   and TypeScript leak `<do_statement>`.

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
                [do_statement]
                [foreach]
        "#),
        1);

    claim("Java C-style for embeds variable initialiser, <condition>, update_expression, and body",
        &mut tree,
        &multi_xpath(r#"
            //for
                [variable[type/name='int'][declarator/name='i']]
                [condition/binary]
                [update_expression]
                [body]
        "#),
        1);

    claim("Java while loop holds the condition under <condition> and the loop body under <body>",
        &mut tree,
        "//while[condition/name='running'][body]",
        1);

    claim("Java do-while leaks <do_statement>; body precedes the condition",
        &mut tree,
        "//do_statement[body][condition/name='running']",
        1);

    claim("Java foreach uses <name> + <value> (no <left>/<right>) for the binding and iterable",
        &mut tree,
        &multi_xpath(r#"
            //foreach
                [type/name='Item']
                [name='item']
                [value/name='items']
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
                [variable[type/name='int'][declarator/name='i']]
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

    claim("TypeScript surfaces three <for> forms (C-style, for-of, for-in) plus while",
        &mut tree,
        &multi_xpath(r#"
            //function[name='run']/body/block
                [count(for)=3]
                [while]
                [do_statement]
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

    claim("TypeScript do-while leaks <do_statement>; body precedes the condition",
        &mut tree,
        "//do_statement[body/block][condition/expression/bool or condition/expression/name]",
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
                [left/name='item']
                [right/name='items']
                [body//call/name='handle']
        "#),
        1);

    claim("Python while loop holds the condition under <condition>",
        &mut tree,
        "//while[condition/name='running'][body//call/name='tick']",
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
            for {
                break
            }
            for k, v := range items {
                handle(k, v)
            }
        }
    "#);

    claim("Go renders three <for> elements covering C-style, infinite, and range forms",
        &mut tree,
        "//function[name='run']/body/for",
        3);

    claim("Go C-style for has <variable[short]> init, <condition>, <assign> update, and body",
        &mut tree,
        &multi_xpath(r#"
            //for
                [variable[short][left/name='i']]
                [condition/binary]
                [assign[left/name='i']]
                [body]
        "#),
        1);

    claim("Go infinite for has only a body (no init, condition, or update)",
        &mut tree,
        "//for[not(condition)][not(variable)][not(range)][body]",
        1);

    claim("Go range for nests binding and iterable under a <range> child",
        &mut tree,
        &multi_xpath(r#"
            //for/range
                [left[name='k'][name='v']]
                [right/name='items']
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

    claim("Ruby for-in uses <name> + <value>/<in> (extra <in> wrapper unique to Ruby)",
        &mut tree,
        &multi_xpath(r#"
            //for
                [name='item']
                [value/in/name='items']
                [body]
        "#),
        1);

    claim("Ruby while loop holds the condition under <condition>",
        &mut tree,
        "//while[condition/name='running'][body]",
        1);

    claim("Ruby until loop holds the inverted condition under <condition>",
        &mut tree,
        "//until[condition/name='done'][body]",
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

    claim("PHP C-style for has <assign> init, <condition>, <update_expression>, and body",
        &mut tree,
        &multi_xpath(r#"
            //for
                [assign[left/variable/name='i']]
                [condition/binary]
                [update_expression]
                [body]
        "#),
        1);

    claim("PHP do-while renames to <do>; body precedes the condition",
        &mut tree,
        "//do[body][condition]",
        1);

    claim("PHP foreach lists the iterable then the binding (no <left>/<right>, no <name>/<value> wrappers)",
        &mut tree,
        &multi_xpath(r#"
            //foreach
                [variable/name='items']
                [variable/name='item']
                [body]
        "#),
        1);
}
