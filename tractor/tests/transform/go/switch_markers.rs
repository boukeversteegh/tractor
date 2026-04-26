//! Go: switch markers — type switch carries <type/>.

use crate::support::semantic::*;

/// `switch x.(type) { … }` and a regular `switch x { … }` both
/// collapse to <switch>. The type switch carries a <type/>
/// marker so `//switch[type]` picks out every type switch.
#[test]
fn go() {
    let mut tree = parse_src("go", r#"
        package main

        func f(x interface{}) {
            switch x.(type) { case int: }
            switch x { case 1: }
        }
    "#);

    claim("Go function body keeps both switches flat and marks only the type switch",
        &mut tree,
        &multi_xpath(r#"
            //function[name='f']/body
                [switch
                    [type]
                    [value/name='x']
                    [type/name='int']]
                [switch
                    [not(type)]
                    [value/name='x']
                    [expression_case/value/int='1']]
        "#),
        1);
}
