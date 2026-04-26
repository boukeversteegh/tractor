//! C#: `expression_statement` rename to <expression>.

use crate::support::semantic::*;

/// Principle #5 — `expression_statement` renames to <expression>
/// (not the raw tree-sitter kind).
#[test]
fn csharp() {
    let mut tree = parse_src("csharp", r#"
        class X {
            void F() {
                int y = 0;
                y = 1;
            }
        }
    "#);

    claim("no raw `expression_statement` kind leak",
        &mut tree, "//expression_statement", 0);

    claim("`y = 1` renders as <expression>",
        &mut tree, "//expression", 1);
}
