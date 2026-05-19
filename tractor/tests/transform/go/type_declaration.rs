//! Go: `type_declaration` wrapper drop.

use crate::support::semantic::*;

/// Go's `type_declaration` wrapper is dropped; `type_spec`
/// renders as <type> directly. Parallel with struct/interface
/// declarations so //type queries find every declared type.
#[test]
fn go() {
    claim("Go plain type declaration exposes the declared type directly",
        &mut parse_src("go", r#"
        package main

        type ID uint64
    "#),
        &multi_xpath(r#"
            //type[name='ID']
                [type[name='uint64']]
        "#),
        1);

    claim("Go struct type declaration exposes struct directly",
        &mut parse_src("go", r#"
        package main

        type User struct {
            Name string
            Age  int
        }
    "#),
        &multi_xpath(r#"
            //struct[name='User']
                [field
                    [name='Name']
                    [type[name='string']]]
                [field
                    [name='Age']
                    [type[name='int']]]
                [not(../type[name='User'])]
        "#),
        1);

    claim("Go interface type declaration exposes interface directly",
        &mut parse_src("go", r#"
        package main

        type Greeter interface {
            Greet() string
        }
    "#),
        &multi_xpath(r#"
            //interface[name='Greeter']
                [method[name='Greet']
                    [returns/type[name='string']]]
                [not(../type[name='Greeter'])]
        "#),
        1);
}
