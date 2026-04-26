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

    claim("plain `type ID uint64` renders as <type>",
        &mut tree, "//file/type[name='ID']", 1);

    claim("struct/interface forms do NOT also produce a <type> wrapper",
        &mut tree, "//file/type[name='User'] | //file/type[name='Greeter']", 0);

    claim("no `type_declaration` grammar wrapper leaks",
        &mut tree, "//type_declaration", 0);

    claim("inner referenced type of `type ID uint64`",
        &mut tree, "//type[name='ID']/type[name='uint64']", 1);
}
