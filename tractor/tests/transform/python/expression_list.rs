//! Python: `expression_list` wrapper flattening.

use crate::support::semantic::*;

/// Principle #12: `expression_list` (tuple-like return/yield
/// expressions) is a pure grouping node; drop it so the
/// expressions become direct children of the enclosing
/// <return>/<yield>/<assign>.
#[test]
fn python() {
    claim("Python return expression list flattens directly under return",
        &mut parse_src("python", r#"
def pair():
    return 1, 2
"#),
        &multi_xpath(r#"
            //return
                [expression/int='1']
                [expression/int='2']
        "#),
        1);

    claim("Python string expression list flattens directly under return",
        &mut parse_src("python", r#"
def triple():
    return "a", "b", "c"
"#),
        "//return[count(expression/string)=3]",
        1);

    claim("Python assignment target expression list flattens directly under left",
        &mut parse_src("python", "a, b = pair()\n"),
        &multi_xpath(r#"
            //assign/left
                [expression/name='a']
                [expression/name='b']
        "#),
        1);

    // Multi-value `return a, b, c` produces multiple <expression>
    // sibling direct children of <return>. Per Principle #19 they
    // are role-uniform (each is a return value); tag with
    // `list="expressions"` so JSON renders as `expressions: [...]`
    // array rather than overflowing to `children`.
    claim("Python multi-value return tags each <expression> with list='expressions'",
        &mut parse_src("python", r#"
def f():
    return a, b, c
"#),
        "//return/expression",
        3);
}
