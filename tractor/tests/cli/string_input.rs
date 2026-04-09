use crate::common::{repo_root, tractor_fails};

tractor_tests!(string_flag_languages, repo_root(),
    ["--string", "fn add(a: i32, b: i32) -> i32 { a + b }", "-l", "rust", "-x", "function", "--expect", "1"],
    ["--string", "def hello(): pass", "-l", "python", "-x", "function", "--expect", "1"],
    ["--string", "public class Foo { public void Bar() {} }", "-l", "csharp", "-x", "class", "--expect", "1"],
    ["--string", "function greet() { return 'hi'; }", "-l", "javascript", "-x", "function", "--expect", "1"],
    ["--string", "const greet = (): string => 'hi';", "-l", "typescript", "-x", "lambda", "--expect", "1"],
    ["-s", "fn main() {}", "-l", "rust", "-x", "function", "--expect", "1"],
);

tractor_tests!(string_expect_variants, repo_root(),
    ["-s", "fn a() {} fn b() {}", "-l", "rust", "-x", "function", "--expect", "2"],
    ["-s", "fn a() {} fn b() {}", "-l", "rust", "-x", "function", "--expect", "some"],
    ["-s", "let x = 1;", "-l", "rust", "-x", "function", "--expect", "none"],
);

tractor_tests!(string_output_formats, repo_root(),
    ["-s", "class Foo { }", "-l", "csharp", "-x", "class/name", "-v", "value", "--expect", "1"],
    ["-s", "class Foo { }", "-l", "csharp", "-x", "class", "-v", "count", "--expect", "1"],
    ["-s", "class Foo { }", "-l", "csharp", "-x", "class", "-f", "gcc", "--expect", "1"],
    ["-s", "let x = 1;", "-l", "rust", "-v", "count", "--expect", "1"],
);

#[test]
fn string_without_lang_fails() {
    tractor_fails(&repo_root(), &["--string", "let x = 1;"]);
}
