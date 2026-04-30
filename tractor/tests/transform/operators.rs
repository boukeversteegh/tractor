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
                [left/number='1']
                [op[plus]]
                [right/number='2']
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
                [left/int='1']
                [op[plus]]
                [right/int='2']
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
                [left/name='x']
                [op[plus]]
                [right/name='y']
        "#),
        1);
}

#[test]
fn go_binary() {
    claim("Go binary expression extracts + into <op[plus]>",
        &mut parse_src("go", "package m\nvar x = 1 + 2\n"),
        &multi_xpath(r#"
            //binary
                [left/int='1']
                [op[plus]]
                [right/int='2']
        "#),
        1);
}

#[test]
fn php_binary() {
    claim("PHP binary expression extracts + into <op[plus]>",
        &mut parse_src("php", "<?php $z = $x + $y;"),
        &multi_xpath(r#"
            //binary
                [left/variable/name='x']
                [op[plus]]
                [right/variable/name='y']
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

/// C#'s null-forgiving operator (`name!`) is a postfix unary that
/// asserts a nullable expression is non-null at runtime. It MUST
/// parse without an `<ERROR>` node and surface as `<unary>` so
/// queries can find every site (e.g. for a "no null-forgiving"
/// rule). The operator survives chained member access, method-call
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

    claim("each `name!` site renders as <unary> — five total in this body",
        &mut tree, "//unary", 5);

    claim("simple `name!.Length` exposes <member[unary]>",
        &mut tree,
        &multi_xpath(r#"
            //variable[declarator/name='simple']
                /declarator/member
                    [unary/name='nullable']
                    [name='Length']
        "#),
        1);

    claim("chained `name!.A().B` keeps <unary> on the innermost member access",
        &mut tree,
        &multi_xpath(r#"
            //variable[declarator/name='chained']
                /declarator/member
                    [call/member/unary/name='nullable']
                    [name='Length']
        "#),
        1);

    claim("binary `first!.Length + second!.Length` carries <unary> under both operands",
        &mut tree,
        &multi_xpath(r#"
            //variable[declarator/name='combined']
                /declarator/binary
                    [op[plus]]
                    [left/expression/member[unary/name='first'][name='Length']]
                    [right/expression/member[unary/name='second'][name='Length']]
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
                [left/name='a']
                [op/logical[and]]
                [right/name='b']
        "#),
        1);
}
