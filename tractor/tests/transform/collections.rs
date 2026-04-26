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

    claim("collection assignment shapes distinguish literal and comprehension forms",
        &mut tree,
        &multi_xpath(r#"
            //module
                [assign[left/name='nums']
                    [right/list[literal]]
                ]
                [assign[left/name='squares']
                    [right/list[comprehension]]
                ]
                [assign[left/name='pairs']
                    [right/dict[literal]]
                ]
                [assign[left/name='inverted']
                    [right/dict[comprehension]]
                ]
                [assign[left/name='unique']
                    [right/set[literal]]
                ]
                [assign[left/name='uniq_sq']
                    [right/set[comprehension]]
                ]
                [assign[left/name='gen']
                    [right/generator]
                ]
        "#),
        1);

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

    claim("Ruby array assignment shapes preserve percent-literal flavors",
        &mut tree,
        &multi_xpath(r#"
            //program
                [assign[left/constant='A']
                    [right/array[string]]
                ]
                [assign[left/constant='B']
                    [right/array[symbol]]
                ]
                [assign[left/constant='C']
                    [right/array[not(string)][not(symbol)]]
                ]
                [count(assign/right/array)=3]
        "#),
        1);
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

    claim("spread shapes cover parameter, call, list, and dict contexts",
        &mut tree,
        &multi_xpath(r#"
            //module
                [function[name='f']
                    [spread[list][name='args']]
                    [spread[dict][name='kwargs']]
                ]
                [call
                    [spread[list][name='xs']]
                    [spread[dict][name='kw']]
                ]
                [list[spread[list][name='a']][spread[list][name='b']]]
                [dict[spread[dict][name='a']][spread[dict][name='b']]]
        "#),
        1);
}
