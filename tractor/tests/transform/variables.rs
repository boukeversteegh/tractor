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

    claim("each declarator in a multi-variable declaration is its own <declarator>",
        &mut tree, "//variable/declarator", 2);
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

    claim("11 statement-level assignments (1 plain + 10 compound)",
        &mut tree, "//body/assign", 11);

    claim("plain `=` is the only top-level assign without an <op>",
        &mut tree, "//body/assign[not(op)]", 1);

    claim("10 compound assignments carry an <op> child",
        &mut tree, "//body/assign/op", 10);

    claim("`+=` carries assign[plus] marker",
        &mut tree, "//assign/op/assign[plus]", 1);

    claim("`-=` carries assign[minus] marker",
        &mut tree, "//assign/op/assign[minus]", 1);

    claim("`**=` carries assign[power] marker",
        &mut tree, "//assign/op/assign[power]", 1);

    claim("bitwise compound ops carry assign/bitwise[*] markers",
        &mut tree, "//assign/op/assign/bitwise[and] | //assign/op/assign/bitwise[or] | //assign/op/assign/bitwise[xor]", 3);

    claim("shift compound ops carry assign/shift[*] markers",
        &mut tree, "//assign/op/assign/shift[left] | //assign/op/assign/shift[right]", 2);
}
