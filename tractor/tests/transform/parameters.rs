//! Cross-language: parameter shape markers.
//!
//! Every <parameter> / <param> carries an exhaustive marker
//! describing its role: <required/>, <optional/>, <rest/>,
//! <keyword/>, or a spread shape (<spread[list]/>, <spread[dict]/>).
//! Defaulted parameters stay <required/> with a <value> child.

use crate::support::semantic::*;

/// Every <param> carries an exhaustive marker: <required/> or
/// <optional/>. Covers required, optional (?), defaulted, and
/// rest parameters; also the JS-style untyped param shape.
#[test]
fn typescript() {
    claim("required TypeScript parameter carries required marker and type",
        &mut parse_src("typescript", "function call(required: string): void {}\n"),
        &multi_xpath(r#"
            //parameter[name='required']
                [required]
                [type/name='string']
        "#),
        1);

    claim("optional TypeScript parameter carries optional marker and type",
        &mut parse_src("typescript", "function call(optional?: number): void {}\n"),
        &multi_xpath(r#"
            //parameter[name='optional']
                [optional]
                [type/name='number']
        "#),
        1);

    claim("defaulted TypeScript parameter remains required and has value",
        &mut parse_src("typescript", "function call(defaulted: boolean = true): void {}\n"),
        &multi_xpath(r#"
            //parameter[name='defaulted']
                [required]
                [value]
        "#),
        1);

    claim("rest TypeScript parameter carries required and rest markers",
        &mut parse_src("typescript", "function call(...rest: string[]): void {}\n"),
        &multi_xpath(r#"
            //parameter[rest/name='rest']
                [required]
                [rest]
        "#),
        1);

    claim("untyped noTypes parameters are still required parameters",
        &mut parse_src("typescript", "function noTypes(x, y) {}\n"),
        "//function[name='noTypes']/parameter[required]",
        2);

    claim("mixing required and optional parameters tags only the optional parameter",
        &mut parse_src("typescript", "function f(a: string, b?: number) {}\n"),
        &multi_xpath(r#"
            //function[name='f']
                [count(parameter)=2]
                [parameter[name='a'][required][not(optional)]]
                [parameter[name='b'][optional][not(required)]]
        "#),
        1);
}

/// Ruby splat parameters distinguish iterable `*args` (list) from
/// mapping `**kwargs` (dict); keyword parameters (`key:`) carry a
/// <keyword/> marker distinguishing them from positional ones.
#[test]
fn ruby() {
    claim("Ruby splat parameter carries list spread marker",
        &mut parse_src("ruby", "def f(*xs)\nend\n"),
        "//spread[list][name='xs']",
        1);

    claim("Ruby keyword parameter carries keyword marker and default value",
        &mut parse_src("ruby", "def f(key: 1)\nend\n"),
        &multi_xpath(r#"
            //parameter[name='key']
                [keyword]
                [value]
        "#),
        1);

    claim("Ruby kwargs parameter carries dict spread marker",
        &mut parse_src("ruby", "def f(**kw)\nend\n"),
        "//spread[dict][name='kw']",
        1);
}
