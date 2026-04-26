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

    claim("type switch carries <type/> marker",
        &mut tree, "//switch[type]", 1);

    claim("both regular and type switch collapse to <switch>",
        &mut tree, "//switch", 2);
}
