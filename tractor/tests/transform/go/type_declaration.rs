//! Go: `type_declaration` wrapper drop.

use crate::support::semantic::*;

/// Go's `type_declaration` wrapper is dropped; `type_spec`
/// renders as <type> directly. Parallel with struct/interface
/// declarations so //type queries find every declared type.
#[test]
fn go() {
    let mut tree = parse_src("go", r#"
        package main

        type ID uint64

        type User struct {
            Name string
            Age  int
        }

        type Greeter interface {
            Greet() string
        }
    "#);

    claim("Go file shape exposes plain, struct, and interface type declarations directly",
        &mut tree,
        &multi_xpath(r#"
            //file
                [type[name='ID']
                    [type[name='uint64']]]
                [struct[name='User']
                    [field
                        [name='Name']
                        [type[name='string']]]
                    [field
                        [name='Age']
                        [type[name='int']]]]
                [interface[name='Greeter']
                    [method[name='Greet']
                        [returns/type[name='string']]]]
        "#),
        1);

    claim("struct/interface forms do NOT also produce a <type> wrapper",
        &mut tree, "//file/type[name='User'] | //file/type[name='Greeter']", 0);
}
