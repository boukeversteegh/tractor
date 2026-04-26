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

    claim("4 arms as siblings under <match>/<body>",
        &mut tree, "//match/body/arm", 4);

    claim("arm with literal pattern `0`",
        &mut tree, "//arm[pattern/int='0']", 1);

    claim("guard arm carries a <condition> child inside <pattern>",
        &mut tree, "//arm/pattern/condition", 1);

    claim("or-pattern uses pattern[or] markers (left-associative nesting)",
        &mut tree, "//arm/pattern/pattern[or]", 1);

    claim("each arm has a <pattern> and a <value>",
        &mut tree, "//arm[pattern and value]", 4);
}
