//! Cross-language: binary / unary / compare / logical expression
//! shapes and the <op> element with embedded operator markers.
//!
//! Operator extraction is the convention shared by TS, Rust, Java,
//! C#, Python, Go, PHP: the source operator text moves into an <op>
//! child whose marker (<plus/>, <minus/>, <less/>, …) names the
//! operator semantically. Queries can match by marker without
//! reading text. Augmented-assignment operators (`+=` etc.) are
//! exercised in `variables.rs` because the surrounding shape there
//! is <assign>, not <binary>.

use crate::support::semantic::*;

// ---- binary ---------------------------------------------------------------

/// Binary expressions extract the operator into an <op> child whose
/// marker names the operator. The text token (`+`, `-`, …) stays on
/// <op> so source can be reconstructed.
#[test]
fn typescript_binary() {
    claim("TypeScript binary expression extracts + into <op[plus]>",
        &mut parse_src("typescript", "let x = 1 + 2;"),
        &multi_xpath(r#"
            //binary
                [left/expression/number='1']
                [op[plus]]
                [right/expression/number='2']
        "#),
        1);
}

#[test]
fn rust_binary() {
    claim("Rust binary expression extracts + into <op[plus]>",
        &mut parse_src("rust", "fn f() { let x = 1 + 2; }"),
        &multi_xpath(r#"
            //binary
                [left/expression/int='1']
                [op[plus]]
                [right/expression/int='2']
        "#),
        1);
}

#[test]
fn java_binary() {
    claim("Java binary expression extracts + into <op[plus]>",
        &mut parse_src("java", "class X { void f() { int y = 1 + 2; } }"),
        &multi_xpath(r#"
            //binary
                [left/expression/int='1']
                [op[plus]]
                [right/expression/int='2']
        "#),
        1);
}

#[test]
fn csharp_binary() {
    claim("C# binary expression extracts + into <op[plus]>",
        &mut parse_src("csharp", "class X { void f() { int y = 1 + 2; } }"),
        &multi_xpath(r#"
            //binary
                [left/expression/int='1']
                [op[plus]]
                [right/expression/int='2']
        "#),
        1);
}

#[test]
fn python_binary() {
    claim("Python binary expression extracts + into <op[plus]>",
        &mut parse_src("python", "z = x + y\n"),
        &multi_xpath(r#"
            //binary
                [left/expression/name='x']
                [op[plus]]
                [right/expression/name='y']
        "#),
        1);
}

#[test]
fn go_binary() {
    claim("Go binary expression extracts + into <op[plus]>",
        &mut parse_src("go", "package m\nvar x = 1 + 2\n"),
        &multi_xpath(r#"
            //binary
                [left/expression/int='1']
                [op[plus]]
                [right/expression/int='2']
        "#),
        1);
}

#[test]
fn php_binary() {
    claim("PHP binary expression extracts + into <op[plus]>",
        &mut parse_src("php", "<?php $z = $x + $y;"),
        &multi_xpath(r#"
            //binary
                [left/expression/variable/name='x']
                [op[plus]]
                [right/expression/variable/name='y']
        "#),
        1);
}

// ---- unary ----------------------------------------------------------------

/// Unary expressions follow the same <op> extraction pattern as
/// binary, with a single operand. The operator marker names which
/// unary operator is in play (<minus/>, <not/>, …).
#[test]
fn python_unary() {
    claim("Python unary - operator extracts into <op[minus]>",
        &mut parse_src("python", "x = -y\n"),
        &multi_xpath(r#"
            //unary
                [op[minus]]
                [name='y']
        "#),
        1);
}

/// C#'s `prefix_unary_expression` is a separate kind from
/// `unary_expression`, so it doesn't get the standard
/// `ExtractOpThenRename` treatment. Its prior shape
/// `<unary[prefix]>"-"<int>1</int></unary>` left the operator as bare
/// text, making `//unary[op[minus]]` and similar broad-narrow queries
/// silently miss every prefix unary site. The fix extracts the
/// operator AND keeps the `[prefix]` marker (needed to distinguish
/// `++x` from `x++` since both use `<op[increment]/>`).
#[test]
fn csharp_prefix_unary() {
    claim("`-1` extracts <op[minus]> and carries <prefix>",
        &mut parse_src("csharp", "int n = -1;"),
        "//unary[prefix][op[minus]]/int='1'",
        1);

    claim("`!true` extracts <op> with logical-not marker",
        &mut parse_src("csharp", "bool b = !true;"),
        "//unary[prefix][op/logical[not]]/bool='true'",
        1);

    claim("`~x` extracts <op> and carries <prefix>",
        &mut parse_src("csharp", "int x = 0; int p = ~x;"),
        "//unary[prefix]/op",
        1);

    claim("`++x` carries [prefix] AND op[increment] — distinguishable from x++ which lacks [prefix]",
        &mut parse_src("csharp", "int x = 0; ++x;"),
        "//unary[prefix][op[increment]]",
        1);
}

/// TypeScript / Java / PHP all collapse `++x` and `x++` under one
/// tree-sitter kind (`update_expression`), distinguished only by child
/// order. To match C#'s `<unary[prefix]>` shape, the per-language
/// transform detects prefix-form by inspecting child order BEFORE
/// operator extraction, and prepends a `<prefix/>` marker. After this
/// change `//unary[prefix][op[increment]]` matches `++x` in every
/// language that emits `<unary>`; postfix sites (`x++`) lack `[prefix]`
/// so the predicate cleanly distinguishes them.
#[test]
fn typescript_update_prefix_vs_postfix() {
    claim("`++x` extracts <op[increment]> AND carries <prefix>",
        &mut parse_src("typescript", "let x = 0; ++x;"),
        "//unary[prefix][op[increment]]",
        1);

    claim("`x++` extracts <op[increment]> WITHOUT <prefix>",
        &mut parse_src("typescript", "let x = 0; x++;"),
        "//unary[op[increment]][not(prefix)]",
        1);
}

#[test]
fn java_update_prefix_vs_postfix() {
    claim("`++i` extracts <op[increment]> AND carries <prefix>",
        &mut parse_src("java", "class T { void m() { int i = 0; ++i; } }"),
        "//unary[prefix][op[increment]]",
        1);

    claim("`i++` extracts <op[increment]> WITHOUT <prefix>",
        &mut parse_src("java", "class T { void m() { int i = 0; i++; } }"),
        "//unary[op[increment]][not(prefix)]",
        1);
}

/// Several common operators previously rendered as text-only `<op>`
/// (no semantic marker child). After adding entries to
/// `OPERATOR_MARKERS`, these now carry a marker so cross-language
/// queries like `//op[matmul]` or `//op[typeof]` work without parsing
/// text. The bare arithmetic forms (`//`, `@`) and the augmented-
/// assignment forms (`//=`, `@=`) follow the same nesting convention
/// as `+=` (`<op[assign[plus]]>`) — child marker under `<assign>`.
#[test]
fn python_floor_divide_and_matmul() {
    claim("Python `//=` extracts `<op[assign[floor-divide]]>`",
        &mut parse_src("python", "x = 4\nx //= 2\n"),
        "//assign[op[assign[floor-divide]]]",
        1);

    claim("Python `@=` extracts `<op[assign[matmul]]>`",
        &mut parse_src("python", "import numpy as np\nA = np.eye(2)\nA @= A\n"),
        "//assign[op[assign[matmul]]]",
        1);

    claim("Python `//` (bare floor-divide) extracts `<op[floor-divide]>`",
        &mut parse_src("python", "y = 7 // 2\n"),
        "//binary[op[floor-divide]]",
        1);
}

#[test]
fn typescript_typeof_and_void_unary() {
    claim("TypeScript `typeof x` extracts `<op[typeof]>`",
        &mut parse_src("typescript", "let s: string = typeof x;"),
        "//unary[op[typeof]]",
        1);

    claim("TypeScript `void 0` extracts `<op[void]>`",
        &mut parse_src("typescript", "let u = void 0;"),
        "//unary[op[void]]",
        1);
}

#[test]
fn ruby_defined_unary() {
    claim("Ruby `defined? x` extracts `<op[defined]>`",
        &mut parse_src("ruby", "x = 1\ndefined? x\n"),
        "//unary[op[defined]]",
        1);
}

#[test]
fn go_channel_receive_unary() {
    claim("Go `<-ch` extracts `<op[receive]>`",
        &mut parse_src("go", "package m\nfunc f(ch chan int) { v := <-ch; _ = v }"),
        "//unary[op[receive]][name='ch']",
        1);
}

#[test]
fn php_update_prefix_vs_postfix() {
    claim("`++$x` extracts <op[increment]> AND carries <prefix>",
        &mut parse_src("php", "<?php $x = 0; ++$x;"),
        "//unary[prefix][op[increment]]",
        1);

    claim("`$x++` extracts <op[increment]> WITHOUT <prefix>",
        &mut parse_src("php", "<?php $x = 0; $x++;"),
        "//unary[op[increment]][not(prefix)]",
        1);
}

/// C#'s null-forgiving operator (`name!`) is a postfix non-null
/// assertion — it doesn't change the value at runtime, just suppresses
/// a nullable warning. Per Principle #15 it surfaces as
/// `<expression>` host with `<non_null/>` marker (NOT as `<unary>` —
/// that bucket is reserved for `++`/`--` which are real unary
/// operators sharing tree-sitter's `postfix_unary_expression` kind).
/// The operator survives chained member access, method-call
/// arguments, and binary expressions.
#[test]
fn csharp_null_forgiving_postfix_unary() {
    let mut tree = parse_src("csharp", r#"
        class T {
            void M() {
                var simple = nullable!.Length;
                var chained = nullable!.ToUpper().Length;
                Process(nullable!);
                var combined = first!.Length + second!.Length;
            }
        }
    "#);

    claim("null-forgiving never produces an <ERROR> node",
        &mut tree, "//ERROR", 0);

    claim("each `name!` site renders as <expression[non_null]> — five total in this body",
        &mut tree, "//expression[non_null]", 5);

    claim("`name!` is NOT classified as a unary not-operator",
        &mut tree, "//unary[op[logical[not]]]", 0);

    claim("simple `name!.Length` exposes <member> with non-null host on the receiver",
        &mut tree,
        &multi_xpath(r#"
            //variable[declarator/name='simple']
                /declarator/member
                    [expression[non_null]/name='nullable']
                    [name='Length']
        "#),
        1);

    claim("chained `name!.A().B` keeps the non-null marker on the innermost receiver",
        &mut tree,
        &multi_xpath(r#"
            //variable[declarator/name='chained']
                /declarator/member
                    [call/member/expression[non_null]/name='nullable']
                    [name='Length']
        "#),
        1);

    claim("binary `first!.Length + second!.Length` carries non-null host under both operands",
        &mut tree,
        &multi_xpath(r#"
            //variable[declarator/name='combined']
                /declarator/binary
                    [op[plus]]
                    [left/expression/member[expression[non_null]/name='first'][name='Length']]
                    [right/expression/member[expression[non_null]/name='second'][name='Length']]
        "#),
        1);
}

// ---- compare --------------------------------------------------------------

/// Python comparisons render as <compare> (a distinct element from
/// <binary>) — they cover `<`, `<=`, `==`, `in`, `is`, … which
/// chain in Python's grammar. The <op> child holds a nested
/// <compare[KIND]/> marker so cross-comparison queries match on
/// the marker without parsing text.
#[test]
fn python_compare() {
    claim("Python relational comparison wraps the operator marker under <compare>",
        &mut parse_src("python", "y = x < 5\n"),
        &multi_xpath(r#"
            //compare
                [name='x']
                [op/compare[less]]
                [int='5']
        "#),
        1);
}

// ---- logical --------------------------------------------------------------

/// Python boolean operators render as <logical> (covering `and` /
/// `or` / `not`). The <op> wrapper holds a <logical[KIND]/>
/// marker, parallel to compare.
#[test]
fn python_logical() {
    claim("Python boolean and operator wraps the operator marker under <logical>",
        &mut parse_src("python", "z = a and b\n"),
        &multi_xpath(r#"
            //logical
                [left/expression/name='a']
                [op/logical[and]]
                [right/expression/name='b']
        "#),
        1);
}
