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

/// TypeScript multi-declarator (`let i = 0, j = 100`) keeps the
/// `<declarator>` wrappers and tags them with `list="declarators"`
/// so JSON renders as `declarators: [...]`. Pre-iter-264, TS
/// unconditionally flattened the wrapper, which made name↔value
/// pairing positional-only and caused JSON consumers to lose
/// binding (children-overflow + singleton-key collision).
/// Single-declarator (`const x = 1`) still flattens.
#[test]
fn typescript_multi_declarator_keeps_wrappers() {
    claim("TS single-declarator `const x = 1` flattens (no <declarator>)",
        &mut parse_src("typescript", "const x = 1;\n"),
        "//variable[name='x'][not(declarator)][value/expression/number='1']",
        1);

    let mut tree = parse_src("typescript", "let i = 0, j = 100;\n");

    claim("TS multi-declarator keeps two <declarator> children with name↔value pairing",
        &mut tree,
        &multi_xpath(r#"
            //variable[let]
                [declarator[name='i'][value/expression/number='0']]
                [declarator[name='j'][value/expression/number='100']]
                [count(declarator)=2]
        "#),
        1);

    claim("TS multi-declarator declarators carry list='declarators' for JSON array shape",
        &mut tree,
        "//variable/declarator[@list='declarators']",
        2);
}

/// Goal #5: augmented_assignment unifies with plain assignment
/// as <assign> plus an <op> child carrying the operator. A single
/// //assign query matches every assignment. Iter 341 closed the
/// last cross-language gap: Python plain `=` now extracts to
/// `<op>=</op>` (annotated `x: int = 5` correctly skips `:` via
/// the `transformations::assignment` Custom handler). All 8
/// chain-inverting languages now uniform.
#[test]
fn python() {
    claim("plain assignment carries <op>=</op> wrapper (iter 341: Principle #5)",
        &mut parse_src("python", "x = 0\n"),
        "//assign[left/expression/name='x'][right/expression/int='0']/op='='",
        1);

    claim("annotated assignment correctly extracts `=` (skipping `:` separator)",
        &mut parse_src("python", "y: int = 0\n"),
        "//assign[left/expression/name='y'][type/name='int']/op='='",
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
