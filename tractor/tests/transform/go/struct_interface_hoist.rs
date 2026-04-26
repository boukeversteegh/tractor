//! Go: hoisting `type Foo struct {…}` and `type Foo interface {…}`
//! out of the spec wrapper.

use crate::support::semantic::*;

/// Goal #5 mental model — `type Foo struct { … }` and
/// `type Foo interface { … }` hoist: the outer element becomes
/// <struct> or <interface> directly instead of the Go-spec
/// `<type>` wrapper.
#[test]
fn go() {
    let mut tree = parse_src("go", r#"
        package main

        type Config struct {
            Host string
            Port int
        }

        type Greeter interface {
            Greet() string
        }
    "#);

    claim("Go struct and interface declarations hoist to top-level semantic nodes",
        &mut tree,
        &multi_xpath(r#"
            //file
                [struct[name='Config']
                    [exported]
                    [field
                        [name='Host']
                        [type[name='string']]]
                    [field
                        [name='Port']
                        [type[name='int']]]]
                [interface[name='Greeter']
                    [exported]
                    [method[name='Greet']
                        [returns/type[name='string']]]]
        "#),
        1);

    claim("the `type` wrapper does NOT also surface a <type> for the struct",
        &mut tree, "//file/type[name='Config']", 0);
}
