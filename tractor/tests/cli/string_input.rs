use crate::common::{integration_dir, repo_root, tractor_test, tractor_fails};

fn dir() -> std::path::PathBuf {
    integration_dir("string-input")
}

// ---------------------------------------------------------------------------
// Basic --string usage with different languages
// ---------------------------------------------------------------------------

#[test]
fn string_rust() {
    tractor_test(
        &dir(),
        &["--string", "fn add(a: i32, b: i32) -> i32 { a + b }", "-l", "rust", "-x", "function", "--expect", "1"],
    );
}

#[test]
fn string_python() {
    tractor_test(
        &dir(),
        &["--string", "def hello(): pass", "-l", "python", "-x", "function", "--expect", "1"],
    );
}

#[test]
fn string_csharp() {
    tractor_test(
        &dir(),
        &["--string", "public class Foo { public void Bar() {} }", "-l", "csharp", "-x", "class", "--expect", "1"],
    );
}

#[test]
fn string_javascript() {
    tractor_test(
        &dir(),
        &["--string", "function greet() { return 'hi'; }", "-l", "javascript", "-x", "function", "--expect", "1"],
    );
}

#[test]
fn string_typescript() {
    tractor_test(
        &dir(),
        &["--string", "const greet = (): string => 'hi';", "-l", "typescript", "-x", "lambda", "--expect", "1"],
    );
}

// ---------------------------------------------------------------------------
// Short flag -s
// ---------------------------------------------------------------------------

#[test]
fn short_flag_s() {
    tractor_test(
        &dir(),
        &["-s", "fn main() {}", "-l", "rust", "-x", "function", "--expect", "1"],
    );
}

// ---------------------------------------------------------------------------
// --expect integration
// ---------------------------------------------------------------------------

#[test]
fn expect_exact_count() {
    tractor_test(
        &dir(),
        &["-s", "fn a() {} fn b() {}", "-l", "rust", "-x", "function", "--expect", "2"],
    );
}

#[test]
fn expect_some() {
    tractor_test(
        &dir(),
        &["-s", "fn a() {} fn b() {}", "-l", "rust", "-x", "function", "--expect", "some"],
    );
}

#[test]
fn expect_none() {
    tractor_test(
        &dir(),
        &["-s", "let x = 1;", "-l", "rust", "-x", "function", "--expect", "none"],
    );
}

// ---------------------------------------------------------------------------
// Output formats
// ---------------------------------------------------------------------------

#[test]
fn output_value() {
    tractor_test(
        &dir(),
        &["-s", "class Foo { }", "-l", "csharp", "-x", "class/name", "-v", "value", "--expect", "1"],
    );
}

#[test]
fn output_count() {
    tractor_test(
        &dir(),
        &["-s", "class Foo { }", "-l", "csharp", "-x", "class", "-v", "count", "--expect", "1"],
    );
}

#[test]
fn output_gcc() {
    tractor_test(
        &dir(),
        &["-s", "class Foo { }", "-l", "csharp", "-x", "class", "-f", "gcc", "--expect", "1"],
    );
}

// ---------------------------------------------------------------------------
// Without xpath (full AST output)
// ---------------------------------------------------------------------------

#[test]
fn string_without_xpath() {
    tractor_test(
        &dir(),
        &["-s", "let x = 1;", "-l", "rust", "-v", "count", "--expect", "1"],
    );
}

// ---------------------------------------------------------------------------
// Error: --string without --lang should fail
// ---------------------------------------------------------------------------

#[test]
fn string_without_lang_fails() {
    tractor_fails(&repo_root(), &["--string", "let x = 1;"]);
}
