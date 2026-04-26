//! Rust struct construction expression shape.

use crate::support::semantic::*;

/// Struct construction `Point { x: 1, y: 2 }` renders as
/// <literal> with a <name> child for the struct name and
/// <field> siblings for each initializer. Symmetric with JS/C#
/// object construction: //literal[name='Point'] finds every
/// Point construction site.
#[test]
fn rust() {
    claim("Rust struct literal keeps constructor name and field values flat",
        &mut parse_src("rust", r#"
        struct Point { x: i32, y: i32 }

        fn make() {
            let p = Point { x: 1, y: 2 };
        }
    "#),
        &multi_xpath(r#"
            //literal
                [name='Point']
                [body
                    [field
                        [name='x']
                        [value/int='1']]
                    [field
                        [name='y']
                        [value/int='2']]
                    [not(field[base])]]
                [not(type)]
        "#),
        1);

    claim("Rust struct update keeps the base field flat",
        &mut parse_src("rust", r#"
        struct Point { x: i32, y: i32 }

        fn make(p: Point) {
            let q = Point { x: 0, ..p };
        }
    "#),
        &multi_xpath(r#"
            //literal
                [name='Point']
                [body
                    [field
                        [name='x']
                        [value/int='0']]
                    [field
                        [base]
                        [name='p']]]
                [not(type)]
        "#),
        1);
}
