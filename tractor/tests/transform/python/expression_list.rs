//! Python: `expression_list` wrapper flattening.

use crate::support::semantic::*;

/// Principle #12: `expression_list` (tuple-like return/yield
/// expressions) is a pure grouping node; drop it so the
/// expressions become direct children of the enclosing
/// <return>/<yield>/<assign>.
#[test]
fn python() {
    let mut tree = parse_src("python", r#"
def pair():
    return 1, 2

def triple():
    return "a", "b", "c"

def unpack():
    a, b = pair()
    return a + b
"#);

    claim("`return 1, 2` puts both ints as direct children of <return>",
        &mut tree, "//return[int='1' and int='2']", 1);

    claim("`return \"a\", \"b\", \"c\"` flattens 3 strings under <return>",
        &mut tree, "//return[count(string)=3]", 1);

    claim("tuple unpack `a, b = pair()` exposes both names directly under <assign>/left",
        &mut tree, "//assign/left[name='a' and name='b']", 1);
}
