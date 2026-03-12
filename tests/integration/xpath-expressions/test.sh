#!/usr/bin/env bash
# Tests for XPath 3.1 expression forms (let, for, if, quantified, etc.)
# Ensures normalize_xpath doesn't break these by auto-prefixing with //
source "$(dirname "$0")/../common.sh"

echo "XPath 3.1 expressions:"

# let expressions
run_test tractor test -s "let x = 1; let y = 2;" -l typescript -x "let \$v := //variable return \$v/name" -v value --expect 2 -m "let expression returns bound results"

# for expressions
run_test tractor test -s "let x = 1; let y = 2;" -l typescript -x "for \$v in //name return string(\$v)" -v value --expect 2 -m "for expression iterates over matches"

# if expressions
run_test tractor test -s "let x = 1;" -l typescript -x "if (//variable) then //name else ()" -v value --expect 1 -m "if expression with true branch"
run_test tractor test -s "let x = 1;" -l typescript -x "if (//function) then //name else //variable" -v value --expect 1 -m "if expression with false branch"

# quantified expressions (some/every return boolean atomics)
run_test tractor test -s "function add(a, b) { return a + b; }" -l javascript -x "some \$f in //function satisfies \$f/name = 'add'" --expect 1 -m "some quantified expression"
run_test tractor test -s "function add(a, b) { return a + b; }" -l javascript -x "every \$f in //function satisfies \$f/name = 'add'" --expect 1 -m "every quantified expression"

# variable references (standalone)
run_test tractor test -s "let x = 1;" -l typescript -x "let \$v := //name return \$v" -v value --expect 1 -m "let with variable reference in return"

# bare element names should still get auto-prefixed
run_test tractor test -s "let x = 1;" -l typescript -x "variable" --expect 1 -m "bare element name still auto-prefixed"
run_test tractor test -s "function foo() {}" -l javascript -x "function[name='foo']" --expect 1 -m "bare element with predicate still auto-prefixed"

report
