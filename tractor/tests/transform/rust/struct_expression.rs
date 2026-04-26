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

    claim("Rust struct literals keep constructor name, field values, and update base flat",
        &mut tree,
        &multi_xpath(r#"
            //function[name='make']/body
                [let
                    [name='p']
                    [value/literal
                        [name='Point']
                        [body
                            [field
                                [name='x']
                                [value/int='1']]
                            [field
                                [name='y']
                                [value/int='2']]
                            [not(field[base])]]]]
                [let
                    [name='q']
                    [value/literal
                        [name='Point']
                        [body
                            [field
                                [name='x']
                                [value/int='0']]
                            [field
                                [base]
                                [name='p']]]]]
        "#),
        1);

    claim("struct literal names do not render as type references",
        &mut tree, "//literal/type", 0);
}
