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

/// Cross-language: the `<binary>[op[plus]]` extraction contract is
/// the same xpath query across every language with a binary `+`.
/// Per-language tests above pin one language each (with the
/// language-specific literal kind on left/right inner expressions);
/// this loop pins the cross-language contract: `<binary>` exists,
/// has an `<op>` child carrying a `[plus]` marker, and the
/// operands sit in `<left>`/`<right>` slots wrapped in
/// `<expression>` hosts (Principle #15).
///
/// The xpath uses `[expression]` (without inner literal kind) so it
/// works uniformly across languages where the operand renders as
/// `<int>` (Java/C#/Rust/Go), `<number>` (TS/JS), `<name>` (Python
/// where `x + y` is the canonical form), or `<variable>` (PHP).
#[test]
fn cross_language_binary_plus_extracts_op_marker() {
    let canonical = r#"
        //binary
            [left/expression]
            [op[plus]]
            [right/expression]
    "#;

    for (lang, src) in &[
        ("typescript", "let z = 1 + 2;"),
        ("rust",       "fn f() { let z = 1 + 2; }"),
        ("java",       "class X { void f() { int z = 1 + 2; } }"),
        ("csharp",     "class X { void f() { int z = 1 + 2; } }"),
        ("go",         "package m\nvar z = 1 + 2"),
        ("python",     "z = x + y\n"),
        ("php",        "<?php $z = $x + $y;"),
    ] {
        claim(
            &format!("{lang}: binary `+` extracts <op[plus]> with expression-wrapped operands"),
            &mut parse_src(lang, src),
            &multi_xpath(canonical),
            1,
        );
    }
}

// ---- unary ----------------------------------------------------------------

/// Cross-language: the `<unary>[op[minus]]` extraction contract is
/// the same xpath query across every language with prefix `-x`.
/// Pinned uniformly to catch any future regression in unary
/// operator extraction. Sister test to
/// `cross_language_binary_plus_extracts_op_marker` (iter 334).
///
/// The xpath uses descendant-axis `//name='x'` for the operand
/// because PHP wraps `$x` in a `<variable>` element while other
/// languages have a bare `<name>` operand. Both produce a `<name>`
/// somewhere within the `<unary>`.
#[test]
fn cross_language_unary_minus_extracts_op_marker() {
    let canonical = "//unary[op[minus]]//name='x'";

    for (lang, src) in &[
        ("typescript", "let y = -x;"),
        ("rust",       "fn f() { let _y = -x; }"),
        ("java",       "class X { void f(int x) { int y = -x; } }"),
        ("csharp",     "class X { void F(int x) { int y = -x; } }"),
        ("go",         "package m\nfunc f(x int) int { return -x }"),
        ("python",     "y = -x\n"),
        ("php",        "<?php $y = -$x;"),
        ("ruby",       "y = -x\n"),
    ] {
        claim(
            &format!("{lang}: unary `-x` extracts <op[minus]> with <name>='x' operand somewhere inside"),
            &mut parse_src(lang, src),
            canonical,
            1,
        );
    }
}

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
    // After iter 52: `<prefix/>` marker is reserved for ++/--
    // (the only operators with a postfix counterpart). For
    // !x / -x / ~x there's no postfix form, so the marker would
    // be noise. C# now matches TS/Java/PHP's cross-language
    // convention.
    claim("`-1` extracts <op[minus]> on a plain <unary> (no [prefix] for !/-/~)",
        &mut parse_src("csharp", "int n = -1;"),
        "//unary[op[minus]][not(prefix)]/int='1'",
        1);

    claim("`!true` extracts <op> with logical-not marker (no [prefix] needed)",
        &mut parse_src("csharp", "bool b = !true;"),
        "//unary[op[logical and not]][not(prefix)]/bool='true'",
        1);

    claim("`~x` extracts <op> on plain <unary> (no [prefix] needed)",
        &mut parse_src("csharp", "int x = 0; int p = ~x;"),
        "//unary[op][not(prefix)]/name='x'",
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
    claim("Python `//=` carries flat `assign` + `floor` markers on `<op>`",
        &mut parse_src("python", "x = 4\nx //= 2\n"),
        "//assign[op[assign and floor]]",
        1);

    claim("Python `@=` carries flat `assign` + `matmul` markers on `<op>`",
        &mut parse_src("python", "import numpy as np\nA = np.eye(2)\nA @= A\n"),
        "//assign[op[assign and matmul]]",
        1);

    claim("Python `//` (bare floor-divide) extracts `<op[floor]>`",
        &mut parse_src("python", "y = 7 // 2\n"),
        "//binary[op[floor]]",
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

/// Go's `inc_statement` and `dec_statement` (`i++`, `i--`) are postfix-
/// only and were previously `Rename(Unary)` — so the operator was left
/// as bare text inside `<unary>`. Cross-language increment/decrement
/// queries (`//unary/op[increment]`) silently missed Go. Switching to
/// `ExtractOpThenRename(Unary)` aligns Go with the C-family postfix
/// shape established in iter 8.
#[test]
fn go_inc_dec_statement_extracts_op() {
    claim("Go `i++` extracts `<op[increment]>`",
        &mut parse_src("go", "package m\nfunc f() { i := 0; i++; _ = i }"),
        "//unary[op[increment]][name='i']",
        1);

    claim("Go `n--` extracts `<op[decrement]>`",
        &mut parse_src("go", "package m\nfunc f() { n := 0; n--; _ = n }"),
        "//unary[op[decrement]][name='n']",
        1);
}

/// Go `arr[i]` index expression — wraps the operand (the array)
/// in `<object>` to match member-access vocabulary and avoid
/// JSON name-key collision when both array and index are
/// identifiers (`seen[x]` would otherwise produce two
/// `<name>` siblings). Unary expressions are NOT touched (they
/// also use `field="operand"` but only have one operand, no
/// collision).
#[test]
fn go_index_expression_wraps_operand() {
    claim("Go index `seen[x]` wraps the array operand in <object>",
        &mut parse_src("go", "package m\nfunc f(seen []int, x int) { _ = seen[x] }"),
        "//index[object/name='seen'][name='x']",
        1);

    claim("Go unary `-x` is unchanged (still bare <name> operand, no <object> wrap)",
        &mut parse_src("go", "package m\nfunc f(x int) { _ = -x }"),
        "//unary[name='x'][not(object)]",
        1);
}

/// Go `s[i:j]` / `s[i:j:k]` / `s[:]` slice expression — wraps the
/// operand in `<object>` (matching `index_expression` iter 284) and
/// the bounds in `<from>` / `<to>` / `<capacity>` slots so two
/// `<int>` siblings don't collide on a singleton JSON key. The
/// `<slice/>` marker remains so `//index[slice]` still picks slice
/// ops out. Vocabulary mirrors Rust ranges (iter 270) and Ruby
/// ranges (iter 180) per Principle #5.
#[test]
fn go_slice_expression_wraps_bounds() {
    let mut tree = parse_src("go", r#"
        package m
        func f(s []int) []int {
            _ = s[1:3]
            _ = s[1:3:4]
            return s[:]
        }
    "#);

    claim("Go `s[1:3]` wraps operand in <object>, bounds in <from>/<to>",
        &mut tree,
        "//index[slice][object/name='s'][from/int='1'][to/int='3'][not(capacity)]",
        1);

    claim("Go `s[1:3:4]` adds <capacity> slot",
        &mut tree,
        "//index[slice][object/name='s'][from/int='1'][to/int='3'][capacity/int='4']",
        1);

    claim("Go `s[:]` slice has neither <from> nor <to>",
        &mut tree,
        "//index[slice][object/name='s'][not(from)][not(to)]",
        1);
}

/// PHP `$arr[$key]` subscript expression — wraps the operand in
/// `<object>` so the array variable doesn't collide with the
/// index variable on the JSON `variable` key. Mirrors Go iter 284.
#[test]
fn php_subscript_wraps_operand() {
    claim("PHP `$arr[$key]` wraps the array operand in <object>",
        &mut parse_src("php", "<?php $r = $arr[$key];"),
        "//index[object/variable/name='arr'][variable/name='key']",
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

    // Iter 245: C# chain inversion. Member access becomes
    // `<object[access]>` with the non-null expression as the
    // chain receiver (a direct child of the chain wrapper).
    claim("simple `name!.Length` puts <expression[non_null]> as the chain receiver",
        &mut tree,
        &multi_xpath(r#"
            //variable[name='simple']
                /object[access]
                    [expression[non_null]/name='nullable']
                    [member/name='Length']
        "#),
        1);

    claim("chained `name!.A().B` keeps the non-null marker on the innermost receiver",
        &mut tree,
        &multi_xpath(r#"
            //variable[name='chained']
                /object[access]
                    [expression[non_null]/name='nullable']
                    [.//call/name='ToUpper']
                    [.//member/name='Length']
        "#),
        1);

    claim("binary `first!.Length + second!.Length` carries non-null host under both operands",
        &mut tree,
        &multi_xpath(r#"
            //variable[name='combined']
                /binary
                    [op[plus]]
                    [left/expression/object[access][expression[non_null]/name='first'][member/name='Length']]
                    [right/expression/object[access][expression[non_null]/name='second'][member/name='Length']]
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
                [op[compare and less]]
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
                [op[logical and and]]
                [right/expression/name='b']
        "#),
        1);
}

// ---- range -----------------------------------------------------------------

/// Rust `0..10` (exclusive end), `0..=10` (inclusive end), open-
/// ended `5..` / `..3`, and full `..` all emit `<range>` with
/// `<from>` / `<to>` slot wrappers and an `<inclusive/>` /
/// `<exclusive/>` end-marker (Principle #8: source must be
/// reconstructable; `..` vs `..=` are semantically distinct).
/// Mirrors Ruby iter 180.
#[test]
fn rust_range_bounds_and_inclusivity() {
    let mut tree = parse_src("rust", r#"
        fn f() {
            let _ = (0..10, 1..=5, 5.., ..3, ..);
        }
    "#);

    claim("Rust `0..10` is range[exclusive] with from/to slots",
        &mut tree,
        "//range[exclusive][from/int='0'][to/int='10']",
        1);

    claim("Rust `1..=5` is range[inclusive] with from/to slots",
        &mut tree,
        "//range[inclusive][from/int='1'][to/int='5']",
        1);

    claim("Rust `5..` is range[exclusive] with from but no to",
        &mut tree,
        "//range[exclusive][from/int='5'][not(to)]",
        1);

    claim("Rust `..3` is range[exclusive] with to but no from",
        &mut tree,
        "//range[exclusive][to/int='3'][not(from)]",
        1);

    claim("Rust pattern range `0..=9` reuses left/right field-wrap, renamed to from/to",
        &mut parse_src("rust", "fn f(x: i32) { match x { 0..=9 => {}, _ => {} } }"),
        "//pattern/range[inclusive][from/int='0'][to/int='9']",
        1);
}
