//! TSX (TypeScript + JSX) — covers the JSX-specific elements
//! (<element>, <opening>, <closing>, <prop>, <value>, <text>) on
//! top of the standard TS shape inherited from the typescript
//! transform.
//!
//! One focused source per construct.

use crate::support::semantic::*;

/// Paired vs self-closing JSX has two different shapes — a paired
/// `<div>...</div>` carries `<opening>` and `<closing>` siblings
/// holding the tag name; a self-closing `<span/>` puts the tag
/// name directly under `<element>` with no opening/closing wrappers.
#[test]
fn tsx_paired_element_uses_opening_and_closing() {
    claim("paired `<div>x</div>` exposes <opening>/<name> and <closing>/<name>",
        &mut parse_src("tsx", r#"
            function F() { return <div>x</div>; }
        "#),
        &multi_xpath(r#"
            //element
                [opening/name='div']
                [closing/name='div']
        "#),
        1);
}

#[test]
fn tsx_self_closing_element_has_name_directly() {
    claim("self-closing `<span/>` puts <name> directly under <element>, no opening/closing",
        &mut parse_src("tsx", r#"
            function F() { return <span/>; }
        "#),
        &multi_xpath(r#"
            //element
                [name='span']
                [not(opening)]
                [not(closing)]
        "#),
        1);
}

#[test]
fn tsx_element_attribute_renders_as_prop() {
    claim(r#"`<div className="x"/>` puts the attribute under <prop> as a direct child of <element>"#,
        &mut parse_src("tsx", r#"
            function F() { return <div className="x"/>; }
        "#),
        "//element/prop[name='className']",
        1);
}

#[test]
fn tsx_text_child_renders_as_text_leaf() {
    claim("static text inside an element renders as <text>",
        &mut parse_src("tsx", r#"
            function F() { return <h1>Hello</h1>; }
        "#),
        "//element/text",
        1);
}

#[test]
fn tsx_expression_brace_renders_as_value() {
    claim("`{name}` inside an element renders as <value> (queryable expression)",
        &mut parse_src("tsx", r#"
            function F(name: string) { return <h1>{name}</h1>; }
        "#),
        "//element/value",
        1);
}

#[test]
fn tsx_nested_elements() {
    claim("`<div><span/></div>` nests the inner self-closing element under the outer paired one",
        &mut parse_src("tsx", r#"
            function F() { return <div><span/></div>; }
        "#),
        &multi_xpath(r#"
            //element
                [opening/name='div']
                [element[name='span'][not(opening)]]
                [closing/name='div']
        "#),
        1);
}
