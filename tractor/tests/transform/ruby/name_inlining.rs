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
    let mut tree = parse_src("ruby", r#"
        class Calculator
          def add(a, b)
            a + b
          end
        end

        module Utils
          def self.greet(name)
            "hi, #{name}"
          end
        end
    "#);

    claim("Ruby declarations inline names while expression identifiers still render as <name>",
        &mut tree,
        &multi_xpath(r#"
            //program
                [class
                    [name='Calculator']
                    [body/method
                        [name='add']
                        [name='a']
                        [name='b']
                        [body//binary
                            [left/name='a']
                            [right/name='b']]]]
                [module
                    [name='Utils']
                    [.//method
                        [name='greet']
                        [singleton]
                        [name='name']]]
        "#),
        1);

    claim("inlined declaration names do not retain nested parser-name wrappers",
        &mut tree, "//class/name/identifier | //class/name/constant | //module/name/identifier", 0);
}
