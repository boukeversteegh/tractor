//! Go: hoisting `type Foo struct {…}` and `type Foo interface {…}`
//! out of the spec wrapper.

use crate::support::semantic::*;

/// Goal #5 mental model — `type Foo struct { … }` and
/// `type Foo interface { … }` hoist: the outer element becomes
/// <struct> or <interface> directly instead of the Go-spec
/// `<type>` wrapper.
#[test]
fn go() {
    claim("Go struct declaration hoists to top-level struct node",
        &mut parse_src("go", r#"
        package main

        type Config struct {
            Host string
            Port int
        }
    "#),
        &multi_xpath(r#"
            //struct[name='Config']
                [exported]
                [field
                    [name='Host']
                    [type[name='string']]]
                [field
                    [name='Port']
                    [type[name='int']]]
                [not(../type[name='Config'])]
        "#),
        1);

    claim("Go interface declaration hoists to top-level interface node",
        &mut parse_src("go", r#"
        package main

        type Greeter interface {
            Greet() string
        }
    "#),
        &multi_xpath(r#"
            //interface[name='Greeter']
                [exported]
                [method[name='Greet']
                    [returns/type[name='string']]]
                [not(../type[name='Greeter'])]
        "#),
        1);
}

/// Multi-field struct declarations tag each `<field>` sibling with
/// `list="fields"` so JSON renders as `fields: [...]` array.
/// Single-field struct keeps the singleton `<field>` JSON key
/// (cardinality discriminator on `tag_multi_role_children`).
#[test]
fn go_multi_field_struct_lists_fields() {
    claim("Go 2+ struct fields emit one <field> each",
        &mut parse_src("go", r#"
        package main
        type Pair struct { A int; B string }
    "#),
        "//struct/field",
        2);

    claim("Go single-field struct emits one <field>",
        &mut parse_src("go", r#"
        package main
        type One struct { Only int }
    "#),
        "//struct/field",
        1);
}
