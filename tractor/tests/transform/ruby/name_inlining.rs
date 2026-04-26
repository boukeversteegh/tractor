//! Ruby: identifier-to-name renaming and inlining inside
//! method/class/module declarations.

use crate::support::semantic::*;

/// (1) Every Ruby `identifier` becomes <name> unconditionally.
/// (2) When a <name> wrapper sits inside method/class/module and
/// contains a single identifier, the transform inlines its text
/// directly, so <method><name>foo</name>… not
/// <method><name><identifier>foo</identifier></name>….
#[test]
fn ruby() {
    claim("Ruby class and method declarations inline declaration names",
        &mut parse_src("ruby", r#"
        class Calculator
          def add(a, b)
            a + b
          end
        end
    "#),
        &multi_xpath(r#"
            //class
                [name='Calculator']
                [body/method
                    [name='add']
                    [name='a']
                    [name='b']
                    [body//binary
                        [left/name='a']
                        [right/name='b']]]
                [not(name/identifier | name/constant)]
        "#),
        1);

    claim("Ruby singleton method declaration carries singleton marker",
        &mut parse_src("ruby", r#"
        module Utils
          def self.greet(name)
            "hi"
          end
        end
    "#),
        &multi_xpath(r#"
            //module
                [name='Utils']
                [.//method
                    [name='greet']
                    [singleton]
                    [name='name']]
                [not(name/identifier)]
        "#),
        1);
}
