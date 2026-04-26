//! Java: `parenthesized_expression` wrapper drop.

use crate::support::semantic::*;

/// Principle #12 — `parenthesized_expression` is grammar
/// bleed-through; drop the wrapper so inner expressions sit
/// directly under their enclosing node. The parens remain as
/// text children.
#[test]
fn java() {
    let mut tree = parse_src("java", r#"
        class X {
            boolean f(int n) { return (n + 1) > 0; }
        }
    "#);

    claim("no <parenthesized_expression> wrapper",
        &mut tree, "//parenthesized_expression", 0);
}
