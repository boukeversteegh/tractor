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

    claim("struct hoists to top level (no enclosing <type>)",
        &mut tree, "//file/struct[name='Config']", 1);

    claim("interface hoists to top level (no enclosing <type>)",
        &mut tree, "//file/interface[name='Greeter']", 1);

    claim("uppercase struct name carries <exported/>",
        &mut tree, "//struct[exported][name='Config']", 1);

    claim("uppercase interface name carries <exported/>",
        &mut tree, "//interface[exported][name='Greeter']", 1);

    claim("the `type` wrapper does NOT also surface a <type> for the struct",
        &mut tree, "//file/type[name='Config']", 0);
}
