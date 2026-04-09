use crate::common::{repo_root, tractor_test};

fn dir() -> std::path::PathBuf {
    repo_root()
}

#[test]
fn xpath_31_expression_forms() {
    let d = dir();
    // let expressions
    tractor_test(&d, &["-s", "let x = 1; let y = 2;", "-l", "typescript", "-x", "let $v := //variable return $v/name", "-v", "value", "--expect", "2"]);
    // for expressions
    tractor_test(&d, &["-s", "let x = 1; let y = 2;", "-l", "typescript", "-x", "for $v in //name return string($v)", "-v", "value", "--expect", "2"]);
    // if expressions
    tractor_test(&d, &["-s", "let x = 1;", "-l", "typescript", "-x", "if (//variable) then //name else ()", "-v", "value", "--expect", "1"]);
    tractor_test(&d, &["-s", "let x = 1;", "-l", "typescript", "-x", "if (//function) then //name else //variable", "-v", "value", "--expect", "1"]);
    // quantified expressions (some/every return boolean atomics)
    tractor_test(&d, &["-s", "function add(a, b) { return a + b; }", "-l", "javascript", "-x", "some $f in //function satisfies $f/name = 'add'", "--expect", "1"]);
    tractor_test(&d, &["-s", "function add(a, b) { return a + b; }", "-l", "javascript", "-x", "every $f in //function satisfies $f/name = 'add'", "--expect", "1"]);
    // variable references (standalone)
    tractor_test(&d, &["-s", "let x = 1;", "-l", "typescript", "-x", "let $v := //name return $v", "-v", "value", "--expect", "1"]);
}

#[test]
fn bare_element_auto_prefix() {
    let d = dir();
    tractor_test(&d, &["-s", "let x = 1;", "-l", "typescript", "-x", "variable", "--expect", "1"]);
    tractor_test(&d, &["-s", "function foo() {}", "-l", "javascript", "-x", "function[name='foo']", "--expect", "1"]);
}
