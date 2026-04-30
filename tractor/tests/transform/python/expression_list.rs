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
}
