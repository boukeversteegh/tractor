//! Go: switch markers — type switch carries <type/>.

use crate::support::semantic::*;

/// `switch x.(type) { … }` and a regular `switch x { … }` both
/// collapse to <switch>. The type switch carries a <type/>
/// marker so `//switch[type]` picks out every type switch.
#[test]
fn go() {
    claim("Go type switch carries type marker and switched value",
        &mut parse_src("go", r#"
        package main

        func f(x interface{}) {
            switch x.(type) { case int: }
        }
    "#),
        &multi_xpath(r#"
            //switch
                [type]
                [value/expression/name='x']
                [type/name='int']
        "#),
        1);

    claim("Go regular switch has no type marker",
        &mut parse_src("go", r#"
        package main

        func f(x int) {
            switch x { case 1: }
        }
    "#),
        &multi_xpath(r#"
            //switch
                [not(type)]
                [value/expression/name='x']
                [case/value/expression/int='1']
        "#),
        1);
}
