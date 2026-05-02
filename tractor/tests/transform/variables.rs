//! Cross-language: variable declarations and assignment shape
//! (including compound / augmented assignments).

use crate::support::semantic::*;

/// Principle #2 — `variable_declarator` renames to <declarator>
/// (no underscores in the final vocabulary, short but not
/// abbreviated). Each declarator in a multi-variable declaration
/// is its own <declarator>.
#[test]
fn java() {
    claim("multi-variable declaration shape has one variable with two declarators",
        &mut parse_src("java", r#"
        class X {
            void f() { int x = 1, y = 2; }
        }
    "#),
        &multi_xpath(r#"
            //method[name='f']/body/variable
                [type/name='int']
                [declarator[name='x']
                    [value/expression/int='1']
                ]
                [declarator[name='y']
                    [value/expression/int='2']
                ]
                [count(declarator)=2]
        "#),
        1);
}

/// Goal #5: augmented_assignment unifies with plain assignment
/// as <assign> plus an <op> child carrying the compound operator.
/// A single //assign query matches every assignment.
#[test]
fn python() {
    claim("plain assignment has no op child",
        &mut parse_src("python", "x = 0\n"),
        "//assign[left/expression/name='x'][right/expression/int='0'][not(op)]",
        1);

    claim("arithmetic augmented assignment keeps assign shape with operator marker",
        &mut parse_src("python", "x += 1\n"), "//assign[op[assign and plus]]", 1);

    claim("power augmented assignment uses power marker",
        &mut parse_src("python", "x **= 1\n"), "//assign[op[assign and power]]", 1);

    claim("bitwise augmented assignment carries flat bitwise marker siblings",
        &mut parse_src("python", "x &= 1\n"), "//assign[op[assign and bitwise and and]]", 1);

    claim("shift augmented assignment carries flat shift marker siblings",
        &mut parse_src("python", "x <<= 1\n"), "//assign[op[assign and shift and left]]", 1);
}
