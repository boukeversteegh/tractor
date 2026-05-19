//! Singleton-children lifter transformer.
//!
//! Marks the single semantic child of "singleton wrappers" (e.g.
//! `<value>`, `<returns>`) with a `field` attribute so the JSON
//! serializer can lift it as a direct property instead of nesting it
//! under a `children` array. Per-language wrapper lists live in
//! `languages/mod.rs`; defaults shared by most C-like languages live
//! in [`DEFAULT_SINGLETON_WRAPPERS`].

use xot::{Xot, Node as XotNode};

use super::helpers::{get_attr, get_element_children, get_element_name, set_attr};

/// Default singleton wrappers — wrappers that typically contain exactly
/// one semantic child. Languages may adjust this list.
pub const DEFAULT_SINGLETON_WRAPPERS: &[&str] = &[
    "value",        // assigned/initial value
    "left",         // binary left operand
    "right",        // binary right operand
    "condition",    // if/while/for condition
    "then",         // if true branch (flat-conditional shape)
    "returns",      // return type
    // Note: "body" and "else" excluded — "else" can either hold a block
    // (single child) or, for the non-C-like languages where the grammar
    // already produces flat alternatives, be unused. More importantly,
    // after transforms inline declaration lists, body can contain
    // multiple children. Lifting the first child would violate
    // cardinality-independence (issue #34).
];

/// Mark the first element child of singleton wrappers with `field`.
///
/// Singleton wrappers (e.g. `<value>`, `<returns>`) contain exactly one
/// semantic child element. Adding `field="{element_name}"` to that child
/// lets the JSON serializer lift it as a direct property instead of
/// wrapping it in a `children` array.
///
/// Call after language transforms with a language-specific wrapper list.
pub fn lift_singleton_children(xot: &mut Xot, root: XotNode, singletons: &[&str]) {
    let all_elements = collect_all_elements(xot, root);
    for node in all_elements {
        let name = match get_element_name(xot, node) {
            Some(n) => n,
            None => continue,
        };
        if !singletons.contains(&name.as_str()) { continue; }
        // Must be a wrapper (has field attr, no kind attr)
        if get_attr(xot, node, "field").is_none() { continue; }
        if get_attr(xot, node, "kind").is_some() { continue; }
        // Add field to first element child if it doesn't already have one
        let children = get_element_children(xot, node);
        if let Some(&child) = children.first() {
            if get_attr(xot, child, "field").is_none() {
                if let Some(child_name) = get_element_name(xot, child) {
                    set_attr(xot, child, "field", &child_name);
                }
            }
        }
    }
}

/// Recursively collect all element nodes in document order
fn collect_all_elements(xot: &Xot, node: XotNode) -> Vec<XotNode> {
    let mut result = Vec::new();
    collect_elements_recursive(xot, node, &mut result);
    result
}

fn collect_elements_recursive(xot: &Xot, node: XotNode, result: &mut Vec<XotNode>) {
    if xot.element(node).is_some() {
        result.push(node);
    }
    for child in xot.children(node) {
        collect_elements_recursive(xot, child, result);
    }
}
