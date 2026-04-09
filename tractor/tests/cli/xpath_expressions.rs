use crate::common::{integration_dir, tractor_test};

fn dir() -> std::path::PathBuf {
    integration_dir("xpath-expressions")
}

#[test]
fn let_expression() {
    tractor_test(
        &dir(),
        &[
            "-s", "let x = 1; let y = 2;", "-l", "typescript",
            "-x", "let $v := //variable return $v/name",
            "-v", "value", "--expect", "2",
        ],
    );
}

#[test]
fn for_expression() {
    tractor_test(
        &dir(),
        &[
            "-s", "let x = 1; let y = 2;", "-l", "typescript",
            "-x", "for $v in //name return string($v)",
            "-v", "value", "--expect", "2",
        ],
    );
}

#[test]
fn if_expression_true_branch() {
    tractor_test(
        &dir(),
        &[
            "-s", "let x = 1;", "-l", "typescript",
            "-x", "if (//variable) then //name else ()",
            "-v", "value", "--expect", "1",
        ],
    );
}

#[test]
fn if_expression_false_branch() {
    tractor_test(
        &dir(),
        &[
            "-s", "let x = 1;", "-l", "typescript",
            "-x", "if (//function) then //name else //variable",
            "-v", "value", "--expect", "1",
        ],
    );
}

#[test]
fn some_quantified_expression() {
    tractor_test(
        &dir(),
        &[
            "-s", "function add(a, b) { return a + b; }", "-l", "javascript",
            "-x", "some $f in //function satisfies $f/name = 'add'",
            "--expect", "1",
        ],
    );
}

#[test]
fn every_quantified_expression() {
    tractor_test(
        &dir(),
        &[
            "-s", "function add(a, b) { return a + b; }", "-l", "javascript",
            "-x", "every $f in //function satisfies $f/name = 'add'",
            "--expect", "1",
        ],
    );
}

#[test]
fn let_with_variable_reference() {
    tractor_test(
        &dir(),
        &[
            "-s", "let x = 1;", "-l", "typescript",
            "-x", "let $v := //name return $v",
            "-v", "value", "--expect", "1",
        ],
    );
}

#[test]
fn bare_element_name_auto_prefixed() {
    tractor_test(
        &dir(),
        &["-s", "let x = 1;", "-l", "typescript", "-x", "variable", "--expect", "1"],
    );
}

#[test]
fn bare_element_with_predicate_auto_prefixed() {
    tractor_test(
        &dir(),
        &[
            "-s", "function foo() {}", "-l", "javascript",
            "-x", "function[name='foo']",
            "--expect", "1",
        ],
    );
}
