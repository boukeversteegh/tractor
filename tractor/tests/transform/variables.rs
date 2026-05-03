//! Cross-language: variable declarations and assignment shape
//! (including compound / augmented assignments).

use crate::support::semantic::*;

/// Principle #2 — `variable_declarator` renames to <declarator>.
/// Multi-declarator declarations (rare in practice, but valid syntax)
/// keep the <declarator> wrapper because each is a role-mixed
/// name+value group whose name↔value pairing depends on the
/// wrapper. Single-declarator declarations flatten the wrapper
/// (see java_single_declarator_flattens below).
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

/// Single-declarator Java fields and locals flatten the
/// `<declarator>` wrapper so `int x = 1;` produces
/// `field/{type, name, value}` rather than
/// `field/{type, declarator/{name, value}}`. Multi-declarator
/// keeps the wrapper (see `java` above for the multi-declarator
/// case). Same flatten applies to local variables (within-Java
/// Principle #5: fields and locals share shape).
#[test]
fn java_single_declarator_flattens() {
    let mut tree = parse_src("java", r#"
        class T {
            int x = 1;
            String name;
            void f() { int y = 2; }
        }
    "#);

    claim("single-declarator Java field exposes name as a direct child (no <declarator>)",
        &mut tree,
        "//field[name='x'][not(declarator)][value/expression/int='1']",
        1);

    claim("single-declarator Java field without initializer exposes name directly",
        &mut tree,
        "//field[name='name'][not(declarator)][not(value)]",
        1);

    claim("single-declarator Java local variable also flattens",
        &mut tree,
        "//variable[name='y'][not(declarator)][value/expression/int='2']",
        1);
}

/// Same archetype for C#: single-declarator fields and locals
/// flatten; multi-declarator (rare in C#) keeps wrappers.
#[test]
fn csharp_single_declarator_flattens() {
    let mut tree = parse_src("csharp", r#"
        class T {
            int X = 1;
            string name;
            void M() { int y = 2; }
        }
    "#);

    claim("single-declarator C# field exposes name directly (no <declarator>)",
        &mut tree,
        "//field[name='X'][not(declarator)]",
        1);

    claim("single-declarator C# local variable flattens",
        &mut tree,
        "//variable[name='y'][not(declarator)]",
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
