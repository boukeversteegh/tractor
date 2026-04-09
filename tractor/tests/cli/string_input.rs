use crate::common::{repo_root, tractor_test, tractor_fails};

fn dir() -> std::path::PathBuf {
    repo_root()
}

#[test]
fn string_flag_languages() {
    let d = dir();
    tractor_test(&d, &["--string", "fn add(a: i32, b: i32) -> i32 { a + b }", "-l", "rust", "-x", "function", "--expect", "1"]);
    tractor_test(&d, &["--string", "def hello(): pass", "-l", "python", "-x", "function", "--expect", "1"]);
    tractor_test(&d, &["--string", "public class Foo { public void Bar() {} }", "-l", "csharp", "-x", "class", "--expect", "1"]);
    tractor_test(&d, &["--string", "function greet() { return 'hi'; }", "-l", "javascript", "-x", "function", "--expect", "1"]);
    tractor_test(&d, &["--string", "const greet = (): string => 'hi';", "-l", "typescript", "-x", "lambda", "--expect", "1"]);
    // short flag -s
    tractor_test(&d, &["-s", "fn main() {}", "-l", "rust", "-x", "function", "--expect", "1"]);
}

#[test]
fn string_expect_variants() {
    let d = dir();
    tractor_test(&d, &["-s", "fn a() {} fn b() {}", "-l", "rust", "-x", "function", "--expect", "2"]);
    tractor_test(&d, &["-s", "fn a() {} fn b() {}", "-l", "rust", "-x", "function", "--expect", "some"]);
    tractor_test(&d, &["-s", "let x = 1;", "-l", "rust", "-x", "function", "--expect", "none"]);
}

#[test]
fn string_output_formats() {
    let d = dir();
    tractor_test(&d, &["-s", "class Foo { }", "-l", "csharp", "-x", "class/name", "-v", "value", "--expect", "1"]);
    tractor_test(&d, &["-s", "class Foo { }", "-l", "csharp", "-x", "class", "-v", "count", "--expect", "1"]);
    tractor_test(&d, &["-s", "class Foo { }", "-l", "csharp", "-x", "class", "-f", "gcc", "--expect", "1"]);
    // without xpath (full AST output)
    tractor_test(&d, &["-s", "let x = 1;", "-l", "rust", "-v", "count", "--expect", "1"]);
}

#[test]
fn string_without_lang_fails() {
    tractor_fails(&dir(), &["--string", "let x = 1;"]);
}
