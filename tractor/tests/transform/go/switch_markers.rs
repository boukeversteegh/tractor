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
                [case/type/name='int']
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

/// Go type-switch case with multiple types — `case int, int32:`
/// produces `<case>` with multiple `<type>` siblings (one per
/// alternative). Per Principle #19 they're role-uniform; tag with
/// `list="types"` so JSON renders as `types: [...]` instead of
/// overflowing.
#[test]
fn go_type_switch_multi_type_case_lists_types() {
    claim("Go type-switch case with two types tags each <type> with list='types'",
        &mut parse_src("go", r#"
        package main
        func f(x interface{}) {
            switch x.(type) { case int32, int64: }
        }
    "#),
        "//case/type[@list='types']",
        2);

    claim("Go type-switch case with single type keeps singleton <type>",
        &mut parse_src("go", r#"
        package main
        func f(x interface{}) {
            switch x.(type) { case int: }
        }
    "#),
        "//case/type[not(@list)]",
        1);
}
