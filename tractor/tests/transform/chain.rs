//! Cross-language: member-access / method-call chain inversion
//! shape (`<object[access]>`).
//!
//! After iter 248 (chain inversion rolled out across all 8
//! programming languages), every right-deep `<member>`/`<call>`
//! tree gets rewritten into the inverted `<object[access]>`
//! shape — receiver as the first child, each access/call step
//! nested as the LAST child of the previous step. The full
//! design lives in `docs/design-chain-inversion.md`.
//!
//! This file is the cross-language contract: the same chain
//! expression should produce a structurally-identical tree
//! across every language, allowing one xpath per query goal to
//! work for all of them.
//!
//! ## Canonical chain
//!
//! All tests use `obj.foo().bar.baz()` (or its language-specific
//! equivalent) — receiver, call, property access, terminal call.
//! Four segments cover all the shape concerns:
//! - first child is the receiver
//! - call as a non-terminal step (mid-chain call)
//! - member as a non-terminal step (property access)
//! - call as the terminal step
//!
//! ## Per-language quirks pinned here
//!
//! - **Ruby**: parses every `.foo` as a method call, so even
//!   bare property access (`obj.foo.bar`) emits `<call>` on
//!   every step rather than `<member>`. The canonical shape is
//!   `//object[access]/call/call/call` for Ruby; `//object[access]/call/member/call`
//!   for the other 7 languages.
//! - **PHP**: the receiver is wrapped in `<variable>` (because
//!   PHP variables carry the `$` sigil), so the receiver query
//!   is `//object[access]/variable/name='obj'` rather than
//!   `//object[access]/name='obj'`.
//!
//! (Iter 255: the redundant `[instance]` marker that C# and PHP
//! previously added to every chain step was removed. The chain-
//! root `[access]` marker already says "this is access," so the
//! per-step marker carried no information.)
//! - **Java**: method calls go through a synthetic `<member>`
//!   wrapper in the canonical input (Java's flat-call shape was
//!   normalised in iter 244), so the inverted shape is identical
//!   to the others.
//!
//! All other 5 languages (Python, Go, TypeScript, Rust) match a
//! single canonical shape:
//!     `//object[access]/<step>/<step>/.../<step>`
//! where each `<step>` is `<member>` for property access and
//! `<call>` for invocation, with the receiver as the first
//! direct child of `<object[access]>`.

use crate::support::semantic::*;

#[test]
fn python() {
    let mut tree = parse_src("python", "obj.foo().bar.baz()\n");

    claim("Python chain `obj.foo().bar.baz()` inverts to <object[access]>",
        &mut tree, "//object[access]", 1);

    claim("Python chain receiver is `obj` (first child of the chain wrapper)",
        &mut tree, "//object[access]/name='obj'", 1);

    claim("Python full chain reads as a path: receiver / call / member / call",
        &mut tree,
        "//object[access][name='obj']/call[name='foo']/member[name='bar']/call[name='baz']",
        1);

    claim("Python chain depth is 3 nested step-elements (Law-of-Demeter probe)",
        &mut tree,
        "//object[access][count(.//*[self::member or self::call or self::subscript]) >= 3]",
        1);
}

#[test]
fn typescript_meta_property_chain() {
    // `import.meta.url` and `new.target.name` — JS meta-properties
    // are single atomic compound identifiers. Pre-iter 283 they
    // renamed to `<member>` and collided with the chain step
    // `<member>` (both ended up as `<member>` siblings under
    // `<object[access]>`, JSON overflowed). Iter 283 renamed
    // meta_property to `<name>` so the receiver slot is bare-name
    // (matching Python's `__file__` precedent).
    claim("TS `import.meta.url` chain has <name>import.meta</name> receiver",
        &mut parse_src("typescript", "const u = import.meta.url;\n"),
        "//object[access][name='import.meta']/member[name='url']",
        1);

    claim("TS meta-property `new.target.name` chain similarly uses <name> receiver",
        &mut parse_src("typescript", "const t = new.target.name;\n"),
        "//object[access][name='new.target']/member[name='name']",
        1);
}

#[test]
fn typescript() {
    let mut tree = parse_src("typescript", "obj.foo().bar.baz();\n");

    claim("TypeScript chain `obj.foo().bar.baz()` inverts to <object[access]>",
        &mut tree, "//object[access]", 1);

    claim("TypeScript chain receiver is `obj`",
        &mut tree, "//object[access]/name='obj'", 1);

    claim("TypeScript full chain reads as a path: receiver / call / member / call",
        &mut tree,
        "//object[access][name='obj']/call[name='foo']/member[name='bar']/call[name='baz']",
        1);
}

#[test]
fn java() {
    let mut tree = parse_src("java", r#"
        class X {
            void m() {
                obj.foo().bar.baz();
            }
        }
    "#);

    claim("Java chain `obj.foo().bar.baz()` inverts to <object[access]>",
        &mut tree, "//object[access]", 1);

    claim("Java chain receiver is `obj`",
        &mut tree, "//object[access]/name='obj'", 1);

    claim("Java full chain reads as a path: receiver / call / member / call",
        &mut tree,
        "//object[access][name='obj']/call[name='foo']/member[name='bar']/call[name='baz']",
        1);
}

#[test]
fn csharp() {
    let mut tree = parse_src("csharp", r#"
        class X {
            void M() {
                obj.foo().bar.baz();
            }
        }
    "#);

    claim("C# chain `obj.foo().bar.baz()` inverts to <object[access]>",
        &mut tree, "//object[access]", 1);

    claim("C# chain receiver is `obj`",
        &mut tree, "//object[access]/name='obj'", 1);

    claim("C# full chain reads as a path: receiver / call / member / call",
        &mut tree,
        "//object[access][name='obj']/call[name='foo']/member[name='bar']/call[name='baz']",
        1);
}

#[test]
fn go() {
    let mut tree = parse_src("go", r#"
        package m
        func main() { obj.foo().bar.baz() }
    "#);

    claim("Go chain `obj.foo().bar.baz()` inverts to <object[access]>",
        &mut tree, "//object[access]", 1);

    claim("Go chain receiver is `obj`",
        &mut tree, "//object[access]/name='obj'", 1);

    claim("Go full chain reads as a path: receiver / call / member / call",
        &mut tree,
        "//object[access][name='obj']/call[name='foo']/member[name='bar']/call[name='baz']",
        1);
}

#[test]
fn rust() {
    let mut tree = parse_src("rust", r#"
        fn main() {
            obj.foo().bar.baz();
        }
    "#);

    claim("Rust chain `obj.foo().bar.baz()` inverts to <object[access]>",
        &mut tree, "//object[access]", 1);

    claim("Rust chain receiver is `obj`",
        &mut tree, "//object[access]/name='obj'", 1);

    claim("Rust full chain reads as a path: receiver / call / member / call",
        &mut tree,
        "//object[access][name='obj']/call[name='foo']/member[name='bar']/call[name='baz']",
        1);
}

#[test]
fn ruby() {
    // Ruby parses every `.foo` as a method call, so the
    // intermediate property accesses also become <call>. Source
    // omits the trailing `()` (Ruby's convention) and leaves
    // `obj.foo.bar.baz` as four method calls.
    let mut tree = parse_src("ruby", "obj.foo.bar.baz\n");

    claim("Ruby chain `obj.foo.bar.baz` inverts to <object[access]>",
        &mut tree, "//object[access]", 1);

    claim("Ruby chain receiver is `obj`",
        &mut tree, "//object[access]/name='obj'", 1);

    claim("Ruby every step is a <call> (no <member>; Ruby treats . as method call)",
        &mut tree,
        "//object[access][name='obj']/call[name='foo']/call[name='bar']/call[name='baz']",
        1);

    claim("Ruby chain has zero <member> steps (Ruby invariant)",
        &mut tree, "//object[access]//member", 0);
}

#[test]
fn php() {
    let mut tree = parse_src("php", "<?php $obj->foo()->bar->baz();\n");

    claim("PHP chain `$obj->foo()->bar->baz()` inverts to <object[access]>",
        &mut tree, "//object[access]", 1);

    claim("PHP chain receiver is `$obj` (a <variable> with name='obj')",
        &mut tree, "//object[access]/variable/name='obj'", 1);

    claim("PHP full chain reads as a path: receiver / call / member / call",
        &mut tree,
        "//object[access][variable/name='obj']/call[name='foo']/member[name='bar']/call[name='baz']",
        1);
}

// ---- Cross-language convergence -----------------------------------------

/// All 7 of the languages where `.X` (without parens) is a
/// property access (everything except Ruby) produce the same
/// step-element sequence for `obj.foo().bar.baz()`. The xpath
/// here is the SAME across all 7 — proof of cross-language query
/// uniformity (Goal #1: Intuitive Queries).
#[test]
fn cross_language_uniformity() {
    let canonical_xpath = "//object[access]/call[name='foo']/member[name='bar']/call[name='baz']";

    for (lang, src) in &[
        ("python",     "obj.foo().bar.baz()\n"),
        ("typescript", "obj.foo().bar.baz();\n"),
        ("java",       "class X { void m() { obj.foo().bar.baz(); } }"),
        ("go",         "package m\nfunc main() { obj.foo().bar.baz() }"),
        ("rust",       "fn main() { obj.foo().bar.baz(); }"),
    ] {
        claim(
            &format!("{lang}: same canonical xpath finds the chain", lang=lang),
            &mut parse_src(lang, src),
            canonical_xpath,
            1,
        );
    }

    // C# and PHP previously added redundant `[instance]` markers
    // (dropped iter 255). The canonical xpath now matches them
    // identically to the others — chain steps are bare across all
    // 7 non-Ruby languages.
    claim(
        "csharp: canonical xpath matches (no per-step markers)",
        &mut parse_src("csharp", "class X { void M() { obj.foo().bar.baz(); } }"),
        canonical_xpath,
        1,
    );
    claim(
        "php: canonical xpath matches when receiver is wrapped in <variable>",
        &mut parse_src("php", "<?php $obj->foo()->bar->baz();\n"),
        canonical_xpath,
        1,
    );

    // Ruby is the deliberate exception — every step is <call>.
    // Both claims spell out the literal xpath inline (rather
    // than reusing `canonical_xpath`) so the let binding's uses
    // stay adjacent to its definition.
    claim(
        "ruby: canonical xpath does NOT match (Ruby treats . as method call)",
        &mut parse_src("ruby", "obj.foo.bar.baz\n"),
        "//object[access]/call[name='foo']/member[name='bar']/call[name='baz']",
        0,
    );
    claim(
        "ruby: all-call xpath matches the Ruby chain shape",
        &mut parse_src("ruby", "obj.foo.bar.baz\n"),
        "//object[access]/call[name='foo']/call[name='bar']/call[name='baz']",
        1,
    );
}

// ---- Subscript step ------------------------------------------------------

/// Subscript chains (`arr[0].field`) emit a `<subscript>` step
/// alongside `<member>` and `<call>`. The walker handles two
/// input flavours (slot-wrapped TS / Python `<index>` and bare-
/// children Go/Java/Rust) — both produce the same canonical
/// `<subscript>` step in the inverted output.
#[test]
fn subscript_typescript() {
    claim("TS subscript chain produces <subscript> step inside chain",
        &mut parse_src("typescript", "arr[0].field;\n"),
        "//object[access][name='arr']/subscript[number='0']/member[name='field']",
        1);
}

#[test]
fn subscript_python() {
    claim("Python subscript chain produces <subscript> step inside chain",
        &mut parse_src("python", "arr[0].field\n"),
        "//object[access][name='arr']/subscript[int='0']/member[name='field']",
        1);
}

#[test]
fn subscript_rust() {
    claim("Rust subscript chain produces <subscript> step inside chain",
        &mut parse_src("rust", "fn main() { let _ = arr[0].field; }"),
        "//object[access][name='arr']/subscript[int='0']/member[name='field']",
        1);
}

// ---- Implicit-receiver chains (C# base./this.) -------------------------

/// C# tree-sitter inlines `base` and `this` as a literal text leak
/// (`"base."` / `"this."`) rather than emitting a structural
/// `base_expression` / `this_expression` element. The C#
/// `member_access_expression` handler synthesises an `<object>`
/// slot containing a `<base/>` / `<this/>` empty element so the
/// chain inverter sees a proper receiver. After inversion the
/// `<base/>` / `<this/>` empty element rides as a marker on the
/// chain root, queryable via `//object[base]` /
/// `//object[this]`.
#[test]
fn csharp_base_member_access() {
    let mut tree = parse_src("csharp", r#"
        class X : Y {
            public override int Priority {
                get => base.Priority;
                set => base.Priority = value;
            }
        }
    "#);

    claim("C# `base.Priority` inverts to <object[access and base]>",
        &mut tree,
        "//object[access and base]/member[name='Priority']",
        2);

    claim("C# `base.X` is queryable via //object[base]",
        &mut tree, "//object[base]", 2);

    claim("C# `base.X` chains share the chain-root marker //object[access]",
        &mut tree, "//object[access]", 2);
}

#[test]
fn csharp_this_member_access() {
    let mut tree = parse_src("csharp", r#"
        class X {
            void M() { var y = this.Foo; }
        }
    "#);

    claim("C# `this.Foo` inverts to <object[access and this]>",
        &mut tree,
        "//object[access and this]/member[name='Foo']",
        1);

    claim("C# `this.X` is queryable via //object[this]",
        &mut tree, "//object[this]", 1);
}

// ---- Negative space: bare identifiers and top-level calls -----------------

/// Per the inverter's "useful-chain guard", bare identifiers and
/// top-level function calls (no receiver chain) are NOT wrapped
/// in `<object[access]>`. This is by design — wrapping them would
/// add noise without informational value.
#[test]
fn no_wrap_for_bare_identifier_or_top_level_call() {
    // Bare identifier: `x` — no chain, no wrapper.
    claim("Python bare identifier produces no <object[access]>",
        &mut parse_src("python", "x\n"),
        "//object[access]", 0);
    claim("TypeScript bare identifier produces no <object[access]>",
        &mut parse_src("typescript", "x;\n"),
        "//object[access]", 0);

    // Top-level call: `f(x)` — no receiver chain, no wrapper.
    claim("Python top-level call produces no <object[access]>",
        &mut parse_src("python", "f(x)\n"),
        "//object[access]", 0);
    claim("TypeScript top-level call produces no <object[access]>",
        &mut parse_src("typescript", "f(x);\n"),
        "//object[access]", 0);
}
