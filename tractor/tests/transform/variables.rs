//! Cross-language: variable declarations and assignment shape
//! (including compound / augmented assignments).

use crate::support::semantic::*;

/// Principle #2 — `variable_declarator` renames to <declarator>
/// (no underscores in the final vocabulary, short but not
/// abbreviated). Each declarator in a multi-variable declaration
/// is its own <declarator>.
#[test]
fn java() {
    let mut tree = parse_src("java", r#"
        class X {
            void f() { int x = 1, y = 2; }
        }
    "#);

    claim("multi-variable declaration shape has one variable with two declarators",
        &mut tree,
        &multi_xpath(r#"
            //method[name='f']/body/variable
                [type/name='int']
                [declarator[name='x']
                    [value/int='1']
                ]
                [declarator[name='y']
                    [value/int='2']
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
    let mut tree = parse_src("python", r#"
def ops():
    x = 0
    x += 1
    x -= 2
    x *= 3
    x //= 2
    x **= 2
    x &= 0xFF
    x |= 0x10
    x ^= 0x01
    x <<= 1
    x >>= 1
"#);

    claim("ops body has one plain assignment and compound assignment operator shapes",
        &mut tree,
        &multi_xpath(r#"
            //function[name='ops']/body
                [assign[left/name='x']
                    [right/int='0']
                    [not(op)]
                ]
                [assign[op/assign[plus]]]
                [assign[op/assign[minus]]]
                [assign[op/assign[power]]]
                [assign[op/assign/bitwise[and]]]
                [assign[op/assign/bitwise[or]]]
                [assign[op/assign/bitwise[xor]]]
                [assign[op/assign/shift[left]]]
                [assign[op/assign/shift[right]]]
                [count(assign)=11]
                [count(assign/op)=10]
        "#),
        1);
}
