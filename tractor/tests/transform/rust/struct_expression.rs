//! Rust struct construction expression shape.

use crate::support::semantic::*;

/// Struct construction `Point { x: 1, y: 2 }` renders as
/// <literal> with a <name> child for the struct name and
/// <field> siblings for each initializer. Symmetric with JS/C#
/// object construction: //literal[name='Point'] finds every
/// Point construction site.
#[test]
fn rust() {
    let mut tree = parse_src("rust", r#"
        struct Point { x: i32, y: i32 }

        fn make() {
            let p = Point { x: 1, y: 2 };
            let q = Point { x: 0, ..p };
        }
    "#);

    claim("two Point construction sites",
        &mut tree, "//literal[name='Point']", 2);

    claim("struct name lives as <name> on <literal> (NOT a <type>)",
        &mut tree, "//literal/type", 0);

    claim("first construction has 2 plain fields, no [base]",
        &mut tree, "//literal[name='Point'][not(body/field[base])]/body/field", 2);

    claim("second construction has a [base] field for `..p`",
        &mut tree, "//literal/body/field[base][name='p']", 1);

    claim("field initializers carry <value> children",
        &mut tree, "//literal/body/field[name='x']/value/int", 2);
}
