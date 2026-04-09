use crate::common::repo_root;

tractor_tests!(xpath_31_expressions, repo_root(),
    ["-s", "let x = 1; let y = 2;", "-l", "typescript", "-x", "let $v := //variable return $v/name", "-v", "value", "--expect", "2"],
    ["-s", "let x = 1; let y = 2;", "-l", "typescript", "-x", "for $v in //name return string($v)", "-v", "value", "--expect", "2"],
    ["-s", "let x = 1;", "-l", "typescript", "-x", "if (//variable) then //name else ()", "-v", "value", "--expect", "1"],
    ["-s", "let x = 1;", "-l", "typescript", "-x", "if (//function) then //name else //variable", "-v", "value", "--expect", "1"],
    ["-s", "function add(a, b) { return a + b; }", "-l", "javascript", "-x", "some $f in //function satisfies $f/name = 'add'", "--expect", "1"],
    ["-s", "function add(a, b) { return a + b; }", "-l", "javascript", "-x", "every $f in //function satisfies $f/name = 'add'", "--expect", "1"],
    ["-s", "let x = 1;", "-l", "typescript", "-x", "let $v := //name return $v", "-v", "value", "--expect", "1"],
);

tractor_tests!(bare_element_auto_prefix, repo_root(),
    ["-s", "let x = 1;", "-l", "typescript", "-x", "variable", "--expect", "1"],
    ["-s", "function foo() {}", "-l", "javascript", "-x", "function[name='foo']", "--expect", "1"],
);
