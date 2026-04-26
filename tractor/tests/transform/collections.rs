//! Cross-language: array / list / dict / set literals, comprehension
//! markers, and spread / rest forms.

use crate::support::semantic::*;

/// Python collection literals unify by produced type. <list>,
/// <dict>, <set>, <generator> carry exhaustive <literal/> or
/// <comprehension/> markers so queries can distinguish
/// `[x for x in xs]` from `[1, 2, 3]` without kind-specific
/// element names.
#[test]
fn python_collections() {
    let mut tree = parse_src("python", r#"
nums = [1, 2, 3]
squares = [x * x for x in nums]
pairs = {"a": 1, "b": 2}
inverted = {v: k for k, v in pairs.items()}
unique = {1, 2, 3}
uniq_sq = {x * x for x in nums}
gen = (x for x in nums)
"#);

    claim("list literal carries <literal/>",
        &mut tree, "//list[literal]", 1);

    claim("list comprehension carries <comprehension/>",
        &mut tree, "//list[comprehension]", 1);

    claim("dict literal carries <literal/>",
        &mut tree, "//dict[literal]", 1);

    claim("dict comprehension carries <comprehension/>",
        &mut tree, "//dict[comprehension]", 1);

    claim("set literal carries <literal/>",
        &mut tree, "//set[literal]", 1);

    claim("set comprehension carries <comprehension/>",
        &mut tree, "//set[comprehension]", 1);

    claim("generator expression renders as <generator>",
        &mut tree, "//generator", 1);

    claim("literal and comprehension are mutually exclusive on collections",
        &mut tree, "//*[literal and comprehension]", 0);
}

/// Ruby percent-literal arrays collapse to <array> with a
/// <string/> / <symbol/> marker so the element name matches a
/// normal array while the flavor stays queryable.
#[test]
fn ruby() {
    let mut tree = parse_src("ruby", r#"
        A = %w[one two]
        B = %i[alpha beta]
        C = [1, 2]
    "#);

    claim("%w[…] carries <string/>",
        &mut tree, "//array[string]", 1);

    claim("%i[…] carries <symbol/>",
        &mut tree, "//array[symbol]", 1);

    claim("all three forms collapse to <array>",
        &mut tree, "//array", 3);
}

/// `*args` and `**kwargs` collapse to <spread> but carry a
/// <list/> / <dict/> marker that survives argument, pattern, and
/// literal contexts so shape queries work without string matching
/// on `*` / `**` operator text.
#[test]
fn python_spread() {
    let mut tree = parse_src("python", r#"
def f(*args, **kwargs): pass
g(*xs, **kw)
[*a, *b]
{**a, **b}
"#);

    claim("`*args`, `g(*xs)`, `[*a]`, `[*b]` all carry spread[list]",
        &mut tree, "//spread[list]", 4);

    claim("`**kwargs`, `g(**kw)`, `{**a}`, `{**b}` all carry spread[dict]",
        &mut tree, "//spread[dict]", 4);
}
