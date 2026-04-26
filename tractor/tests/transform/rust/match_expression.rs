//! Rust `match` expression shape.

use crate::support::semantic::*;

/// Principle #12: `match_block` (the `{ ... }` wrapper around
/// match arms) is a pure grouping node; drop it so arms are
/// direct siblings of <match> via <body>.
#[test]
fn rust() {
    let mut tree = parse_src("rust", r#"
        fn classify(n: i32) -> &'static str {
            match n {
                0 => "zero",
                1 | 2 | 3 => "small",
                _ if n < 0 => "negative",
                _ => "other",
            }
        }
    "#);

    claim("Rust match body exposes arms directly with literal, or-pattern, guard, and fallback shapes",
        &mut tree,
        &multi_xpath(r#"
            //match
                [value/name='n']
                [body
                    [count(arm)=4]
                    [arm
                        [pattern/int='0']
                        [value]]
                    [arm
                        [pattern/pattern[or]]
                        [value]]
                    [arm
                        [pattern
                            [condition]]
                        [value]]
                    [arm
                        [pattern]
                        [value]]]
        "#),
        1);
}
