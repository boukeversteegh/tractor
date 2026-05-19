//! XML semantic shape: elements, attributes, and text leaves
//! preserved by tractor's XML transform.
//!
//! XML is a "data" language — its grammar IS its data, so the
//! semantic transform is mostly identity. The interesting
//! invariants are: attributes survive as `@key='value'` predicates,
//! text content is queryable directly under the element, and
//! structural ancestry works as expected.
//!
//! One focused source per construct.

use crate::support::semantic::*;

#[test]
fn xml_element_with_attribute_predicate() {
    claim(r#"`<item type="feature">` exposes the attribute as `@type='feature'`"#,
        &mut parse_src("xml", r#"
            <items>
                <item type="feature"/>
                <item type="bug"/>
                <item type="feature"/>
            </items>
        "#),
        "//item[@type='feature']",
        2);
}

#[test]
fn xml_element_text_as_child_predicate() {
    claim("element text is queryable as a child predicate (`item[status='complete']`)",
        &mut parse_src("xml", r#"
            <items>
                <item><status>complete</status></item>
                <item><status>pending</status></item>
            </items>
        "#),
        "//item[status='complete']",
        1);
}

#[test]
fn xml_attribute_axis_query() {
    claim("the `@name` axis returns the attribute itself, not the element",
        &mut parse_src("xml", r#"<project name="sample"></project>"#),
        "//project/@name",
        1);
}

#[test]
fn xml_descendant_text_leaves() {
    claim("nested text leaves under any depth are reachable via the descendant axis",
        &mut parse_src("xml", r#"
            <items>
                <item><name>Login</name></item>
                <item><name>Logout</name></item>
                <item><name>Fix validation</name></item>
            </items>
        "#),
        "//name",
        3);
}

/// Deep-path predicate (`a[b/c/d]`) — the predicate is satisfied
/// by the existence of the full descendant chain. Surfaced as
/// regression issue #129: 3-level predicates must match the same
/// as the `count()` workaround.
#[test]
fn xml_deep_path_predicate() {
    claim("`a[b/c/d]` matches the ancestor when the full chain exists",
        &mut parse_src("xml", r#"
            <a>
                <b><c><d/></c></b>
            </a>
        "#),
        "//a[b/c/d]",
        1);
}

/// A bare-child predicate (`a[x]`) is an existence check: the
/// parent matches once even if multiple matching children exist.
/// Without this guarantee, `count(//a[x])` would conflate "how
/// many `a` have an `x` child" with "how many `a/x` pairs exist".
#[test]
fn xml_bare_child_predicate_matches_parent_once() {
    claim("`a[x]` matches the single ancestor once even with two `x` children",
        &mut parse_src("xml", r#"
            <a>
                <x/>
                <x/>
            </a>
        "#),
        "//a[x]",
        1);
}
