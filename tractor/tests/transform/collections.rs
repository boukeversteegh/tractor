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
    let mut list_literal = parse_src("python", "[1, 2, 3]\n");

    claim("Python list literal carries literal marker",
        &mut list_literal, "//list[literal]", 1);

    claim("literal and comprehension are mutually exclusive on collections",
        &mut list_literal, "//*[literal and comprehension]", 0);

    claim("Python list comprehension carries comprehension marker",
        &mut parse_src("python", "[x * x for x in nums]\n"), "//list[comprehension]", 1);

    claim("Python dict literal carries literal marker",
        &mut parse_src("python", r#"{"a": 1, "b": 2}"#), "//dict[literal]", 1);

    claim("Python dict comprehension carries comprehension marker",
        &mut parse_src("python", "{v: k for k, v in pairs.items()}\n"),
        "//dict[comprehension]",
        1);

    claim("Python set literal carries literal marker",
        &mut parse_src("python", "{1, 2, 3}\n"), "//set[literal]", 1);

    claim("Python set comprehension carries comprehension marker",
        &mut parse_src("python", "{x * x for x in nums}\n"), "//set[comprehension]", 1);

    claim("Python generator expression stays a bare generator node",
        &mut parse_src("python", "(x for x in nums)\n"), "//generator", 1);
}

/// Ruby percent-literal arrays collapse to <array> with a
/// <string/> / <symbol/> marker so the element name matches a
/// normal array while the flavor stays queryable.
#[test]
fn ruby() {
    claim("Ruby percent string array carries string marker",
        &mut parse_src("ruby", "%w[one two]\n"), "//array[string]", 1);

    claim("Ruby percent symbol array carries symbol marker",
        &mut parse_src("ruby", "%i[alpha beta]\n"), "//array[symbol]", 1);

    claim("Ruby ordinary array has no percent-literal flavor marker",
        &mut parse_src("ruby", "[1, 2]\n"), "//array[not(string)][not(symbol)]", 1);
}

/// `*args` and `**kwargs` collapse to <spread> but carry a
/// <list/> / <dict/> marker that survives argument, pattern, and
/// literal contexts so shape queries work without string matching
/// on `*` / `**` operator text.
#[test]
fn python_spread() {
    claim("spread nodes carry list/dict markers in parameter and argument contexts",
        &mut parse_src("python", r#"
def f(*args, **kwargs): pass
"#),
        &multi_xpath(r#"
            //function[name='f']
                [spread[list][name='args']]
                [spread[dict][name='kwargs']]
        "#),
        1);

    claim("call spread nodes carry list/dict markers",
        &mut parse_src("python", "g(*xs, **kw)\n"),
        &multi_xpath(r#"
            //call
                [spread[list][name='xs']]
                [spread[dict][name='kw']]
        "#),
        1);

    claim("list literal spread nodes carry list markers",
        &mut parse_src("python", "[*a, *b]\n"), "//list[count(spread[list])=2]", 1);

    claim("dict literal spread nodes carry dict markers",
        &mut parse_src("python", "{**a, **b}\n"), "//dict[count(spread[dict])=2]", 1);
}
