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
