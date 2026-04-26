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

    claim("class name is inlined text on <name>",
        &mut tree, "//class/name='Calculator'", 1);

    claim("class <name> has no <identifier> child",
        &mut tree, "//class/name/identifier", 0);

    claim("class <name> has no nested <constant> child",
        &mut tree, "//class/name/constant", 0);

    claim("module name is inlined text on <name>",
        &mut tree, "//module/name='Utils'", 1);

    claim("method name `add` is inlined text on <name>",
        &mut tree, "//method/name='add'", 1);

    claim("singleton method `self.greet` carries [singleton] marker",
        &mut tree, "//method[singleton][name='greet']", 1);

    claim("method parameters are <name> elements (identifier renamed)",
        &mut tree, "//method[name='add']/name[. ='a' or .='b']", 2);

    claim("identifiers in expressions render as <name>",
        &mut tree, "//binary/left/name[.='a']", 1);
}
